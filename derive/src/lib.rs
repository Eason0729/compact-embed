#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use quote::quote;
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};
use syn::{DeriveInput, LitBool, LitInt, LitStr};

#[proc_macro_derive(Embed, attributes(embed))]
pub fn derive_embed(input: TokenStream) -> TokenStream {
    match syn::parse(input).and_then(expand) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[derive(Debug)]
struct Options {
    folder: String,
    allow_missing: bool,
    dynamic: bool,
    compression: bool,
    max_files: usize,
    max_entries: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
    max_depth: usize,
    quality: u32,
    window: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            folder: String::new(),
            allow_missing: true,
            dynamic: true,
            compression: true,
            max_files: 2048,
            max_entries: 8192,
            max_file_bytes: 8 * 1024 * 1024,
            max_total_bytes: 32 * 1024 * 1024,
            max_depth: 64,
            quality: 19,
            window: 20,
        }
    }
}

fn options(input: &DeriveInput) -> syn::Result<Options> {
    let mut options = Options::default();
    let mut seen = std::collections::HashSet::new();
    for attribute in input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("embed"))
    {
        attribute.parse_nested_meta(|meta| {
            let name = meta
                .path
                .get_ident()
                .ok_or_else(|| meta.error("expected an embed option"))?
                .to_string();
            if !seen.insert(name.clone()) {
                return Err(meta.error("duplicate embed option"));
            }
            let value = meta.value()?;
            match name.as_str() {
                "folder" => options.folder = value.parse::<LitStr>()?.value(),
                "allow_missing" => options.allow_missing = value.parse::<LitBool>()?.value,
                "dynamic" => options.dynamic = value.parse::<LitBool>()?.value,
                "compression" => options.compression = value.parse::<LitBool>()?.value,
                "max_files" => options.max_files = value.parse::<LitInt>()?.base10_parse()?,
                "max_entries" => options.max_entries = value.parse::<LitInt>()?.base10_parse()?,
                "max_file_bytes" => {
                    options.max_file_bytes = value.parse::<LitInt>()?.base10_parse()?
                }
                "max_total_bytes" => {
                    options.max_total_bytes = value.parse::<LitInt>()?.base10_parse()?
                }
                "max_depth" => options.max_depth = value.parse::<LitInt>()?.base10_parse()?,
                "quality" => options.quality = value.parse::<LitInt>()?.base10_parse()?,
                "window" => options.window = value.parse::<LitInt>()?.base10_parse()?,
                _ => return Err(meta.error("unknown embed option")),
            }
            Ok(())
        })?;
    }
    if options.folder.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "embed requires folder = \"...\"",
        ));
    }
    if options.max_files == 0
        || options.max_entries == 0
        || options.max_file_bytes == 0
        || options.max_total_bytes == 0
        || options.max_depth == 0
    {
        return Err(syn::Error::new_spanned(
            input,
            "embed limits must be greater than zero",
        ));
    }
    if options.quality > 22 || !(10..=24).contains(&options.window) {
        return Err(syn::Error::new_spanned(
            input,
            "quality must be 0..=22 and window must be 10..=24",
        ));
    }
    Ok(options)
}

#[derive(Debug)]
struct Source {
    path: PathBuf,
    name: String,
    size: u64,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn inspect(root: &Path, options: &Options) -> io::Result<Vec<Source>> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound && options.allow_missing => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        return Err(invalid(format!(
            "symlink is not allowed: {}",
            root.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(invalid("embed folder must be a directory"));
    }
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut sources = Vec::new();
    let mut entries = 0usize;
    let mut total = 0u64;
    while let Some((directory, depth)) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            entries += 1;
            if entries > options.max_entries {
                return Err(invalid("embed exceeds max_entries"));
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(invalid(format!(
                    "symlink is not allowed: {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                if depth >= options.max_depth {
                    return Err(invalid("embed exceeds max_depth"));
                }
                pending.push((path, depth + 1));
            } else if metadata.is_file() {
                if sources.len() >= options.max_files {
                    return Err(invalid("embed exceeds max_files"));
                }
                if metadata.len() > options.max_file_bytes {
                    return Err(invalid(format!(
                        "{} exceeds max_file_bytes",
                        path.display()
                    )));
                }
                total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| invalid("embed total size overflow"))?;
                if total > options.max_total_bytes {
                    return Err(invalid("embed exceeds max_total_bytes"));
                }
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| invalid(error.to_string()))?;
                let name = relative
                    .components()
                    .map(|component| {
                        let component = component
                            .as_os_str()
                            .to_str()
                            .ok_or_else(|| invalid("asset paths must be UTF-8"))?;
                        if component.contains('\\') {
                            return Err(invalid("asset paths cannot contain backslashes"));
                        }
                        Ok(component)
                    })
                    .collect::<io::Result<Vec<_>>>()?
                    .join("/");
                sources.push(Source {
                    path,
                    name,
                    size: metadata.len(),
                });
            } else {
                return Err(invalid(format!(
                    "only regular files and directories are allowed: {}",
                    path.display()
                )));
            }
        }
    }
    sources.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(sources)
}

fn payload(source: &Source, options: &Options) -> io::Result<(Vec<u8>, bool)> {
    if fs::symlink_metadata(&source.path)?.file_type().is_symlink() {
        return Err(invalid("symlink is not allowed"));
    }
    let mut original = Vec::new();
    // Bound reads too: a file can grow after the metadata pass.
    fs::File::open(&source.path)?
        .take(source.size.saturating_add(1))
        .read_to_end(&mut original)?;
    if original.len() as u64 != source.size {
        return Err(invalid(
            "asset changed while embedding; rebuild with a stable asset directory",
        ));
    }
    if !options.compression || original.is_empty() {
        return Ok((original, false));
    }
    let mut compressor = zstd::bulk::Compressor::new(options.quality as i32)?;
    compressor.window_log(options.window)?;
    let compressed = compressor.compress(&original)?;
    if compressed.len() >= original.len() {
        return Ok((original, false));
    }
    let decoded = zstd::bulk::decompress(&compressed, original.len())?;
    if decoded != original {
        return Err(invalid("compression round-trip validation failed"));
    }
    Ok((compressed, true))
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    if !matches!(input.data, syn::Data::Struct(_)) || !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input,
            "Embed requires a struct without generic parameters",
        ));
    }
    let options = options(&input)?;
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .ok_or_else(|| syn::Error::new_spanned(&input, "CARGO_MANIFEST_DIR is missing"))?;
    let root = Path::new(&manifest).join(&options.folder);
    let failure = |error: io::Error| {
        syn::Error::new_spanned(
            &input,
            format!("compact-embed: {}: {error}", root.display()),
        )
    };
    let _sources = inspect(&root, &options).map_err(failure)?;
    let root = root
        .to_str()
        .ok_or_else(|| syn::Error::new_spanned(&input, "embed folder must be UTF-8"))?;
    let ident = &input.ident;
    let maximum_file_bytes = options.max_file_bytes;
    let dynamic = if options.dynamic {
        quote! {
            #[cfg(debug_assertions)]
            { return ::compact_embed::dynamic_asset(#root, path, #maximum_file_bytes); }
        }
    } else {
        quote!()
    };
    let embedded = quote! {
        {
            if !::compact_embed::valid_path(path) { return Ok(None); }
            static ASSETS: &[(&str, &[u8], ::compact_embed::Encoding, u64)] = ::compact_embed::__embed_data!(#input);
            let Ok(index) = ASSETS.binary_search_by(|asset| asset.0.cmp(path)) else { return Ok(None); };
            let (_, bytes, encoding, original_size) = ASSETS[index];
            Ok(Some(::compact_embed::Asset {
                data: ::std::borrow::Cow::Borrowed(bytes), encoding, original_size,
                mime_type: ::compact_embed::mime_type(path),
            }))
        }
    };
    let embedded = if options.dynamic {
        quote!(#[cfg(not(debug_assertions))] #embedded)
    } else {
        embedded
    };
    Ok(quote! {
        impl ::compact_embed::Embed for #ident {
            fn get(path: &str) -> ::std::io::Result<Option<::compact_embed::Asset>> {
                #dynamic
                #embedded
            }
        }
    })
}

#[proc_macro]
pub fn __embed_data(input: TokenStream) -> TokenStream {
    match syn::parse(input).and_then(embed_data) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn embed_data(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let options = options(&input)?;
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .ok_or_else(|| syn::Error::new_spanned(&input, "CARGO_MANIFEST_DIR is missing"))?;
    let root = Path::new(&manifest).join(&options.folder);
    let failure = |error: io::Error| {
        syn::Error::new_spanned(
            &input,
            format!("compact-embed: {}: {error}", root.display()),
        )
    };
    let sources = inspect(&root, &options).map_err(failure)?;
    let mut assets = Vec::new();
    for source in sources {
        let (bytes, compressed) = payload(&source, &options).map_err(failure)?;
        let bytes = proc_macro2::Literal::byte_string(&bytes);
        let name = source.name;
        let size = source.size;
        let encoding = if compressed {
            quote!(::compact_embed::Encoding::Zstd)
        } else {
            quote!(::compact_embed::Encoding::Identity)
        };
        assets.push(quote!((#name, #bytes as &'static [u8], #encoding, #size)));
    }
    Ok(quote!(&[#(#assets),*]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> tempfile::TempDir {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/test-data");
        fs::create_dir_all(&root).unwrap();
        tempfile::tempdir_in(root).unwrap()
    }

    #[test]
    fn missing_directory_is_empty_unless_required() {
        let root = directory();
        let missing = root.path().join("missing");
        assert!(inspect(&missing, &Options::default()).unwrap().is_empty());
        let options = Options {
            allow_missing: false,
            ..Options::default()
        };
        assert_eq!(
            inspect(&missing, &options).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn file_and_total_limits_reject_before_reading() {
        let root = directory();
        let file = fs::File::create(root.path().join("huge")).unwrap();
        file.set_len(9 * 1024 * 1024).unwrap();
        assert!(
            inspect(root.path(), &Options::default())
                .unwrap_err()
                .to_string()
                .contains("max_file_bytes")
        );
        file.set_len(8).unwrap();
        fs::write(root.path().join("second"), b"12345678").unwrap();
        let options = Options {
            max_total_bytes: 15,
            ..Options::default()
        };
        assert!(
            inspect(root.path(), &options)
                .unwrap_err()
                .to_string()
                .contains("max_total_bytes")
        );
    }

    #[test]
    fn file_entry_and_depth_limits_are_independent() {
        let root = directory();
        fs::write(root.path().join("a"), []).unwrap();
        fs::write(root.path().join("b"), []).unwrap();
        let options = Options {
            max_files: 1,
            ..Options::default()
        };
        assert!(
            inspect(root.path(), &options)
                .unwrap_err()
                .to_string()
                .contains("max_files")
        );
        let options = Options {
            max_entries: 1,
            ..Options::default()
        };
        assert!(
            inspect(root.path(), &options)
                .unwrap_err()
                .to_string()
                .contains("max_entries")
        );
        fs::create_dir_all(root.path().join("x/y")).unwrap();
        let options = Options {
            max_depth: 1,
            ..Options::default()
        };
        assert!(
            inspect(root.path(), &options)
                .unwrap_err()
                .to_string()
                .contains("max_depth")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_root_file_directory_dangling_and_cyclic_symlinks() {
        use std::os::unix::fs::symlink;
        for target in [".", "missing", "file"] {
            let root = directory();
            fs::write(root.path().join("file"), b"source").unwrap();
            symlink(target, root.path().join("link")).unwrap();
            assert!(
                inspect(root.path(), &Options::default())
                    .unwrap_err()
                    .to_string()
                    .contains("symlink")
            );
            assert!(
                inspect(&root.path().join("link"), &Options::default())
                    .unwrap_err()
                    .to_string()
                    .contains("symlink")
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_special_entries() {
        let root = directory();
        let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket")).unwrap();
        assert!(
            inspect(root.path(), &Options::default())
                .unwrap_err()
                .to_string()
                .contains("regular files")
        );
    }

    #[test]
    fn sorts_paths_and_stores_compressed_bytes_only_if_smaller() {
        let root = directory();
        fs::write(root.path().join("z"), b"x").unwrap();
        fs::write(
            root.path().join("a"),
            b"const message = 'hello';\n".repeat(1024),
        )
        .unwrap();
        let options = Options::default();
        let sources = inspect(root.path(), &options).unwrap();
        assert_eq!(
            sources
                .iter()
                .map(|source| source.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        let (compressed, selected) = payload(&sources[0], &options).unwrap();
        assert!(selected);
        assert!((compressed.len() as u64) < sources[0].size);
        let (original, selected) = payload(&sources[1], &options).unwrap();
        assert!(!selected);
        assert_eq!(original, b"x");
        let options = Options {
            compression: false,
            ..options
        };
        assert!(!payload(&sources[0], &options).unwrap().1);
    }

    #[test]
    fn detects_files_growing_after_metadata_pass() {
        let root = directory();
        let path = root.path().join("a");
        fs::write(&path, b"old").unwrap();
        let options = Options::default();
        let sources = inspect(root.path(), &options).unwrap();
        fs::write(path, b"new larger source").unwrap();
        assert!(
            payload(&sources[0], &options)
                .unwrap_err()
                .to_string()
                .contains("changed")
        );
    }

    #[test]
    fn invalid_options_produce_macro_diagnostics() {
        for (attributes, expected) in [
            (
                quote!(#[embed(folder="x", max_files=0)]),
                "greater than zero",
            ),
            (quote!(#[embed(folder="x", quality=23)]), "quality"),
            (quote!(#[embed(folder="x", window=25)]), "window"),
            (quote!(#[embed(folder="x", folder="y")]), "duplicate"),
            (quote!(#[embed(folder="x", unknown=true)]), "unknown"),
            (quote!(), "requires folder"),
        ] {
            let input = syn::parse2(quote!(#attributes struct Assets;)).unwrap();
            assert!(options(&input).unwrap_err().to_string().contains(expected));
        }
    }
}
