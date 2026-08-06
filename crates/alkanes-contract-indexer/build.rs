use std::{env, path::Path, process::Command};

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn main() {
    println!("cargo:rerun-if-env-changed=ALKANES_PRODUCER_REVISION");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");

    let revision = env::var("ALKANES_PRODUCER_REVISION").ok().or_else(|| {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
        let output = Command::new("git")
            .arg("-C")
            .arg(Path::new(&manifest_dir))
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout)
            .ok()
            .map(|value| value.trim().to_owned())
    });

    let revision = revision.unwrap_or_else(|| {
        panic!(
            "a pinned producer revision is required: build from Git or set ALKANES_PRODUCER_REVISION"
        )
    });
    if !valid_revision(&revision) {
        panic!("ALKANES_PRODUCER_REVISION must be 40 or 64 lowercase hexadecimal characters");
    }
    println!("cargo:rustc-env=ALKANES_PRODUCER_REVISION={revision}");
}
