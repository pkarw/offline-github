use crate::{
    process::git,
    store::{absolute, Store},
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;

fn commit(store: &Store, repo: &str, reference: &str) -> Result<String> {
    if reference.starts_with('-') || reference.contains(':') || reference.len() > 256 {
        bail!("Invalid git reference");
    }
    Ok(git(
        &store.mirror(repo)?,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
    )?
    .trim()
    .into())
}
pub fn diff(store: &Store, repo: &str, n: u64) -> Result<String> {
    let d = store.detail(repo, n)?;
    let item = &d["item"];
    if item.get("pull_request").is_none() {
        bail!("Item is not a pull request");
    }
    let base = item["base"]["sha"]
        .as_str()
        .context("Base SHA not cached")?;
    let head = item["head"]["sha"]
        .as_str()
        .context("Head SHA not cached")?;
    let base = commit(store, repo, base)?;
    let head = commit(store, repo, head)?;
    // GitHub PRs compare with the merge base.
    git(
        &store.mirror(repo)?,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            &format!("{base}...{head}"),
            "--",
        ],
    )
}
pub fn detail(store: &Store, repo: &str, n: u64) -> Result<Value> {
    let mut d = store.detail(repo, n)?;
    if d["item"].get("pull_request").is_some() {
        match diff(store, repo, n) {
            Ok(patch) => {
                // Keep UI response bounded. CLI `pr diff` returns the complete patch.
                let mut end = patch.len().min(4_000_000);
                while !patch.is_char_boundary(end) {
                    end -= 1;
                }
                d["diff"] = json!(&patch[..end]);
                d["diff_truncated"] = json!(end < patch.len());
                let item = &d["item"];
                let range = format!(
                    "{}...{}",
                    item["base"]["sha"].as_str().unwrap(),
                    item["head"]["sha"].as_str().unwrap()
                );
                let names = git(
                    &store.mirror(repo)?,
                    &["diff", "--name-only", "-z", &range, "--"],
                )?;
                d["files"] = json!(names
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .map(|s| json!({"filename":s,"status":"modified"}))
                    .collect::<Vec<_>>());
            }
            Err(e) => {
                d["diff_error"] = json!(e.to_string());
            }
        }
    }
    Ok(d)
}
pub fn checkout(store: &Store, repo: &str, n: u64, path: &Path) -> Result<Value> {
    let d = store.detail(repo, n)?;
    if d["item"].get("pull_request").is_none() {
        bail!("Item is not a pull request");
    }
    let sha = commit(
        store,
        repo,
        d["item"]["head"]["sha"]
            .as_str()
            .context("PR head not cached; sync first")?,
    )?;
    let path = absolute(path)?;
    if path.exists() {
        bail!("Checkout destination already exists; choose a new --dir path");
    }
    git(
        &store.mirror(repo)?,
        &[
            "worktree",
            "add",
            "--detach",
            path.to_str().context("Invalid checkout path")?,
            &sha,
        ],
    )?;
    Ok(
        json!({"path":path,"sha":sha,"number":n,"message":"Created detached worktree; use git switch -c to start a local branch"}),
    )
}
pub fn tree(store: &Store, repo: &str, reference: &str, path: &str) -> Result<Value> {
    let sha = commit(store, repo, reference)?;
    validate_path(path)?;
    let spec = if path.is_empty() {
        sha
    } else {
        format!("{sha}:{path}")
    };
    let raw = git(&store.mirror(repo)?, &["ls-tree", "-z", &spec])?;
    let mut rows = vec![];
    for row in raw.split('\0').filter(|s| !s.is_empty()) {
        if let Some((meta, name)) = row.split_once('\t') {
            let kind = meta.split_whitespace().nth(1).unwrap_or("blob");
            rows.push(json!({"name":name,"path":if path.is_empty(){name.into()}else{format!("{path}/{name}")},"kind":kind}));
        }
    }
    Ok(json!(rows))
}
fn validate_path(path: &str) -> Result<()> {
    if path.starts_with('/') || path.split('/').any(|p| p == "..") || path.contains('\0') {
        bail!("Invalid repository path");
    }
    Ok(())
}
pub fn blob(store: &Store, repo: &str, reference: &str, path: &str) -> Result<Value> {
    let sha = commit(store, repo, reference)?;
    validate_path(path)?;
    let spec = format!("{sha}:{path}");
    let size = git(&store.mirror(repo)?, &["cat-file", "-s", &spec])?
        .trim()
        .parse::<usize>()?;
    if size > 2_000_000 {
        bail!("File exceeds 2 MB preview limit; use local worktree");
    }
    Ok(json!({"content":git(&store.mirror(repo)?,&["show",&spec])?}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::run;
    use std::fs;
    #[test]
    fn offline_git_checkout_diff_and_browse() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let p = source.to_str().unwrap();
        run("git", &["init", "-b", "main", p], 10).unwrap();
        run(
            "git",
            &["-C", p, "config", "user.email", "test@example.org"],
            10,
        )
        .unwrap();
        run("git", &["-C", p, "config", "user.name", "Test"], 10).unwrap();
        fs::write(source.join("README.md"), "before\n").unwrap();
        run("git", &["-C", p, "add", "."], 10).unwrap();
        run("git", &["-C", p, "commit", "-m", "base"], 10).unwrap();
        let base = run("git", &["-C", p, "rev-parse", "HEAD"], 10)
            .unwrap()
            .trim()
            .to_owned();
        fs::write(source.join("README.md"), "after\n").unwrap();
        run("git", &["-C", p, "commit", "-am", "change"], 10).unwrap();
        let head = run("git", &["-C", p, "rev-parse", "HEAD"], 10)
            .unwrap()
            .trim()
            .to_owned();
        let store = Store::new(temp.path().join("cache")).unwrap();
        store.add("test/repo").unwrap();
        let mirror = store.mirror("test/repo").unwrap();
        run("git", &["clone", "--bare", p, mirror.to_str().unwrap()], 10).unwrap();
        let mut s = store.load("test/repo").unwrap();
        s.details.insert(7,json!({"item":{"number":7,"pull_request":{},"base":{"sha":base},"head":{"sha":head}},"comments":[],"reviews":[],"review_comments":[]}));
        store.save(&s).unwrap();
        assert!(diff(&store, "test/repo", 7).unwrap().contains("+after"));
        assert_eq!(
            tree(&store, "test/repo", "HEAD", "").unwrap()[0]["name"],
            "README.md"
        );
        assert_eq!(
            blob(&store, "test/repo", "HEAD", "README.md").unwrap()["content"],
            "after\n"
        );
        assert!(blob(&store, "test/repo", "--help", "README.md").is_err());
        assert!(tree(&store, "test/repo", "HEAD", "../").is_err());
        let dest = temp.path().join("checkout");
        checkout(&store, "test/repo", 7, &dest).unwrap();
        assert_eq!(
            fs::read_to_string(dest.join("README.md")).unwrap(),
            "after\n"
        );
        assert!(checkout(&store, "test/repo", 7, &dest).is_err());
    }
}
