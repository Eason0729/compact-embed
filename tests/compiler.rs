#![cfg(feature = "derive")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn check(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO"))
        .args(["check", "--offline", "-j", "1", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/compiler-tests"),
        )
        .output()
        .unwrap()
}

#[test]
fn compiler_accepts_missing_assets_and_rejects_limits_and_symlinks() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let parent = manifest.join("target/test-data");
    fs::create_dir_all(&parent).unwrap();
    let root = tempfile::tempdir_in(parent).unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        format!(
            r#"
[package]
name = "embed-compiler-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
compact-embed = {{ path = {:?}, default-features = false, features = ["derive"] }}
"#,
            manifest
        ),
    )
    .unwrap();
    let source = |attributes: &str| {
        fs::write(root.path().join("src/main.rs"), format!("use compact_embed::Embed;\n#[derive(Embed)]\n#[embed(folder=\"assets\", {attributes})]\nstruct Assets;\nfn main() {{ assert!(Assets::get(\"missing\").unwrap().is_none()); }}\n")).unwrap();
    };
    source("dynamic=false");
    let output = check(root.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    source("allow_missing=false");
    let output = check(root.path());
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("compact-embed:"));
    let assets = root.path().join("assets");
    fs::create_dir(&assets).unwrap();
    fs::write(assets.join("a"), b"1234").unwrap();
    fs::write(assets.join("b"), b"5678").unwrap();
    for attributes in ["max_files=1", "max_file_bytes=3", "max_total_bytes=7"] {
        source(attributes);
        let output = check(root.path());
        assert!(!output.status.success(), "{attributes}");
        let expected = attributes.split('=').next().unwrap();
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(".", assets.join("loop")).unwrap();
        source("dynamic=true");
        let output = check(root.path());
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("symlink is not allowed"));
    }
}

#[test]
fn cargo_rebuilds_when_a_missing_directory_appears_or_an_asset_changes() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let parent = manifest.join("target/test-data");
    fs::create_dir_all(&parent).unwrap();
    let root = tempfile::tempdir_in(parent).unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        format!(
            r#"
[package]
name = "embed-rebuild-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
compact-embed = {{ path = {:?}, default-features = false, features = ["derive"] }}
[build-dependencies]
compact-embed = {{ path = {:?}, default-features = false }}
"#,
            manifest, manifest
        ),
    )
    .unwrap();
    fs::write(
        root.path().join("build.rs"),
        "fn main() { compact_embed::track(\"assets\"); }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/main.rs"),
        r#"
use compact_embed::Embed;
#[derive(Embed)]
#[embed(folder="assets", dynamic=false, compression=false)]
struct Assets;
fn main() {
    match Assets::get("new.txt").unwrap() {
        Some(asset) => print!("{}", String::from_utf8_lossy(&asset.data)),
        None => print!("missing"),
    }
}
"#,
    )
    .unwrap();
    let run = || {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "-j", "1", "--quiet", "--manifest-path"])
            .arg(root.path().join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", manifest.join("target/rebuild-tests"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    assert_eq!(run(), "missing");
    fs::create_dir(root.path().join("assets")).unwrap();
    fs::write(root.path().join("assets/new.txt"), b"first").unwrap();
    assert_eq!(run(), "first");
    fs::write(root.path().join("assets/new.txt"), b"changed").unwrap();
    assert_eq!(run(), "changed");
    fs::remove_file(root.path().join("assets/new.txt")).unwrap();
    assert_eq!(run(), "missing");
}
