use crate::process::run;
use anyhow::{bail, Context, Result};
use clap::Args;
use serde_json::{json, Map, Value};
use std::io::Write;
#[derive(Args, Clone, Default)]
pub struct Format {
    /// Optional comma-separated gh field names; bare --json preserves legacy ogh output
    #[arg(long,num_args=0..=1,default_missing_value="")]
    pub json: Option<String>,
    /// Filter JSON with local jq (jq must be installed)
    #[arg(long, short = 'q')]
    pub jq: Option<String>,
}
pub fn project(value: &Value, fields: &str) -> Result<Value> {
    if fields.is_empty() {
        return Ok(value.clone());
    }
    if let Some(rows) = value.as_array() {
        return Ok(json!(rows
            .iter()
            .map(|v| project(v, fields))
            .collect::<Result<Vec<_>>>()?));
    }
    let object = value
        .as_object()
        .context("Requested JSON fields require an object")?;
    let mut out = Map::new();
    for field in fields.split(',') {
        let v=object.get(field).with_context(||format!("Field '{field}' is not available offline; prepare the PR context or choose supported fields"))?;
        out.insert(field.into(), v.clone());
    }
    Ok(Value::Object(out))
}
pub fn emit(value: &Value, fmt: &Format) -> Result<()> {
    let value = if let Some(fields) = &fmt.json {
        project(value, fields)?
    } else {
        value.clone()
    };
    if let Some(filter) = &fmt.jq {
        let mut file = tempfile::NamedTempFile::new()?;
        serde_json::to_writer(&mut file, &value)?;
        file.flush()?;
        let path = file.path().to_str().context("Invalid temporary path")?;
        print!(
            "{}",
            run("jq", &["-r", "-c", "--", filter, path], 10)
                .context("--jq requires a working local jq installation and a valid filter")?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}
fn person(p: &Value) -> Value {
    if p.is_null() {
        Value::Null
    } else {
        json!({"login":p["login"],"id":p["node_id"],"is_bot":p["type"]=="Bot"})
    }
}
fn comments(v: &Value) -> Value {
    json!(v.as_array().unwrap_or(&vec![]).iter().map(|c|json!({"id":c["id"],"databaseId":c["id"],"body":c["body"],"author":person(&c["user"]),"url":c["html_url"],"createdAt":c["created_at"],"updatedAt":c["updated_at"]})).collect::<Vec<_>>())
}
pub fn item(item: &Value, detail: Option<&Value>, repo: &str) -> Result<Value> {
    let d = detail.unwrap_or(&Value::Null);
    let pr = item.get("pull_request").is_some();
    let url = item["html_url"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "https://github.com/{repo}/{}/{}",
                if pr { "pull" } else { "issues" },
                item["number"]
            )
        });
    let mut result = json!({"number":item["number"],"title":item["title"],"body":item["body"],"state":if !item["merged_at"].is_null(){"MERGED"}else if item["state"]=="closed"{"CLOSED"}else{"OPEN"},"author":person(&item["user"]),"url":url,"labels":item["labels"].as_array().cloned().unwrap_or_default(),"assignees":item["assignees"].as_array().cloned().unwrap_or_default(),"createdAt":item["created_at"],"updatedAt":item["updated_at"],"closedAt":item["closed_at"],"comments":comments(&d["comments"]),"isDraft":item["draft"].as_bool().unwrap_or(false),"mergedAt":item["merged_at"],"baseRefName":item["base"]["ref"],"baseRefOid":item["base"]["sha"],"headRefName":item["head"]["ref"],"headRefOid":item["head"]["sha"],"headRepository":item["head"]["repo"],"headRepositoryOwner":item["head"]["repo"]["owner"],"mergeCommit":if item["merged_at"].is_null(){Value::Null}else{json!({"oid":item["merge_commit_sha"]})}});
    if let Some(files) = d["files"].as_array() {
        if !files.is_empty() {
            result["files"]=json!(files.iter().map(|v|json!({"path":v["filename"],"additions":v["additions"],"deletions":v["deletions"]})).collect::<Vec<_>>());
        }
    }
    if let Some(commits) = d["commits"]
        .as_array()
        .filter(|_| !d["skills_head_sha"].is_null() && d["skills_head_sha"] == item["head"]["sha"])
    {
        result["commits"]=json!(commits.iter().map(|v|json!({"oid":v["sha"],"messageHeadline":v["commit"]["message"].as_str().unwrap_or("").lines().next().unwrap_or("")})).collect::<Vec<_>>());
    }
    if d["reviews_cached"] == true {
        result["reviews"]=json!(d["reviews"].as_array().unwrap_or(&vec![]).iter().map(|v|json!({"id":v["id"],"author":person(&v["user"]),"body":v["body"],"state":v["state"],"submittedAt":v["submitted_at"],"commit":{"oid":v["commit_id"]}})).collect::<Vec<_>>());
    }
    if d["skills_head_sha"] == item["head"]["sha"]
        && !d["skills_head_sha"].is_null()
        && d["skills_item_updated_at"] == item["updated_at"]
    {
        if let Some(gh) = d["gh"].as_object() {
            for (key, v) in gh {
                result[key] = v.clone();
            }
        }
    }
    if pr {
        if result["headRepository"].is_null() {
            result.as_object_mut().unwrap().remove("headRepository");
        }
        if result["headRepositoryOwner"].is_null() {
            result
                .as_object_mut()
                .unwrap()
                .remove("headRepositoryOwner");
        }
    }
    Ok(result)
}
pub fn require_fields(fmt: &Format) -> Result<()> {
    if fmt
        .json
        .as_ref()
        .is_some_and(|s| s.split(',').any(|v| v.is_empty()) && !s.is_empty())
    {
        bail!("JSON field list contains an empty field");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_fields() {
        assert!(project(&json!({"title":"x"}), "reviewDecision").is_err());
        assert_eq!(
            project(&json!([{ "number":1,"body":"x"}]), "number").unwrap(),
            json!([{ "number":1}])
        );
    }
}
