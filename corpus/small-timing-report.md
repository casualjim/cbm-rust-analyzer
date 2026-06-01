# Small Corpus Fast Discovery Report

Clean-checkout syntax-only proof. Each repo starts with `target/` absent. Fast path uses `discover_workspace_calls(ResolverConfig::discovery_only(...))`, not rust-analyzer LSP, Cargo metadata, `cargo check`, build scripts, or proc macros.

| repo | rev | discovery_ms | candidates | functions | methods | macros |
|------|-----|--------------|------------|-----------|---------|--------|
| `serde-rs/serde` | `fa7da4a93567ed347ad0735c28e439fca688ef26` | 942 | 11690 | 6781 | 3674 | 1235 |
| `tower-rs/tower` | `251296dc54a044383dffd16d2179b443e2615672` | 224 | 2948 | 1014 | 1312 | 622 |

Notes:
- `discovery_ms` = syntax scan time only.
- no semantic resolution, goto-definition, hover, call hierarchy, proc-macro expansion, or OUT_DIR include proof in this fast path.
