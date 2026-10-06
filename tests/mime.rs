use compact_embed::MimeType;

#[test]
fn mime_type_uses_one_byte_and_supports_const_metadata() {
    const MIME: MimeType = MimeType::from_extension("js");
    const HEADER: &str = MIME.as_str();
    assert_eq!(std::mem::size_of::<MimeType>(), 1);
    assert_eq!(MIME, MimeType::JavaScript);
    assert_eq!(HEADER, "text/javascript; charset=utf-8");
    assert_eq!(MIME.as_ref(), HEADER);
    assert_eq!(MIME.to_string(), HEADER);
}

#[test]
fn filename_mime_values_and_aliases_are_preserved() {
    for (extensions, header) in [
        ("html htm", "text/html; charset=utf-8"),
        ("css", "text/css; charset=utf-8"),
        ("js mjs", "text/javascript; charset=utf-8"),
        ("json map", "application/json"),
        ("svg", "image/svg+xml"),
        ("png", "image/png"),
        ("jpg jpeg", "image/jpeg"),
        ("webp", "image/webp"),
        ("gif", "image/gif"),
        ("ico", "image/vnd.microsoft.icon"),
        ("woff", "font/woff"),
        ("woff2", "font/woff2"),
        ("ttf", "font/ttf"),
        ("otf", "font/otf"),
        ("wasm", "application/wasm"),
        ("webmanifest", "application/manifest+json"),
        ("txt j2", "text/plain; charset=utf-8"),
        ("xml", "application/xml"),
        ("pdf", "application/pdf"),
        ("toml", "application/toml"),
    ] {
        for extension in extensions.split_whitespace() {
            for extension in [extension.to_owned(), extension.to_ascii_uppercase()] {
                let path = format!("nested.dir/asset.{extension}");
                assert_eq!(compact_embed::mime_type(&path).as_str(), header, "{path}");
            }
        }
    }
    for path in [
        "asset",
        "asset.unknown",
        "asset.",
        ".html",
        "nested.css/asset",
    ] {
        assert_eq!(
            compact_embed::mime_type(path),
            MimeType::OctetStream,
            "{path}"
        );
    }
}
