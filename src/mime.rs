#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MimeType {
    OctetStream,
    Html,
    Css,
    JavaScript,
    Json,
    Svg,
    Png,
    Jpeg,
    Webp,
    Gif,
    Icon,
    Woff,
    Woff2,
    Ttf,
    Otf,
    Wasm,
    WebManifest,
    PlainText,
    Xml,
    Pdf,
}

impl MimeType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OctetStream => "application/octet-stream",
            Self::Html => "text/html; charset=utf-8",
            Self::Css => "text/css; charset=utf-8",
            Self::JavaScript => "text/javascript; charset=utf-8",
            Self::Json => "application/json",
            Self::Svg => "image/svg+xml",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
            Self::Icon => "image/vnd.microsoft.icon",
            Self::Woff => "font/woff",
            Self::Woff2 => "font/woff2",
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
            Self::Wasm => "application/wasm",
            Self::WebManifest => "application/manifest+json",
            Self::PlainText => "text/plain; charset=utf-8",
            Self::Xml => "application/xml",
            Self::Pdf => "application/pdf",
        }
    }

    /// Maps a lowercase extension without a leading dot; unknown extensions use `OctetStream`.
    #[doc(hidden)]
    pub const fn from_extension(extension: &str) -> Self {
        // Byte-slice patterns allow the generated asset table to evaluate this at compile time.
        match extension.as_bytes() {
            b"html" | b"htm" => Self::Html,
            b"css" => Self::Css,
            b"js" | b"mjs" => Self::JavaScript,
            b"json" | b"map" => Self::Json,
            b"svg" => Self::Svg,
            b"png" => Self::Png,
            b"jpg" | b"jpeg" => Self::Jpeg,
            b"webp" => Self::Webp,
            b"gif" => Self::Gif,
            b"ico" => Self::Icon,
            b"woff" => Self::Woff,
            b"woff2" => Self::Woff2,
            b"ttf" => Self::Ttf,
            b"otf" => Self::Otf,
            b"wasm" => Self::Wasm,
            b"webmanifest" => Self::WebManifest,
            b"txt" | b"j2" => Self::PlainText,
            b"xml" => Self::Xml,
            b"pdf" => Self::Pdf,
            _ => Self::OctetStream,
        }
    }
}

impl AsRef<str> for MimeType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for MimeType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
