//! Expose a git-derived version string at build time (mirrors
//! `scripts/sync_rust_version.py`), falling back to the Cargo version.

use std::process::Command;

fn main() {
    if let Some(v) = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
    {
        println!("cargo:rustc-env=BIGTUBE_GIT_VERSION={v}");
    }
    println!("cargo:rerun-if-changed=../../../.git/HEAD");
}
