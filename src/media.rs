//! Pull-only image cache. Rendering never makes requests to remote servers.
use crate::store::{Snapshot, Store};
use anyhow::{bail, Context, Result};
use pulldown_cmark::{Event, Parser, Tag};
use reqwest::{blocking::Client, redirect::Policy, Url};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 20 * 1024 * 1024;
fn allowed(url: &Url) -> bool {
    let host = url.host_str().unwrap_or("");
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && (host.ends_with(".githubusercontent.com")
            || host == "github.com"
                && (url.path().starts_with("/user-attachments/")
                    || url.path().contains("/assets/")
                    || url.path().contains("/files/"))
            || host.starts_with("github-production-user-asset-")
                && host.ends_with(".s3.amazonaws.com"))
}
fn key(url: &str) -> String {
    format!("{:x}", Sha256::digest(url.as_bytes()))
}
fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.get(4..8) == Some(b"ftyp")
        && matches!(bytes.get(8..12), Some(b"avif" | b"avis"))
    {
        Some("image/avif")
    } else {
        None
    }
}
fn urls(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    static IMG: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let img = IMG.get_or_init(|| {
        regex::Regex::new(r#"(?is)<img\b[^>]*\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))[^>]*>"#)
            .unwrap()
    });
    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                result.insert(dest_url.to_string());
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for capture in img.captures_iter(&html) {
                    if let Some(source) = capture.get(1).or(capture.get(2)).or(capture.get(3)) {
                        result.insert(source.as_str().replace("&amp;", "&"));
                    }
                }
            }
            _ => {}
        }
    }
    result
}
fn collect(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(body) = map.get("body").and_then(Value::as_str) {
                found.extend(urls(body));
            }
            for (k, v) in map {
                if k != "body" {
                    collect(v, found);
                }
            }
        }
        Value::Array(rows) => {
            for row in rows {
                collect(row, found);
            }
        }
        _ => {}
    }
}
fn download(client: &Client, url: &str) -> Result<Vec<u8>> {
    let parsed = Url::parse(url)?;
    if !allowed(&parsed) {
        bail!("Only HTTPS GitHub-hosted images are downloaded");
    }
    let response = client.get(parsed).send()?.error_for_status()?;
    if response.content_length().is_some_and(|n| n > MAX_BYTES) {
        bail!("Image exceeds 20 MiB");
    }
    let mut bytes = Vec::new();
    response.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        bail!("Image exceeds 20 MiB");
    }
    image_type(&bytes).context("Unsupported image (PNG, JPEG, GIF, WebP and AVIF supported)")?;
    Ok(bytes)
}
pub fn pull(store: &Store, snapshot: &Snapshot) -> Result<Value> {
    // Both Pull and post-Push refresh can run inside the CLI's async runtime.
    // reqwest's blocking client must be created and dropped outside that runtime.
    std::thread::scope(|scope| {
        scope
            .spawn(|| pull_images(store, snapshot))
            .join()
            .map_err(|_| anyhow::anyhow!("Image downloader worker failed"))?
    })
}
fn pull_images(store: &Store, snapshot: &Snapshot) -> Result<Value> {
    let folder = store.root.join("media");
    fs::create_dir_all(&folder)?;
    let mut found = BTreeSet::new();
    for row in snapshot.details.values() {
        collect(row, &mut found);
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .referer(false)
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Image redirect outside GitHub image hosts")
            }
        }))
        .build()?;
    let start = Instant::now();
    let (mut downloaded, mut cached) = (0, 0);
    let mut failures = Vec::new();
    for url in &found {
        let path = folder.join(key(url));
        if path.is_file() {
            cached += 1;
            continue;
        }
        let result = if start.elapsed() >= Duration::from_secs(120) {
            Err(anyhow::anyhow!(
                "Image pull time budget reached; retry next Pull"
            ))
        } else {
            download(&client, url)
        };
        match result {
            Ok(bytes) => {
                let mut temporary = tempfile::NamedTempFile::new_in(&folder)?;
                use std::io::Write;
                temporary.write_all(&bytes)?;
                temporary.as_file().sync_all()?;
                temporary.persist(&path)?;
                downloaded += 1;
            }
            Err(error) => {
                failures.push(json!({"url":url,"error":error.to_string()}));
            }
        }
    }
    let report = json!({"downloaded":downloaded,"cached":cached,"failed":failures.len(),"failures":failures});
    let mut temporary = tempfile::NamedTempFile::new_in(store.dir(&snapshot.full_name)?)?;
    serde_json::to_writer(&mut temporary, &report)?;
    temporary.persist(store.dir(&snapshot.full_name)?.join("image-report.json"))?;
    Ok(report)
}
pub fn read(store: &Store, url: &str) -> Result<(Vec<u8>, &'static str)> {
    let bytes = fs::read(store.root.join("media").join(key(url)))?;
    let mime = image_type(&bytes).context("Unsupported cached image")?;
    Ok((bytes, mime))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit network smoke test against a public GitHub image"]
    fn live_github_image() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().into()).unwrap();
        store.add("test/repo").unwrap();
        let mut snapshot = store.load("test/repo").unwrap();
        let url = "https://raw.githubusercontent.com/github/explore/main/topics/rust/rust.png";
        snapshot
            .details
            .insert(1, json!({"comments":[{"body":format!("![Rust]({url})")}]}));
        assert_eq!(pull(&store, &snapshot).unwrap()["downloaded"], 1);
        let (bytes, mime) = read(&store, url).unwrap();
        assert_eq!(mime, "image/png");
        assert!(bytes.len() > 100);
        let report = pull(&store, &snapshot).unwrap();
        assert_eq!(report["downloaded"], 0);
        assert_eq!(report["cached"], 1);
    }
    #[test]
    fn markdown_html_reference_and_code() {
        let body = "![a](https://github.com/user-attachments/assets/a)\n![b][ref]\n\n[ref]: https://user-images.githubusercontent.com/b\n\n<img width=100 src=\"https://github.com/user-attachments/assets/c?x=1&amp;y=2\" />\n\n`![code](https://github.com/user-attachments/assets/no)`";
        let found = urls(body);
        assert_eq!(found.len(), 3);
        assert!(found.contains("https://github.com/user-attachments/assets/c?x=1&y=2"));
    }
    #[test]
    fn restrict_hosts_redirects_and_active_content() {
        for bad in [
            "http://user-images.githubusercontent.com/x",
            "https://127.0.0.1/x",
            "https://github.com.evil.test/user-attachments/x",
            "https://github.com/login",
            "https://user-images.githubusercontent.com:8443/x",
            "https://user:secret@user-images.githubusercontent.com/x",
        ] {
            assert!(!allowed(&Url::parse(bad).unwrap()), "{bad}");
        }
        assert!(allowed(
            &Url::parse("https://github.com/user-attachments/assets/id").unwrap()
        ));
        assert!(allowed(
            &Url::parse("https://private-user-images.githubusercontent.com/a").unwrap()
        ));
        assert_eq!(image_type(b"<svg onload=alert(1)>"), None);
        assert_eq!(image_type(b"<html>login</html>"), None);
    }
    #[test]
    fn cache_is_local_atomic_and_missing_files_retry() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::new(root.path().into()).unwrap();
        store.add("test/repo").unwrap();
        let mut snapshot = store.load("test/repo").unwrap();
        let url = "https://user-images.githubusercontent.com/test";
        snapshot.details.insert(1, json!({"comments":[{"body":format!("![image]({url})")},{"body":"![external](http://localhost/private)"}]}));
        fs::create_dir_all(root.path().join("media")).unwrap();
        fs::write(
            root.path().join("media").join(key(url)),
            b"\x89PNG\r\n\x1a\nfixture",
        )
        .unwrap();
        let report = pull(&store, &snapshot).unwrap();
        assert_eq!(report["cached"], 1);
        assert_eq!(report["failed"], 1);
        assert_eq!(read(&store, url).unwrap().1, "image/png");
        assert!(read(&store, "../../snapshot.json").is_err());
        assert!(store
            .dir("test/repo")
            .unwrap()
            .join("image-report.json")
            .exists());
    }
}
