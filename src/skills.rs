use crate::{
    offline_api::missing,
    process::{git, run},
    store::{Snapshot, Store},
};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

fn api(endpoint: &str) -> Result<Value> {
    Ok(serde_json::from_str(&run(
        "gh",
        &["api", "--method", "GET", endpoint],
        120,
    )?)?)
}
fn pages(endpoint: &str, key: Option<&str>) -> Result<Vec<Value>> {
    let raw = run(
        "gh",
        &["api", "--method", "GET", "--paginate", "--slurp", endpoint],
        1800,
    )?;
    let pages: Vec<Value> = serde_json::from_str(&raw)?;
    let mut rows = Vec::new();
    for page in pages {
        let values = if let Some(k) = key {
            page[k].as_array()
        } else {
            page.as_array()
        }
        .context("Invalid paginated GitHub response")?;
        rows.extend(values.iter().cloned());
    }
    Ok(rows)
}
fn capture(s: &mut Snapshot, path: String, data: Value, at: &str, sha: &str, pr: u64) {
    s.resources.insert(
        path,
        json!({"data":data,"cached_at":at,"head_sha":sha,"pr_number":pr}),
    );
}
pub fn prepare(store: &Store, repo: &str, n: u64, logs: bool) -> Result<Value> {
    let _guard = store.lock()?;
    let mut s = store.load(repo)?;
    anyhow::ensure!(
        s.last_sync.is_some(),
        "Run ogh sync -R {repo} before preparing a PR"
    );
    let prefix = format!("repos/{repo}");
    let mut item = api(&format!("{prefix}/pulls/{n}"))?;
    let sha = item["head"]["sha"]
        .as_str()
        .context("PR head missing")?
        .to_owned();
    let base = item["base"]["ref"]
        .as_str()
        .context("PR base missing")?
        .to_owned();
    let expected_sha = sha.clone();
    let at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    eprintln!("Preparing {repo} PR #{n} at {sha}; GitHub reads only…");
    let mirror = store.mirror(repo)?;
    git(
        &mirror,
        &[
            "fetch",
            "origin",
            &format!("+refs/pull/{n}/head:refs/pull/{n}/head"),
            &format!("+refs/heads/{base}:refs/remotes/origin/{base}"),
        ],
    )?;
    git(&mirror, &["cat-file", "-e", &format!("{sha}^{{commit}}")])?;
    let comments = pages(&format!("{prefix}/issues/{n}/comments?per_page=100"), None)?;
    let review_comments = pages(&format!("{prefix}/pulls/{n}/comments?per_page=100"), None)?;
    let reviews = pages(&format!("{prefix}/pulls/{n}/reviews?per_page=100"), None)?;
    let files = pages(&format!("{prefix}/pulls/{n}/files?per_page=100"), None)?;
    let commits = pages(&format!("{prefix}/pulls/{n}/commits?per_page=100"), None)?;
    // gh pr view is a whitelisted READ command (GraphQL query), never gh edit/review/merge.
    let gh:Value=serde_json::from_str(&run("gh",&["pr","view",&n.to_string(),"--repo",repo,"--json","headRefOid,mergeable,mergeStateStatus,reviewDecision,closingIssuesReferences,latestReviews,reviewRequests,isCrossRepository,maintainerCanModify,headRepository,headRepositoryOwner,additions,deletions,changedFiles"],120)?)?;
    anyhow::ensure!(
        gh["headRefOid"] == sha,
        "PR head changed during preparation; retry safely"
    );
    if let Some(count) = item["commits"].as_u64() {
        anyhow::ensure!(
            commits.len() as u64 == count,
            "Commit pagination incomplete: expected {count}, got {}",
            commits.len()
        );
    }
    if let Some(count) = item["changed_files"].as_u64() {
        anyhow::ensure!(files.len() as u64==count,"File pagination incomplete: expected {count}, got {}; use local Git (GitHub caps PR files)",files.len());
    }
    s.metadata = api(&prefix)?;
    s.description = s.metadata["description"].as_str().unwrap_or("").into();
    let labels = pages(&format!("{prefix}/labels?per_page=100"), None)?;
    capture(
        &mut s,
        format!("{prefix}/labels"),
        json!(labels),
        &at,
        &sha,
        n,
    );
    capture(&mut s, "user".into(), api("user")?, &at, &sha, n);
    let checks = pages(
        &format!("{prefix}/commits/{sha}/check-runs?per_page=100"),
        Some("check_runs"),
    )?;
    capture(
        &mut s,
        format!("{prefix}/commits/{sha}/check-runs"),
        json!({"total_count":checks.len(),"check_runs":checks}),
        &at,
        &sha,
        n,
    );
    let statuses = api(&format!("{prefix}/commits/{sha}/status"))?;
    capture(
        &mut s,
        format!("{prefix}/commits/{sha}/status"),
        statuses,
        &at,
        &sha,
        n,
    );
    let protection_path = format!("{prefix}/branches/{base}/protection/required_status_checks");
    let mut gaps = Vec::new();
    match api(&protection_path) {
        Ok(v) => capture(&mut s, protection_path.clone(), v, &at, &sha, n),
        Err(e) => {
            gaps.push(format!("Required-check policy unavailable: {e}"));
            s.resources.insert(
                protection_path,
                json!({"error":e.to_string(),"cached_at":at,"head_sha":sha,"pr_number":n}),
            );
        }
    }
    let runs = api(&format!(
        "{prefix}/actions/runs?head_sha={sha}&per_page=100"
    ))?;
    let runs = runs["workflow_runs"]
        .as_array()
        .context("Invalid workflow runs")?
        .iter()
        .take(20)
        .cloned()
        .collect::<Vec<_>>();
    for row in &runs {
        let id = row["id"].as_u64().context("Invalid workflow run id")?;
        let jobs = pages(
            &format!("{prefix}/actions/runs/{id}/jobs?per_page=100"),
            Some("jobs"),
        )?;
        let mut details = row.clone();
        details["jobs"] = json!(jobs);
        capture(
            &mut s,
            format!("{prefix}/actions/runs/{id}"),
            details,
            &at,
            &sha,
            n,
        );
        capture(
            &mut s,
            format!("{prefix}/actions/runs/{id}/jobs"),
            json!({"total_count":jobs.len(),"jobs":jobs}),
            &at,
            &sha,
            n,
        );
        let log_key = format!("logs/{id}");
        s.resources.remove(&log_key);
        if logs {
            let mut text = String::new();
            let mut failed = false;
            for job in jobs.iter().filter(|j| {
                matches!(
                    j["conclusion"].as_str(),
                    Some("failure" | "timed_out" | "cancelled")
                )
            }) {
                let jid = job["id"].as_u64().context("Missing job id")?;
                match run(
                    "gh",
                    &[
                        "api",
                        "--method",
                        "GET",
                        &format!("{prefix}/actions/jobs/{jid}/logs"),
                    ],
                    120,
                ) {
                    Ok(raw) => {
                        text.push_str(&format!(
                            "\n=== {} (job {jid}) ===\n{raw}\n",
                            job["name"].as_str().unwrap_or("Job")
                        ));
                    }
                    Err(e) => {
                        failed = true;
                        gaps.push(format!("Run {id} failed logs unavailable: {e}"));
                    }
                }
            }
            if !failed {
                capture(&mut s, log_key, json!(text), &at, &sha, n);
            }
        }
    }
    // Store a per-head run collection; a later prepared PR cannot erase an earlier one.
    capture(
        &mut s,
        format!("{prefix}/actions/runs/{sha}"),
        json!({"total_count":runs.len(),"workflow_runs":runs}),
        &at,
        &sha,
        n,
    );
    item["pull_request"] = json!({"url":format!("https://api.github.com/{prefix}/pulls/{n}")});
    let updated = item["updated_at"].clone();
    let count = comments.len();
    s.items.insert(n, item.clone());
    s.details.insert(n,json!({"item":item,"comments":comments,"review_comments":review_comments,"reviews":reviews,"reviews_cached":true,"files":files,"commits":commits,"gh":gh,"diff":"","skills_head_sha":sha,"skills_item_updated_at":updated,"skills_prepared_at":at,"commit_count":commits.len(),"workflow_run_limit":20,"gaps":gaps}));
    // Re-read the head after all downloads. On races the prior snapshot stays intact.
    let end = api(&format!("{prefix}/pulls/{n}"))?;
    anyhow::ensure!(
        end["head"]["sha"] == expected_sha && end["updated_at"] == updated,
        "PR changed during preparation; retry safely"
    );
    store.save(&s)?;
    Ok(
        json!({"repository":repo,"number":n,"head_sha":expected_sha,"prepared_at":at,"comments":count,"commits":commits.len(),"files":files.len(),"runs":runs.len(),"gaps":gaps,"read_only":true,"live_merge_authorization":false}),
    )
}
fn prepared(s: &Snapshot, n: u64) -> Result<&Value> {
    let d = s.details.get(&n).context("PR is not cached")?;
    anyhow::ensure!(d["item"].get("pull_request").is_some(), "Item is not a PR");
    anyhow::ensure!(
        !d["skills_head_sha"].is_null()
            && d["skills_head_sha"] == d["item"]["head"]["sha"]
            && d["skills_item_updated_at"] == d["item"]["updated_at"],
        "PR context is missing or stale; run ogh skills prepare --pr {n}"
    );
    Ok(d)
}
pub fn resource(s: &Snapshot, key: &str) -> Result<Value> {
    let value = s.resources.get(key).ok_or_else(|| missing(key))?;
    if let Some(error) = value["error"].as_str() {
        bail!("Snapshot could not capture {key}: {error}");
    }
    value.get("data").cloned().ok_or_else(|| missing(key))
}
pub fn checks(store: &Store, repo: &str, n: u64, required: bool) -> Result<Value> {
    let s = store.load(repo)?;
    let d = prepared(&s, n)?;
    let sha = d["skills_head_sha"].as_str().unwrap();
    let prefix = format!("repos/{repo}");
    eprintln!(
        "Cached checks from {} at head {sha}; never a live merge gate",
        d["skills_prepared_at"].as_str().unwrap_or("unknown time")
    );
    let raw = resource(&s, &format!("{prefix}/commits/{sha}/check-runs"))?;
    let statuses = resource(&s, &format!("{prefix}/commits/{sha}/status"))?;
    let mut rows:Vec<Value>=raw["check_runs"].as_array().context("Check runs missing")?.iter().map(|v|json!({"name":v["name"],"state":v["conclusion"].as_str().unwrap_or_else(||v["status"].as_str().unwrap_or("pending")).to_uppercase(),"link":v["html_url"]})).collect();
    rows.extend(statuses["statuses"].as_array().context("Commit statuses missing")?.iter().map(|v|json!({"name":v["context"],"state":v["state"].as_str().unwrap_or("pending").to_uppercase(),"link":v["target_url"]})));
    if required {
        let policy = resource(
            &s,
            &format!(
                "{prefix}/branches/{}/protection/required_status_checks",
                d["item"]["base"]["ref"]
                    .as_str()
                    .context("Missing base ref")?
            ),
        )?;
        let contexts = policy["contexts"]
            .as_array()
            .context("Required contexts missing")?;
        rows.retain(|v| contexts.contains(&v["name"]));
    }
    Ok(json!(rows))
}
fn all_runs(s: &Snapshot, repo: &str) -> Vec<Value> {
    let prefix = format!("repos/{repo}/actions/runs/");
    let mut runs = std::collections::BTreeMap::new();
    for (key, v) in &s.resources {
        if key.starts_with(&prefix) && key[prefix.len()..].len() == 40 {
            if let Some(rows) = v["data"]["workflow_runs"].as_array() {
                for row in rows {
                    if let Some(id) = row["id"].as_u64() {
                        runs.insert(id, row.clone());
                    }
                }
            }
        }
    }
    runs.into_values().rev().collect()
}
pub fn raw_runs(s: &Snapshot, repo: &str) -> Result<Value> {
    let rows = all_runs(s, repo);
    anyhow::ensure!(
        s.resources
            .keys()
            .any(|k| k.starts_with(&format!("repos/{repo}/actions/runs/"))),
        "Workflow runs have not been prepared"
    );
    Ok(json!({"total_count":rows.len(),"workflow_runs":rows}))
}
fn normalize_run(v: &Value) -> Value {
    json!({"databaseId":v["id"],"workflowName":v["name"],"name":v["display_title"],"status":v["status"],"conclusion":v["conclusion"],"headSha":v["head_sha"],"headBranch":v["head_branch"],"url":v["html_url"],"createdAt":v["created_at"],"updatedAt":v["updated_at"],"jobs":v["jobs"]})
}
pub fn runs(store: &Store, repo: &str, branch: Option<&str>, limit: usize) -> Result<Value> {
    let s = store.load(repo)?;
    let raw = raw_runs(&s, repo)?;
    let rows = raw["workflow_runs"].as_array().unwrap();
    eprintln!("Offline workflow snapshot: only explicitly prepared PR heads; at most 20 runs per preparation");
    Ok(json!(rows
        .iter()
        .filter(|v| branch.is_none_or(|b| v["head_branch"] == b))
        .take(limit)
        .map(normalize_run)
        .collect::<Vec<_>>()))
}
pub fn run_view(store: &Store, repo: &str, id: u64) -> Result<Value> {
    let s = store.load(repo)?;
    Ok(normalize_run(&resource(
        &s,
        &format!("repos/{repo}/actions/runs/{id}"),
    )?))
}
pub fn logs(store: &Store, repo: &str, id: u64) -> Result<String> {
    let s = store.load(repo)?;
    resource(&s, &format!("logs/{id}"))?
        .as_str()
        .map(str::to_owned)
        .context("Invalid cached log")
}
pub fn status(store: &Store, repo: &str, n: Option<u64>) -> Result<Value> {
    let s = store.load(repo)?;
    let mut value = json!({"repository":repo,"mode":"offline_read_only","last_sync":s.last_sync,"resources":s.resources.len(),"writes":false,"live_authentication_verified":false,"live_merge_authorization":false,"provider":"ogh skills provider","blocked":["claims and atomic locks","GitHub mutations and publishing","merges","workflow execution and watching","arbitrary GraphQL"]});
    if let Some(n) = n {
        let d = s.details.get(&n).context("PR not cached")?;
        value["pr"] = json!({"number":n,"head_sha":d["item"]["head"]["sha"],"prepared_head_sha":d["skills_head_sha"],"prepared_at":d["skills_prepared_at"],"context_valid":prepared(&s,n).is_ok(),"gaps":d["gaps"]});
    }
    Ok(value)
}
