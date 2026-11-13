use crate::{
    process::{git, run},
    store::Store,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

fn api(endpoint: &str) -> Result<Value> {
    Ok(serde_json::from_str(&run(
        "gh",
        &["api", "--method", "GET", endpoint],
        120,
    )?)?)
}
fn pages(endpoint: &str) -> Result<Vec<Value>> {
    let raw = run(
        "gh",
        &["api", "--method", "GET", "--paginate", "--slurp", endpoint],
        1800,
    )?;
    let pages: Vec<Vec<Value>> = serde_json::from_str(&raw)?;
    Ok(pages.into_iter().flatten().collect())
}
fn merge_comments(s: &mut crate::store::Snapshot, rows: Vec<Value>, key: &str) {
    for row in rows {
        let url = if key == "comments" {
            row["issue_url"].as_str()
        } else {
            row["pull_request_url"].as_str()
        };
        if let Some(n) = url
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.parse::<u64>().ok())
        {
            if let Some(d) = s.details.get_mut(&n) {
                let list = d[key].as_array_mut().unwrap();
                if let Some(index) = list.iter().position(|v| v["id"] == row["id"]) {
                    list[index] = row;
                } else {
                    list.push(row);
                }
            }
        }
    }
}
pub fn sync(store: &Store, repo: &str, reviews: bool, full: bool) -> Result<Value> {
    let _guard = store.lock()?;
    let mut s = store.load(repo)?;
    if full {
        s.last_sync = None;
        s.items.clear();
        s.details.clear();
    }
    let start = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let since = s
        .last_sync
        .as_ref()
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|v| {
            (v - chrono::Duration::minutes(2)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        });
    let suffix = since
        .as_ref()
        .map(|v| format!("&since={v}"))
        .unwrap_or_default();
    eprintln!("Syncing {repo}: repository and issue delta…");
    let metadata = api(&format!("repos/{repo}"))?;
    s.metadata = metadata.clone();
    s.description = metadata["description"].as_str().unwrap_or("").into();
    s.default_branch = metadata["default_branch"]
        .as_str()
        .context("Missing default branch")?
        .into();
    let issues = pages(&format!(
        "repos/{repo}/issues?state=all&sort=updated&direction=desc&per_page=100{suffix}"
    ))?;
    let changed = issues.len();
    for item in issues {
        let n = item["number"].as_u64().context("Invalid issue number")?;
        let d=s.details.entry(n).or_insert_with(||json!({"item":null,"comments":[],"reviews":[],"review_comments":[],"files":[],"diff":""}));
        if d["item"]["updated_at"] != item["updated_at"] {
            d["reviews_cached"] = json!(false);
        }
        d["item"] = item.clone();
        s.items.insert(n, item);
    }
    eprintln!("Syncing {repo}: pull request metadata…");
    // Pull requests do not accept `since`: stop after the sorted updated window.
    let mut page = 1;
    loop {
        let rows = api(&format!(
            "repos/{repo}/pulls?state=all&sort=updated&direction=desc&per_page=100&page={page}"
        ))?;
        let rows = rows.as_array().context("Expected pull request array")?;
        if rows.is_empty() {
            break;
        }
        let mut old = false;
        for pr in rows {
            if since
                .as_ref()
                .is_some_and(|v| pr["updated_at"].as_str().unwrap_or("") < v.as_str())
            {
                old = true;
                break;
            }
            let n = pr["number"].as_u64().context("Invalid pull request")?;
            if let Some(item) = s.items.get_mut(&n) {
                let marker = item["pull_request"].clone();
                let comments = item["comments"].clone();
                *item = pr.clone();
                item["pull_request"] = marker;
                item["comments"] = comments;
                s.details.get_mut(&n).unwrap()["item"] = item.clone();
            }
        }
        if old || rows.len() < 100 {
            break;
        }
        page += 1;
    }
    eprintln!("Syncing {repo}: conversations…");
    merge_comments(
        &mut s,
        pages(&format!(
            "repos/{repo}/issues/comments?sort=updated&direction=asc&per_page=100{suffix}"
        ))?,
        "comments",
    );
    merge_comments(
        &mut s,
        pages(&format!(
            "repos/{repo}/pulls/comments?sort=updated&direction=asc&per_page=100{suffix}"
        ))?,
        "review_comments",
    );
    // Git downloads are incremental; retain old PR refs for reproducible offline testing.
    let mirror = store.mirror(repo)?;
    if !mirror.exists() {
        eprintln!("Cloning full history for {repo}…");
        run(
            "git",
            &[
                "clone",
                "--bare",
                &format!("https://github.com/{repo}.git"),
                mirror.to_str().unwrap(),
            ],
            900,
        )?;
    }
    // Disable pushes on the cache itself. Generated worktrees inherit this safeguard.
    git(
        &mirror,
        &["config", "remote.origin.pushurl", "disabled://read-only"],
    )?;
    git(
        &mirror,
        &[
            "fetch",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
            "+refs/tags/*:refs/tags/*",
            "+refs/pull/*/head:refs/pull/*/head",
        ],
    )?;
    git(
        &mirror,
        &[
            "symbolic-ref",
            "HEAD",
            &format!("refs/remotes/origin/{}", s.default_branch),
        ],
    )?;
    if reviews {
        for (n, item) in &s.items {
            if item.get("pull_request").is_some()
                && (s.details[n]["reviews_cached"] != true
                    || since
                        .as_ref()
                        .is_none_or(|v| item["updated_at"].as_str().unwrap_or("") >= v.as_str()))
            {
                s.details.get_mut(n).unwrap()["reviews"] = json!(pages(&format!(
                    "repos/{repo}/pulls/{n}/reviews?per_page=100"
                ))?);
                s.details.get_mut(n).unwrap()["reviews_cached"] = json!(true);
            }
        }
    }
    s.last_sync = Some(start.clone());
    store.save(&s)?;
    Ok(
        json!({"message":format!("Synced {repo}; {changed} changed issues/PRs"),"changed":changed,"last_sync":start}),
    )
}
