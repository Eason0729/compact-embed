# compact-embed

`compact-embed` embeds one payload per asset: Zstd when strictly smaller, otherwise original bytes. The encoder runs in the derive macro on the build host. Applications link only the decoder, which can share the same Zstd library as other dependencies.

## Usage

```toml
[dependencies]
compact-embed = { path = "../compact-embed" }

[build-dependencies]
compact-embed = { path = "../compact-embed", default-features = false }
```

```rust
use compact_embed::Embed;

#[derive(Embed)]
#[embed(folder = "public")]
struct Assets;

fn main() -> std::io::Result<()> {
    if let Some(asset) = Assets::get("app.js")? {
        // `data` contains the stored representation, described by `encoding`.
        let decoded = asset.decoded()?;
        assert_eq!(decoded.len() as u64, asset.original_size);
    }
    Ok(())
}
```

Add `build.rs` so Cargo tracks asset additions, modifications, deletions, and a missing directory appearing later:

```rust,ignore
fn main() {
    compact_embed::track("public");
}
```

Stable procedural macros cannot directly register directory dependencies with Cargo. Calling `track` is required for reliable rebuilds. Missing paths trigger rebuilds until they exist; the helper does not recursively track their parent directory.

Paths are relative to the consuming package's `Cargo.toml`. Asset lookup uses `/` separators, rejects traversal and absolute paths, and returns `Ok(None)` for unknown assets. Keep the dependency named `compact-embed`; generated code refers to `::compact_embed`.

## Development and production

With debug assertions enabled, `get` reads current assets from disk and returns original bytes. Rebuilding the frontend does not require restarting the backend. A missing directory produces an empty asset set, allowing API development with a separate frontend dev server. With debug assertions disabled, `get` returns borrowed embedded bytes and needs no asset directory at runtime.

Set `dynamic = false` to embed assets in every profile. Set `allow_missing = false` when assets must exist, and `compression = false` for assets such as prompt templates that should always retain original bytes.

The macro validates the directory in every profile. Only the embedded branch performs compression; debug builds using disk assets skip compression. Successful compilation assumes the directory remains stable during inspection and reading. This is a build safeguard, not a sandbox for an adversarial filesystem changing concurrently.

## Limits

The macro inspects metadata before reading payloads. Each limit is configurable with `#[embed(...)]`; zero limits are invalid.

| Option | Default | Meaning |
| --- | ---: | --- |
| `max_files` | 2,048 | Regular files |
| `max_entries` | 8,192 | Files and directories visited |
| `max_file_bytes` | 8,388,608 | Original bytes per asset |
| `max_total_bytes` | 33,554,432 | Total original asset bytes |
| `max_depth` | 64 | Nested directory levels |
| `quality` | 19 | Zstd level, 0–22 |
| `window` | 20 | Maximum back-reference distance, `2^window` bytes; 10–24 |

All source sizes count, even when compression would make the payload small. Reads also enforce the inspected length to catch files growing during compilation. The limits bound input and traversal; compiler token expansion and codec working memory add overhead. Increasing quality or limits increases build memory and time.

The macro rejects symlinks to files or directories, dangling links, cycles, a symlink used as the root, and special entries such as sockets. It rejects non-UTF-8 paths and filenames containing backslashes. Compression round trips are validated before generating Rust byte literals. Generated data contains only the selected representation, sorted by path for binary-search lookup.

## Reading assets

`Asset::data` is the stored payload, `encoding` distinguishes `Identity` from `Zstd`, and `original_size` is the decoded byte length. `mime_type` describes the original filename; unknown extensions use `application/octet-stream`.

`reader()` decodes incrementally without retaining the complete output. `decoded()` collects decoded bytes and checks their length against metadata; identity assets return borrowed bytes. When streaming, the caller must validate the final decoded length if needed. The default `decode` feature enables Zstd decoding; disabling it still allows direct delivery of stored bytes.

HTTP negotiation, response headers, backpressure, and decoder concurrency belong to the application. Clients accepting `zstd` can receive stored compressed bytes directly; other clients need a decoded response. This crate does not implement Compression Dictionary Transport.

## Tests

```bash
cargo test --workspace
cargo test --release --test embedding
cargo fmt --all --check
```

Compiler regressions exercise missing directories, symlink rejection, hard limits, and Cargo rebuilds. Tests and child Cargo builds use directories under this checkout's `target`, avoiding RAM-backed temporary directories.
