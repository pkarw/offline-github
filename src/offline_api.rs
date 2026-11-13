use crate::{repository, store::Store};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn missing(resource: &str) -> anyhow::Error {
    anyhow::anyhow!("Resource '{resource}' is not cached. While online, run ogh skills prepare --pr NUMBER -R owner/repo (add --logs for failed logs). No online fallback was attempted.")
}
pub fn read(
    store: &Store,
    selected: Option<&str>,
    endpoint: &str,
    paginate: bool,
) -> Result<Value> {
    let endpoint = endpoint
        .strip_prefix("https://api.github.com/")
        .unwrap_or(endpoint)
        .trim_start_matches('/');
    anyhow::ensure!(
        !endpoint.contains("://") && !endpoint.contains("..") && !endpoint.contains('#'),
        "Only supported GitHub REST paths can be read offline"
    );
    let (path, query) = endpoint.split_once('?').unwrap_or((endpoint, ""));
    let params: HashMap<String, String> =
        serde_urlencoded::from_str(query).context("Invalid API query")?;
    let allowed = [
        "page",
        "per_page",
        "state",
        "sort",
        "direction",
        "since",
        "head",
        "base",
        "labels",
        "creator",
        "assignee",
        "head_sha",
        "branch",
        "status",
        "ref",
    ];
    for key in params.keys() {
        anyhow::ensure!(
            allowed.contains(&key.as_str()),
            "Unsupported offline API query parameter {key}"
        );
    }
    let mut components: Vec<_> = path.split('/').collect();
    let repo = if components.first() == Some(&"repos")
        && components.len() >= 3
        && components[1] != "{owner}"
    {
        format!("{}/{}", components[1], components[2])
    } else {
        crate::resolve_repo(store, selected)?
    };
    crate::store::validate_repo(&repo)?;
    let full = if components.first() == Some(&"repos") && components.len() >= 3 {
        components.drain(..3);
        components.join("/")
    } else if path == "user" {
        "user".into()
    } else {
        bail!("Unsupported offline REST endpoint '{path}'; GraphQL, live locks, and mutations are unavailable")
    };
    let s = store.load(&repo)?;
    let prefix = format!("repos/{repo}");
    let canonical = if full == "user" {
        "user".into()
    } else if full.is_empty() {
        prefix.clone()
    } else {
        format!("{prefix}/{full}")
    };
    let mut value = if let Some(resource) = s.resources.get(&canonical) {
        eprintln!(
            "Offline snapshot: {} (not live GitHub state)",
            resource["cached_at"].as_str().unwrap_or("unknown time")
        );
        crate::skills::resource(&s, &canonical)?
    } else if full == "actions/runs" {
        crate::skills::raw_runs(&s, &repo)?
    } else if full.is_empty() {
        if s.metadata.is_null() {
            json!({"full_name":repo,"name":repo.split('/').nth(1),"description":s.description,"default_branch":s.default_branch,"html_url":format!("https://github.com/{repo}")})
        } else {
            s.metadata.clone()
        }
    } else if full == "issues" || full == "pulls" {
        json!(s
            .items
            .values()
            .filter(|v| full == "issues" || v.get("pull_request").is_some())
            .cloned()
            .collect::<Vec<_>>())
    } else if full.starts_with("contents/") {
        let path = full.trim_start_matches("contents/");
        let reference = params.get("ref").map(String::as_str).unwrap_or("HEAD");
        match repository::blob(store, &repo, reference, path) {
            Ok(blob) => {
                let content = blob["content"].as_str().context("Invalid cached blob")?;
                json!({"name":path.rsplit('/').next(),"path":path,"type":"file","encoding":"base64","size":content.len(),"content":base64::engine::general_purpose::STANDARD.encode(content.as_bytes())})
            }
            Err(_) => repository::tree(store, &repo, reference, path)?,
        }
    } else {
        let c: Vec<_> = full.split('/').collect();
        match c.as_slice() {
            ["issues" | "pulls", n] => {
                let n = n.parse::<u64>().context("Invalid item number")?;
                let item = s.items.get(&n).context("Item not in cache")?;
                if c[0] == "pulls" {
                    anyhow::ensure!(item.get("pull_request").is_some(), "Item is not a PR");
                }
                item.clone()
            }
            ["issues", n, "comments"]
            | ["pulls", n, "comments"]
            | ["pulls", n, "reviews"]
            | ["pulls", n, "files"]
            | ["pulls", n, "commits"] => {
                let n = n.parse::<u64>()?;
                let d = s.details.get(&n).context("Item not in cache")?;
                if c[0] == "pulls" {
                    anyhow::ensure!(d["item"].get("pull_request").is_some(), "Item is not a PR");
                }
                let key = if c[0] == "pulls" && c[2] == "comments" {
                    "review_comments"
                } else {
                    c[2]
                };
                if key == "reviews" {
                    anyhow::ensure!(
                        d["reviews_cached"] == true,
                        "Reviews not prepared or stale; run ogh skills prepare --pr {n} -R {repo}"
                    );
                }
                if key == "files" {
                    if d["skills_head_sha"] == d["item"]["head"]["sha"]
                        && d.get("files")
                            .is_some_and(|v| v.as_array().is_some_and(|a| !a.is_empty()))
                    {
                        d["files"].clone()
                    } else {
                        let derived = repository::detail(store, &repo, n)?;
                        if let Some(error) = derived["diff_error"].as_str() {
                            bail!("Cannot derive files: {error}");
                        }
                        derived["files"].clone()
                    }
                } else {
                    d.get(key).cloned().ok_or_else(|| missing(&canonical))?
                }
            }
            ["issues", "comments", id] | ["pulls", "comments", id] => {
                let id = id.parse::<u64>()?;
                let key = if c[0] == "issues" {
                    "comments"
                } else {
                    "review_comments"
                };
                s.details
                    .values()
                    .filter_map(|d| d[key].as_array())
                    .flatten()
                    .find(|v| v["id"].as_u64() == Some(id))
                    .cloned()
                    .context("Comment not in offline cache")?
            }
            ["git", "ref", "heads", ..] => {
                let branch = c[3..].join("/");
                let reference = format!("refs/remotes/origin/{branch}");
                let sha = crate::process::git(
                    &store.mirror(&repo)?,
                    &["rev-parse", "--verify", "--end-of-options", &reference],
                )?;
                json!({"ref":format!("refs/heads/{branch}"),"object":{"sha":sha.trim(),"type":"commit"}})
            }
            _ => return Err(missing(&canonical)),
        }
    };
    // Filter cached collections before applying the requested REST page.
    let collection_key = if value["workflow_runs"].is_array() {
        Some("workflow_runs")
    } else if value["check_runs"].is_array() {
        Some("check_runs")
    } else if value["jobs"].is_array() {
        Some("jobs")
    } else {
        None
    };
    let rows = if let Some(key) = collection_key {
        value[key].as_array_mut()
    } else {
        value.as_array_mut()
    };
    if let Some(rows) = rows {
        let state = params.get("state").map(String::as_str).unwrap_or(
            if full == "issues" || full == "pulls" {
                "open"
            } else {
                "all"
            },
        );
        rows.retain(|v| {
            (state == "all" || v["state"] == state)
                && params
                    .get("since")
                    .is_none_or(|since| v["updated_at"].as_str().unwrap_or("") >= since.as_str())
                && params
                    .get("head_sha")
                    .is_none_or(|sha| v["head_sha"] == sha.as_str())
                && params
                    .get("branch")
                    .is_none_or(|b| v["head_branch"] == b.as_str())
                && params
                    .get("head")
                    .is_none_or(|b| v["head"]["ref"] == b.as_str())
                && params
                    .get("base")
                    .is_none_or(|b| v["base"]["ref"] == b.as_str())
                && params
                    .get("status")
                    .is_none_or(|b| v["status"] == b.as_str())
                && params
                    .get("creator")
                    .is_none_or(|b| v["user"]["login"] == b.as_str())
                && params.get("assignee").is_none_or(|b| {
                    v["assignees"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|p| p["login"] == b.as_str()))
                })
                && params.get("labels").is_none_or(|labels| {
                    labels.split(',').all(|label| {
                        v["labels"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|p| p["name"] == label))
                    })
                })
        });
        let sort = params.get("sort").map(String::as_str).unwrap_or("created");
        anyhow::ensure!(
            matches!(sort, "created" | "updated" | "comments"),
            "Unsupported API sort"
        );
        let key = if sort == "updated" {
            "updated_at"
        } else {
            "created_at"
        };
        rows.sort_by(|a, b| {
            if sort == "comments" {
                a["comments"].as_u64().cmp(&b["comments"].as_u64())
            } else {
                a[key].as_str().cmp(&b[key].as_str())
            }
        });
        let direction = params.get("direction").map(String::as_str).unwrap_or(
            if full == "issues" || full == "pulls" {
                "desc"
            } else {
                "asc"
            },
        );
        anyhow::ensure!(
            matches!(direction, "asc" | "desc"),
            "Unsupported API direction"
        );
        if direction == "desc" {
            rows.reverse();
        }
        if !paginate {
            let per = params
                .get("per_page")
                .map(|p| p.parse::<usize>())
                .transpose()?
                .unwrap_or(100)
                .clamp(1, 100);
            let page = params
                .get("page")
                .map(|p| p.parse::<usize>())
                .transpose()?
                .unwrap_or(1);
            anyhow::ensure!(page > 0, "API page must be positive");
            let start = (page - 1).checked_mul(per).context("Page overflow")?;
            *rows = rows.iter().skip(start).take(per).cloned().collect();
        }
    }
    Ok(value)
}
