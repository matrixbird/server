use std::process::Command;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|output| String::from_utf8(output.stdout).unwrap_or_default().trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);

    println!("cargo:rerun-if-changed=.git/HEAD");
}
