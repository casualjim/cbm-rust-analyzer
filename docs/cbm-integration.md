# Generic Rust analyzer ABI handoff

`src/capi.rs` is the C ABI source of truth. `include/rust_analyzer_lsp.h` is generated from that Rust source by `cbindgen`; do not edit it by hand.

This crate is a Rust analyzer resolver library. It does not include consumer headers, require consumer checkout paths, or know `codebase-memory-mcp` internals. Consumers map their own graph/extraction structs to the generic ABI at their adapter boundary.

## Regenerate ABI header

```sh
mise run abi:generate-header
```

This runs `RUST_ANALYZER_GENERATE_HEADER=1 cargo build` and rewrites `include/rust_analyzer_lsp.h`. Normal `cargo build` does not rewrite the checked-in header.

## Check ABI header drift

```sh
mise run abi:check-header
```

The check generates a fresh header in `target/tmp/` and fails if it differs from the checked-in artifact. It also rejects CBM-prefixed or legacy public ABI symbols in the generated header.

## Install prefix artifact

```sh
PREFIX=target/prefix mise run prefix:install
```

This creates:

```text
$PREFIX/include/rust_analyzer_lsp.h
$PREFIX/lib/librust_analyzer_lsp.a
$PREFIX/lib/librust_analyzer_lsp.dylib|so
$PREFIX/lib/pkgconfig/rust_analyzer_lsp.pc
```

Consumers can use:

```sh
PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig" pkg-config --cflags --libs rust_analyzer_lsp
```

## Check prefix artifact

```sh
PREFIX=target/prefix mise run prefix:check
```

This installs the prefix, compiles `tests/c_abi_smoke/main.c` using only pkg-config flags from `$PREFIX`, runs it against `tests/fixtures/basic_crate`, and audits the installed header/library symbols.

## Exported ABI

Consumers include the generated header and link the Rust `staticlib`/`cdylib`. The supported resolver surface is:

```c
RustAnalyzerStatus rust_analyzer_resolve_batch(
    const char *workspace_root,
    const RustAnalyzerDefSite *def_sites,
    int def_site_count,
    const RustAnalyzerFile *files,
    int file_count,
    RustAnalyzerResolvedCallArray *out);

void rust_analyzer_free_resolved_call_array(RustAnalyzerResolvedCallArray *out);
```

`workspace_root` must be the Cargo workspace root used by rust-analyzer. Every `RustAnalyzerFile.rel_path` and `RustAnalyzerDefSite.rel_path` must be relative to that root.

## Consumer adapter responsibility

`codebase-memory-mcp` or any other consumer should build an adapter that maps internal structs to generic ABI structs:

```text
consumer call record      → RustAnalyzerCall
consumer import record    → RustAnalyzerImport
consumer file record      → RustAnalyzerFile
consumer def-site record  → RustAnalyzerDefSite
RustAnalyzerResolvedCall  → consumer graph edge/result row
```

The adapter owns memory lifetime for input strings/arrays during the call. The Rust library owns returned result strings/array until the consumer calls `rust_analyzer_free_resolved_call_array`.

Do not add consumer-specific structs, headers, or checkout paths to this crate.
