use crate::store::Store;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn copy_assets(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        anyhow::ensure!(
            !kind.is_symlink(),
            "UI installation does not follow symlinks"
        );
        let dest = to.join(entry.file_name());
        if kind.is_dir() {
            copy_assets(&entry.path(), &dest)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), dest)?;
        } else {
            anyhow::bail!("UI build contains a non-regular file");
        }
    }
    Ok(())
}
pub fn install(store: &Store, from: &Path) -> Result<Value> {
    anyhow::ensure!(
        from.join("index.html").is_file(),
        "Build frontend first: npm --prefix web ci && npm --prefix web run build"
    );
    let _guard = store.lock()?;
    let id = uuid::Uuid::new_v4();
    let staging = store.root.join(format!("ui-staging-{id}"));
    copy_assets(from, &staging)?;
    let dest = store.root.join("ui");
    let backup = store.root.join(format!("ui-backup-{id}"));
    if dest.exists() {
        fs::rename(&dest, &backup)?;
    }
    if let Err(error) = fs::rename(&staging, &dest) {
        if backup.exists() {
            let _ = fs::rename(&backup, &dest);
        }
        return Err(error)
            .context("UI installation failed; previous assets restored where possible");
    }
    Ok(
        json!({"ui":dest,"start":"ogh serve","previous_ui_backup":if backup.exists(){Some(backup)}else{None}}),
    )
}
pub fn web_dir(store: &Store, explicit: Option<PathBuf>) -> PathBuf {
    explicit.unwrap_or_else(|| store.root.join("ui"))
}

pub fn paths(store: &Store) -> Value {
    json!({"data_dir":store.root,"installed_ui":store.root.join("ui"),"outbox_database":store.root.join("outbox.sqlite3"),"cli":std::env::current_exe().ok()})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_works_outside_source_and_keeps_previous() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("build");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("index.html"), "v1").unwrap();
        let store = Store::new(tmp.path().join("data")).unwrap();
        install(&store, &source).unwrap();
        assert_eq!(web_dir(&store, None), store.root.join("ui"));
        fs::write(source.join("index.html"), "v2").unwrap();
        let result = install(&store, &source).unwrap();
        assert_eq!(
            fs::read_to_string(store.root.join("ui/index.html")).unwrap(),
            "v2"
        );
        assert_eq!(
            fs::read_to_string(
                PathBuf::from(result["previous_ui_backup"].as_str().unwrap()).join("index.html")
            )
            .unwrap(),
            "v1"
        );
    }
}
