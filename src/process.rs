use anyhow::{bail, Context, Result};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;

pub fn run(program: &str, args: &[&str], seconds: u64) -> Result<String> {
    let mut child = Command::new(program)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Cannot start {program}; install it and retry"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let status = match child.wait_timeout(Duration::from_secs(seconds))? {
        Some(s) => s,
        None => {
            child.kill()?;
            child.wait()?;
            bail!("{program} timed out after {seconds}s; retry is safe")
        }
    };
    let output = out.join().unwrap()?;
    let errors = err.join().unwrap()?;
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
