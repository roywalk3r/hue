use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    let commit = git_output(&["rev-parse", "--short=12", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    let dirty = git_output(&["status", "--porcelain"])
        .is_some_and(|output| !output.is_empty());
    println!("cargo:rustc-env=HUE_GIT_SHA={commit}");
    println!("cargo:rustc-env=HUE_GIT_DIRTY={dirty}");
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();
    if value.is_empty() { None } else { Some(value) }
}
