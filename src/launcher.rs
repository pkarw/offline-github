//! Explicit local desktop launches. Paths come only from this cache's Git worktrees.
use crate::{process, store::Store};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn executable(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|p| p.join(name))
        .find(|p| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                p.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            }
            #[cfg(not(unix))]
            {
                p.is_file()
            }
        })
        .and_then(|p| p.canonicalize().ok())
}
fn terminal() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return Path::new("/System/Applications/Utilities/Terminal.app")
            .exists()
            .then(|| executable("osascript"))
            .flatten();
    }
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return None;
    }
    ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"]
        .into_iter()
        .find_map(executable)
}
fn planned(store: &Store, repo: &str) -> Result<PathBuf> {
    crate::store::validate_repo(repo)?;
    Ok(crate::store::absolute(&store.root)?
        .join("workspaces")
        .join(repo))
}
fn workspaces(store: &Store, repo: &str) -> Result<Vec<PathBuf>> {
    let mirror = store.mirror(repo)?;
    if !mirror.exists() {
        return Ok(vec![]);
    }
    let listing = process::git(&mirror, &["worktree", "list", "--porcelain", "-z"])?;
    let mut paths = Vec::new();
    for record in listing.split("\0\0") {
        if record.split('\0').any(|v| v == "bare") {
            continue;
        }
        if let Some(path) = record
            .strip_prefix("worktree ")
            .and_then(|v| v.split('\0').next())
        {
            if let Ok(path) = Path::new(path).canonicalize() {
                paths.push(path);
            }
        }
    }
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.canonicalize().ok());
    paths.sort_by_key(|p| {
        (
            Some(p) != cwd.as_ref(),
            !p.join(".ogh-workspace.json").exists(),
            p.clone(),
        )
    });
    Ok(paths)
}
pub fn removal_preview(store: &Store, repo: &str) -> Result<Value> {
    store.load(repo)?;
    let report = crate::outbox::report(store, Some(repo), None)?;
    let outgoing = report["events"]
        .as_array()
        .map(|events| {
            events
                .iter()
                .filter(|e| !matches!(e["status"].as_str(), Some("applied" | "cancelled")))
                .count()
        })
        .unwrap_or(0);
    let mut warnings = vec![];
    match workspaces(store, repo) {
        Ok(paths) => {
            for path in paths {
                match process::run(
                    "git",
                    &["-C", &path.to_string_lossy(), "status", "--porcelain"],
                    30,
                ) {
                    Ok(status) if !status.trim().is_empty() => {
                        warnings.push(format!("Uncommitted files in {}", path.display()))
                    }
                    Err(e) => warnings.push(format!("Could not inspect {}: {e}", path.display())),
                    _ => {}
                }
            }
        }
        Err(e) => warnings.push(format!("Could not inspect worktrees: {e}")),
    }
    if store.mirror(repo)?.exists() {
        match process::git(
            &store.mirror(repo)?,
            &["rev-list", "--count", "--branches", "--not", "--remotes"],
        ) {
            Ok(count) if count.trim() != "0" => warnings.push(format!(
                "{} local commits are absent from cached remote branches (last pull only)",
                count.trim()
            )),
            Err(e) => warnings.push(format!("Could not inspect local commits: {e}")),
            _ => {}
        }
    }
    Ok(
        json!({"outgoing_count":outgoing,"warnings":warnings,"retained_path":store.dir(repo)?,"retains_all_data":true}),
    )
}
pub fn context(store: &Store, repo: &str) -> Result<Value> {
    store.load(repo)?;
    let paths = workspaces(store, repo)?;
    let path = paths.first().cloned().unwrap_or(planned(store, repo)?);
    let desktop = terminal().is_some();
    let synced = store.mirror(repo)?.exists();
    let tools: Vec<_> = [
        ("claude", "Claude Code", Some("claude")),
        ("codex", "Codex CLI", Some("codex")),
        ("ogh", "ogh CLI", Some("ogh")),
        ("terminal", "Terminal", None),
    ]
    .into_iter()
    .map(|(id, label, binary)| {
        let installed = binary.is_none_or(|b| executable(b).is_some());
        let reason = if !synced {
            "Sync this repository first"
        } else if !desktop {
            "No desktop terminal available where ogh is running"
        } else if !installed {
            "Command not found on the server PATH"
        } else {
            ""
        };
        json!({"id":id,"label":label,"available":reason.is_empty(),"reason":reason})
    })
    .collect();
    Ok(
        json!({"separate_pull_push":true,"path":path,"workspaces":paths,"checkout_exists":!paths.is_empty(),"cache_path":store.mirror(repo)?,"tools":tools}),
    )
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
fn shell_command(store: &Store, repo: &str, path: &Path, binary: Option<&Path>) -> Result<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
    let mut command = format!(
        "cd {} || exit; export OGH_REPO={} OGH_DATA_DIR={}; ",
        quote(path.to_str().context("Non UTF-8 checkout path")?),
        quote(repo),
        quote(
            crate::store::absolute(&store.root)?
                .to_str()
                .context("Non UTF-8 cache path")?
        )
    );
    if let Some(binary) = binary {
        command.push_str(&quote(binary.to_str().context("Non UTF-8 tool path")?));
        if binary.file_name().is_some_and(|n| n == "ogh") {
            command.push_str(" --help");
        }
        command.push_str("; ");
    }
    command.push_str(&format!("exec {} -i", quote(&shell)));
    Ok(command)
}
pub fn open(store: &Store, repo: &str, tool: &str, selected: Option<&str>) -> Result<Value> {
    let binary = match tool {
        "claude" | "codex" | "ogh" => {
            Some(executable(tool).context("Tool is not installed on the server PATH")?)
        }
        "terminal" => None,
        _ => bail!("Unknown launch target"),
    };
    let terminal = terminal().context("No desktop terminal available; run the agent in your local project shell (ogh-work or ogh-sandbox)")?;
    let snapshot = store.load(repo)?;
    let paths = workspaces(store, repo)?;
    let path = match selected {
        Some(p) => {
            let p = PathBuf::from(p);
            anyhow::ensure!(
                paths.contains(&p) || (paths.is_empty() && p == planned(store, repo)?),
                "Path must be a worktree of the selected repository"
            );
            p
        }
        None => paths.first().cloned().unwrap_or(planned(store, repo)?),
    };
    if paths.is_empty() {
        anyhow::ensure!(
            !path.exists(),
            "Checkout destination already exists; preserving it"
        );
        let sha = process::git(
            &store.mirror(repo)?,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("origin/{}^{{commit}}", snapshot.default_branch),
            ],
        )?;
        std::fs::create_dir_all(path.parent().context("Missing parent")?)?;
        process::git(
            &store.mirror(repo)?,
            &[
                "worktree",
                "add",
                "-b",
                &format!("offline/work-{}", uuid::Uuid::new_v4()),
                path.to_str().context("Invalid path")?,
                sha.trim(),
            ],
        )?;
    }
    let script = shell_command(store, repo, &path, binary.as_deref())?;
    if cfg!(target_os = "macos") {
        process::run(terminal.to_str().context("Invalid terminal")?, &["-e", "on run argv\ntell application \"Terminal\"\nactivate\ndo script (item 1 of argv)\nend tell\nend run", &script], 15)?;
    } else {
        let flag = if terminal.file_name().is_some_and(|n| n == "gnome-terminal") {
            "--"
        } else {
            "-e"
        };
        let mut child = Command::new(terminal)
            .args([flag, "/bin/sh", "-c", &script])
            .current_dir(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    Ok(json!({"path":path,"tool":tool,"message":"Launch requested in a local terminal"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_paths_are_quoted_and_context_is_set() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf()).unwrap();
        let text = shell_command(
            &store,
            "owner/repo",
            Path::new("/tmp/a ' $(touch nope)"),
            Some(Path::new("/tmp/tool name")),
        )
        .unwrap();
        assert!(text.contains("'\"'\"'"));
        assert!(text.contains("export OGH_REPO='owner/repo'"));
        assert!(text.contains("'/tmp/tool name'"));
        assert!(open(&store, "owner/repo", "arbitrary-command", None).is_err());
    }
}
