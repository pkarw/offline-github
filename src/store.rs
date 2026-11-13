use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub full_name: String,
    pub description: String,
    pub default_branch: String,
    pub last_sync: Option<String>,
    pub items: BTreeMap<u64, Value>,
    pub details: BTreeMap<u64, Value>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub resources: BTreeMap<String, Value>,
}
pub fn validate_repo(name: &str) -> Result<()> {
    let parts: Vec<_> = name.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|s| {
            s.is_empty()
                || s.starts_with('.')
                || !s
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
    {
        bail!("Repository must be owner/repo");
    }
    Ok(())
}
#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    pub fn dir(&self, repo: &str) -> Result<PathBuf> {
        validate_repo(repo)?;
        Ok(self.root.join(repo))
    }
    pub fn mirror(&self, repo: &str) -> Result<PathBuf> {
        Ok(self.dir(repo)?.join("repo.git"))
    }
    pub fn load(&self, repo: &str) -> Result<Snapshot> {
        Ok(serde_json::from_slice(
            &fs::read(self.dir(repo)?.join("snapshot.json"))
                .context("Repository not cached; run ogh repo add first")?,
        )?)
    }
    pub fn save(&self, snapshot: &Snapshot) -> Result<()> {
        let dir = self.dir(&snapshot.full_name)?;
        fs::create_dir_all(&dir)?;
        let tmp = dir.join("snapshot.tmp");
        let mut file = fs::File::create(&tmp)?;
        use std::io::Write;
        file.write_all(&serde_json::to_vec(snapshot)?)?;
        file.sync_all()?;
        fs::rename(tmp, dir.join("snapshot.json"))?;
        Ok(())
    }
    pub fn lock(&self) -> Result<fs::File> {
        let f = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.root.join("sync.lock"))?;
        fs2::FileExt::try_lock_exclusive(&f)
            .context("Another cache operation is running; retry when it finishes")?;
        Ok(f)
    }
    pub fn add(&self, repo: &str) -> Result<Value> {
        let _guard = self.lock()?;
        validate_repo(repo)?;
        let removed = self.dir(repo)?.join("removed.json");
        if removed.exists() {
            fs::remove_file(removed)?;
        }
        if self.dir(repo)?.join("snapshot.json").exists() {
            return self.summary(&self.load(repo)?);
        }
        let s = Snapshot {
            full_name: repo.into(),
            description: String::new(),
            default_branch: "main".into(),
            last_sync: None,
            items: BTreeMap::new(),
            details: BTreeMap::new(),
            metadata: Value::Null,
            resources: BTreeMap::new(),
        };
        self.save(&s)?;
        self.summary(&s)
    }
    /// Forget the listing without destroying recovery data or checkout files.
    pub fn remove(&self, repo: &str) -> Result<Value> {
        let _guard = self.lock()?;
        self.load(repo)?;
        let retained = self.dir(repo)?;
        fs::write(
            retained.join("removed.json"),
            serde_json::to_vec(&json!({"removed_at":chrono::Utc::now().to_rfc3339()}))?,
        )?;
        Ok(
            json!({"removed":true,"retained_path":retained,"message":"Removed from app. All files and outgoing changes retained; add the same repository to restore."}),
        )
    }
    pub fn summary(&self, s: &Snapshot) -> Result<Value> {
        Ok(
            json!({"full_name":s.full_name,"description":s.description,"default_branch":s.default_branch,"last_sync":s.last_sync,"issue_count":s.items.values().filter(|v|v.get("pull_request").is_none()).count(),"pr_count":s.items.values().filter(|v|v.get("pull_request").is_some()).count(),"path":self.mirror(&s.full_name)?}),
        )
    }
    pub fn repos(&self) -> Result<Vec<Value>> {
        let mut out = vec![];
        for owner in fs::read_dir(&self.root)? {
            let owner = owner?;
            if !owner.file_type()?.is_dir() {
                continue;
            }
            for repo in fs::read_dir(owner.path())? {
                let repo = repo?;
                let path = repo.path().join("snapshot.json");
                if path.exists() && !repo.path().join("removed.json").exists() {
                    let s: Snapshot = serde_json::from_slice(&fs::read(path)?)?;
                    out.push(self.summary(&s)?);
                }
            }
        }
        out.sort_by_key(|v| v["full_name"].as_str().unwrap_or("").to_owned());
        Ok(out)
    }
    pub fn items(&self, repo: &str, kind: &str, state: &str, query: &str) -> Result<Vec<Value>> {
        let s = self.load(repo)?;
        let q = query.to_lowercase();
        let mut out: Vec<_> = s
            .items
            .values()
            .filter(|v| {
                (kind == "all" || (v.get("pull_request").is_some()) == (kind == "pr"))
                    && (state == "all" || v["state"] == state)
                    && (q.is_empty()
                        || format!("{} {} {}", v["title"], v["body"], v["number"])
                            .to_lowercase()
                            .contains(&q))
            })
            .cloned()
            .collect();
        out.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
        Ok(out)
    }
    pub fn detail(&self, repo: &str, n: u64) -> Result<Value> {
        self.load(repo)?
            .details
            .get(&n)
            .cloned()
            .context("Item is not in the offline cache; sync while online")
    }
}
pub fn absolute(path: &Path) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    })
}

/// Preserve existing Git worktree links and live SQLite users during upgrades.
pub fn default_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Cannot determine home directory; set OGH_DATA_DIR")?;
    let root = home.join(".offline-github");
    if !root.try_exists()? {
        if let Some(legacy) = dirs::data_local_dir()
            .map(|p| p.join("offline-github"))
            .filter(|p| p.is_dir())
        {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&legacy, &root)?;
            }
            #[cfg(not(unix))]
            {
                anyhow::bail!(
                    "Existing cache at {}; set OGH_DATA_DIR to preserve it",
                    legacy.display()
                );
            }
        }
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal() {
        for s in ["../repo", "a/../../b", "a/b/c", "a/-b?", "/b", "a/.git"] {
            assert!(validate_repo(s).is_err(), "{s}");
        }
        assert!(validate_repo("open-mercato/cezar").is_ok());
    }
    #[test]
    fn cache_filters_and_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path().into()).unwrap();
        store.add("a/b").unwrap();
        let mut s = store.load("a/b").unwrap();
        s.items.insert(
            1,
            json!({"number":1,"title":"Fix Sync","state":"open","updated_at":"2026-01-01"}),
        );
        s.items.insert(2,json!({"number":2,"title":"PR","state":"closed","pull_request":{},"updated_at":"2026-01-02"}));
        store.save(&s).unwrap();
        assert_eq!(
            store.items("a/b", "issue", "open", "sync").unwrap().len(),
            1
        );
        assert_eq!(store.items("a/b", "pr", "open", "").unwrap().len(), 0);
        assert_eq!(store.repos().unwrap()[0]["pr_count"], 1);
    }
}
