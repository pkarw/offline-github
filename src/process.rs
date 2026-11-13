use anyhow::{bail, Context, Result};
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use wait_timeout::ChildExt;

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    // The group is created solely for this invocation; never match global process names.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}
pub fn run(program: &str, args: &[&str], seconds: u64) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut command = Command::new(program);
    command
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("Cannot start {program}; install it and retry"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut b = Vec::new();
        let result = stdout.read_to_end(&mut b).map(|_| b);
        let _ = out_tx.send(result);
    });
    std::thread::spawn(move || {
        let mut b = Vec::new();
        let result = stderr.read_to_end(&mut b).map(|_| b);
        let _ = err_tx.send(result);
    });
    let status = match child.wait_timeout(deadline.saturating_duration_since(Instant::now()))? {
        Some(s) => s,
        None => {
            terminate(&mut child);
            bail!("{program} timed out after {seconds}s; retry is safe")
        }
    };
    let output = match out_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(v) => v?,
        Err(_) => {
            terminate(&mut child);
            bail!("{program} output timed out after {seconds}s")
        }
    };
    let errors = match err_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(v) => v?,
        Err(_) => {
            terminate(&mut child);
            bail!("{program} output timed out after {seconds}s")
        }
    };
    if !status.success() {
        bail!("{program} failed: {}", String::from_utf8_lossy(&errors));
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}
pub fn git(mirror: &std::path::Path, args: &[&str]) -> Result<String> {
    let path = mirror.to_str().context("Non UTF-8 path")?;
    let mut a = vec!["--git-dir", path];
    a.extend(args);
    run("git", &a, 600)
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn deadline_includes_descendant_pipes() {
        let start = Instant::now();
        assert!(run("sh", &["-c", "sleep 3 & exit 0"], 1).is_err());
        assert!(start.elapsed() < Duration::from_millis(1800));
    }
}
