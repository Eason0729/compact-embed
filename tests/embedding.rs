#![cfg(all(feature = "derive", feature = "decode"))]

use compact_embed::{Embed, Encoding, MimeType};
use std::{borrow::Cow, fs, io::Read, path::Path};

#[derive(Embed)]
#[embed(folder = "tests/assets", dynamic = false)]
struct Assets;

#[derive(Embed)]
#[embed(folder = "tests/assets", dynamic = false, compression = false)]
struct OriginalAssets;

#[derive(Embed)]
#[embed(folder = "tests/does-not-exist", dynamic = false)]
struct MissingAssets;

#[derive(Embed)]
#[embed(folder = "tests/assets")]
struct DevelopmentAssets;

#[test]
fn default_mode_uses_disk_in_debug_and_embedded_bytes_in_release() {
    let asset = DevelopmentAssets::get("code.js").unwrap().unwrap();
    assert_eq!(asset.mime_type, MimeType::JavaScript);
    if cfg!(debug_assertions) {
        assert!(matches!(asset.data, Cow::Owned(_)));
        assert_eq!(asset.encoding, Encoding::Identity);
    } else {
        assert!(matches!(asset.data, Cow::Borrowed(_)));
        assert_eq!(asset.encoding, Encoding::Zstd);
    }
    assert_eq!(
        asset.decoded().unwrap().as_ref(),
        include_bytes!("assets/code.js")
    );
    assert_eq!(
        DevelopmentAssets::get("style.CSS")
            .unwrap()
            .unwrap()
            .mime_type,
        MimeType::Css
    );
}

#[test]
fn derive_serves_one_representation_and_decodes_exactly() {
    let code = Assets::get("code.js").unwrap().unwrap();
    assert_eq!(code.encoding, Encoding::Zstd);
    assert!(matches!(code.data, Cow::Borrowed(_)));
    assert!((code.data.len() as u64) < code.original_size);
    assert_eq!(code.mime_type, MimeType::JavaScript);
    assert_eq!(code.mime_type.as_str(), "text/javascript; charset=utf-8");
    assert_eq!(
        code.decoded().unwrap().as_ref(),
        include_bytes!("assets/code.js")
    );
    let mut streamed = Vec::new();
    code.reader().unwrap().read_to_end(&mut streamed).unwrap();
    assert_eq!(streamed, include_bytes!("assets/code.js"));
    let tiny = Assets::get("tiny.txt").unwrap().unwrap();
    assert_eq!(tiny.mime_type, MimeType::PlainText);
    assert_eq!(tiny.encoding, Encoding::Identity);
    assert_eq!(tiny.data.as_ref(), b"x");
    assert_eq!(
        Assets::get("style.CSS").unwrap().unwrap().mime_type,
        MimeType::Css
    );
    assert!(matches!(tiny.decoded().unwrap(), Cow::Borrowed(_)));
    assert_eq!(
        Assets::get("empty").unwrap().unwrap().encoding,
        Encoding::Identity
    );
    assert_eq!(
        Assets::get("empty").unwrap().unwrap().mime_type,
        MimeType::OctetStream
    );
    assert_eq!(
        OriginalAssets::get("code.js").unwrap().unwrap().encoding,
        Encoding::Identity
    );
}

#[test]
fn missing_and_unsafe_paths_do_not_resolve() {
    for path in [
        "",
        "/tiny.txt",
        "../tiny.txt",
        "sub/../../tiny.txt",
        "sub\\tiny.txt",
        "unknown",
    ] {
        assert!(Assets::get(path).unwrap().is_none(), "{path}");
    }
    assert!(MissingAssets::get("index.html").unwrap().is_none());
}

#[test]
fn decoded_length_is_checked() {
    let mut asset = Assets::get("code.js").unwrap().unwrap();
    asset.original_size = 1;
    assert!(asset.decoded().is_err());
}

#[test]
fn dynamic_loading_handles_missing_and_changed_sources() {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-data");
    fs::create_dir_all(&parent).unwrap();
    let root = tempfile::tempdir_in(parent).unwrap();
    let root_name = root.path().to_str().unwrap();
    assert!(
        compact_embed::dynamic_asset(root_name, "index.html", 100)
            .unwrap()
            .is_none()
    );
    fs::write(root.path().join("index.html"), b"first").unwrap();
    assert_eq!(
        compact_embed::dynamic_asset(root_name, "index.html", 100)
            .unwrap()
            .unwrap()
            .data
            .as_ref(),
        b"first"
    );
    fs::write(root.path().join("index.html"), b"updated").unwrap();
    assert_eq!(
        compact_embed::dynamic_asset(root_name, "index.html", 100)
            .unwrap()
            .unwrap()
            .data
            .as_ref(),
        b"updated"
    );
    assert!(compact_embed::dynamic_asset(root_name, "index.html", 2).is_err());
    assert!(
        compact_embed::dynamic_asset(root_name, "../secret", 100)
            .unwrap()
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn dynamic_loading_rejects_symlinks_created_after_compilation() {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-data");
    fs::create_dir_all(&parent).unwrap();
    let root = tempfile::tempdir_in(parent).unwrap();
    std::os::unix::fs::symlink(".", root.path().join("cycle")).unwrap();
    assert!(
        compact_embed::dynamic_asset(root.path().to_str().unwrap(), "cycle/source", 100).is_err()
    );
}
