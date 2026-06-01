# cbm-rust-analyzer

`cbm-rust-analyzer` is a Rust library that uses pinned `ra_ap_*` rust-analyzer crates to resolve Rust call relationships and return stable local/workspace call evidence through a generic C ABI.

Use it when a consumer already has Rust files, call records, and definition sites, and needs rust-analyzer-backed evidence for graph edges. The crate is not a CBM plugin: it does not include CBM headers, require `CBM_MCP_ROOT`, or depend on consumer internals.

## Guarantees

- Loads Cargo workspaces through rust-analyzer crates, not stdio LSP at production runtime.
- Resolves direct, trait/generic, and macro-related calls covered by fixtures and proof tests.
- Emits only local/workspace edges backed by rust-analyzer target evidence.
- Emits no edge for unresolved, ambiguous, external, std, or core targets.
- Never resolves by callee short name alone.
- Exposes a generic C ABI with `RustAnalyzer*` structs and `rust_analyzer_*` functions.
- Generates `include/rust_analyzer_lsp.h` from Rust `#[repr(C)]` types in `src/capi.rs`.

## Quick start

Run the verification path:

```sh
mise run abi:generate-header
mise run abi:check-header
cargo nextest run --no-fail-fast
PREFIX=target/prefix mise run prefix:check
```

`prefix:check` installs into `$PREFIX`, compiles `tests/c_abi_smoke/main.c` using only pkg-config flags, runs it against `tests/fixtures/basic_crate`, and audits header/library symbols.

## Install for C consumers

Install header, libraries, and pkg-config metadata into a prefix:

```sh
PREFIX=target/prefix mise run prefix:install
```

Installed artefacts:

```text
$PREFIX/include/rust_analyzer_lsp.h
$PREFIX/lib/librust_analyzer_lsp.a
$PREFIX/lib/librust_analyzer_lsp.dylib|so
$PREFIX/lib/pkgconfig/rust_analyzer_lsp.pc
```

Use pkg-config to compile consumers:

```sh
PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig" pkg-config --cflags --libs rust_analyzer_lsp
```

## C ABI reference

Include the generated header:

```c
#include <rust_analyzer_lsp.h>
```

Main resolver:

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

Inputs:

- `workspace_root` must be the Cargo workspace root used by rust-analyzer.
- `RustAnalyzerFile.rel_path` must be relative to `workspace_root`.
- `RustAnalyzerDefSite.rel_path` must be relative to `workspace_root`.
- File identity must come from `rel_path`, not module names or source text.

Outputs:

- `RustAnalyzerResolvedCallArray` contains only safe local/workspace resolved call rows.
- Returned strings and arrays are Rust-owned.
- Copy strings before storing them beyond the call boundary.
- Release output memory with `rust_analyzer_free_resolved_call_array`.

## Consumer adapter model

Consumers translate their own records into the generic ABI and translate resolved rows back into their own graph model:

```text
consumer call record      -> RustAnalyzerCall
consumer import record    -> RustAnalyzerImport
consumer file record      -> RustAnalyzerFile
consumer def-site record  -> RustAnalyzerDefSite
RustAnalyzerResolvedCall  -> consumer graph edge/result row
```

Consumer code owns input memory for the duration of the call. This crate owns output memory until the free function runs.

For CBM-specific adapter notes, see `docs/cbm-integration.md`.

## Resolution behaviour

The resolver is conservative by design:

- Real resolved local/workspace targets produce rows.
- RA target evidence must map to a stable def-site range.
- Ambiguous, unresolved, external, std, and core targets produce no edge.
- Proc-macro/generated ambiguity must not create false edges.
- Accuracy is preferred over recall.

## Distribution

Releases use cargo-dist with the C API library pattern from `palate-capi`. Config lives in `dist-workspace.toml`.

Configured targets:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-msvc`

Configured installers/artifacts:

- Homebrew formula
- release archives with `cdylib`, `cstaticlib`, `include/rust_analyzer_lsp.h`, and `lib/pkgconfig/rust_analyzer_lsp.pc`
- patched Homebrew formula that installs the header into `include/` and pkg-config metadata into `lib/pkgconfig/`

Check release plan locally:

```sh
dist plan
```

Build current host artifacts locally:

```sh
dist build --target=aarch64-apple-darwin --artifacts=all
```

## Development tasks

```sh
mise run build:debug          # debug build, ensures generated header exists
mise run abi:generate-header  # regenerate include/rust_analyzer_lsp.h
mise run abi:check-header     # drift + forbidden symbol check
PREFIX=target/prefix mise run prefix:install
PREFIX=target/prefix mise run prefix:check
cargo nextest run --no-fail-fast
```

`mise run clean` removes generated header and target artefacts. Restore the header with:

```sh
mise run abi:generate-header
```

## Licence

Licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE`)
- MIT licence (`LICENSE-MIT`)

at your option.

## Source of truth

- ABI source: `src/capi.rs`
- Generated header config: `cbindgen.toml`
- Generated header artefact: `include/rust_analyzer_lsp.h`
- Project spec/proof tasks: `SPEC.md`

Do not edit the generated header by hand.
