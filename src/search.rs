use anyhow::{bail, Result};
use serde_json::Value;

fn words(query: &str) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escape = false;
    for c in query.chars() {
        if escape {
            word.push(c);
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c.is_whitespace() && !quoted {
            if !word.is_empty() {
                parts.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if quoted || escape {
        bail!("Unterminated search quote or escape");
    }
    if !word.is_empty() {
        parts.push(word);
    }
    Ok(parts)
}
fn date_match(date: &str, filter: &str) -> Result<bool> {
    if let Some((a, b)) = filter.split_once("..") {
        return Ok(!date.is_empty() && date >= a && date <= format!("{b}T99").as_str());
    }
    let (op, value) = for_date(filter);
    let actual = if value.len() == 10 {
        date.get(..10).unwrap_or(date)
    } else {
        date
    };
    Ok(!date.is_empty()
        && match op {
            ">=" => actual >= value,
            "<=" => actual <= value,
            ">" => actual > value,
            "<" => actual < value,
            _ => actual == value,
        })
}
fn for_date(s: &str) -> (&str, &str) {
    for op in [">=", "<=", ">", "<"] {
        if let Some(v) = s.strip_prefix(op) {
            return (op, v);
        }
    }
    ("=", s)
}
#[derive(Clone)]
pub struct Search {
    terms: Vec<String>,
}
impl Search {
    pub fn parse(query: &str) -> Result<Self> {
        let terms = words(query)?;
        for term in &terms {
            let raw = term.strip_prefix('-').unwrap_or(term);
            if raw == "OR" {
                bail!("OR search is not supported offline; run separate queries");
            }
            if let Some((key, val)) = raw.split_once(':') {
                match key{"in"=>{if !val.split(',').all(|v|matches!(v,"title"|"body"|"comments")){bail!("Unsupported in: search scope");}},"state"|"is"=>{if !matches!(val,"open"|"closed"|"merged"|"unmerged"|"pr"|"issue"|"draft"){bail!("Unsupported {key}:{val} qualifier");}},"draft"=>{if !matches!(val,"true"|"false"){bail!("draft: requires true or false");}},"author"|"assignee"|"label"|"base"|"head"|"repo"|"number"|"created"|"updated"|"closed"|"merged"=>{},_=>bail!("Unsupported offline search qualifier {key}:; no online fallback was attempted")}
            }
        }
        Ok(Self { terms })
    }
    pub fn matches(&self, item: &Value, detail: Option<&Value>, repo: &str) -> Result<bool> {
        let scopes = self
            .terms
            .iter()
            .find_map(|s| s.strip_prefix("in:"))
            .unwrap_or("title,body");
        let mut haystack = vec![item["number"].to_string()];
        for scope in scopes.split(',') {
            match scope {
                "title" | "body" => haystack.push(item[scope].as_str().unwrap_or("").into()),
                "comments" => {
                    if let Some(d) = detail {
                        if let Some(c) = d["comments"].as_array() {
                            haystack.extend(
                                c.iter()
                                    .map(|v| v["body"].as_str().unwrap_or("").to_owned()),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        let haystack = haystack.join(" ").to_lowercase();
        for term in &self.terms {
            let (negative, term) = if let Some(s) = term.strip_prefix('-') {
                (true, s)
            } else {
                (false, term.as_str())
            };
            let matched = if let Some((key, v)) = term.split_once(':') {
                match key {
                    "in" => continue,
                    "state" | "is" => match v {
                        "open" | "closed" => item["state"] == v,
                        "merged" => !item["merged_at"].is_null(),
                        "unmerged" => item["merged_at"].is_null(),
                        "draft" => item["draft"] == true,
                        "pr" => item.get("pull_request").is_some(),
                        "issue" => item.get("pull_request").is_none(),
                        _ => false,
                    },
                    "author" => item["user"]["login"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(v)),
                    "assignee" => item["assignees"].as_array().is_some_and(|a| {
                        a.iter().any(|x| {
                            x["login"]
                                .as_str()
                                .is_some_and(|s| s.eq_ignore_ascii_case(v))
                        })
                    }),
                    "label" => item["labels"].as_array().is_some_and(|a| {
                        a.iter().any(|x| {
                            x["name"]
                                .as_str()
                                .is_some_and(|s| s.eq_ignore_ascii_case(v))
                        })
                    }),
                    "base" | "head" => item[key]["ref"].as_str() == Some(v),
                    "draft" => item["draft"].as_bool().unwrap_or(false) == (v == "true"),
                    "repo" => repo.eq_ignore_ascii_case(v),
                    "number" => item["number"].to_string() == v,
                    "created" | "updated" | "closed" | "merged" => {
                        date_match(item[format!("{key}_at")].as_str().unwrap_or(""), v)?
                    }
                    _ => false,
                }
            } else {
                haystack.contains(&term.trim_start_matches('#').to_lowercase())
            };
            if matched == negative {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn qualifiers_and_dates() {
        let i = json!({"number":12,"title":"Fix sync","body":"broken","state":"closed","merged_at":"2026-11-02T01:00:00Z","created_at":"2026-10-01T00:00:00Z","user":{"login":"alice"},"labels":[{"name":"needs qa"}],"pull_request":{}});
        assert!(Search::parse(
            "sync in:title,body author:alice label:\"needs qa\" merged:>=2026-11-01 -is:open"
        )
        .unwrap()
        .matches(&i, None, "o/r")
        .unwrap());
        assert!(!Search::parse("is:unmerged")
            .unwrap()
            .matches(&i, None, "o/r")
            .unwrap());
        assert!(Search::parse("review:approved").is_err());
        assert!(Search::parse("foo OR bar").is_err());
    }
}
