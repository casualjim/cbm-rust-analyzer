# Proof Corpus Fast Discovery Report

Fast path uses syntax-only `discover_workspace_calls(ResolverConfig::discovery_only(...))`. It does not start `rust-analyzer`, load Cargo metadata, run `cargo check`, execute build scripts, or expand proc macros.

| discovery_ms | textual_call_candidates | function_candidates | method_candidates | macro_candidates | proof_rows |
|--------------|-------------------------|---------------------|-------------------|------------------|------------|
| 1260 | 29625 | 6436 | 16853 | 6336 | 9 |

## Proof rows

| label | file | callee | kind | byte_range |
|-------|------|--------|------|------------|
| `macro_rules_arg` | `tests/macros.rs` | `arg` | `macro` | `65..74` |
| `proc_macro_command` | `tests/derive/app_name.rs` | `MyApp::command` | `function` | `1385..1399` |
| `trait_generic_mkeymap_get` | `clap_builder/src/mkeymap.rs` | `get` | `method` | `4121..4124` |
| `builder_command_new` | `tests/builder/utils.rs` | `Command::new` | `function` | `1726..1738` |
| `builder_command_arg` | `tests/builder/utils.rs` | `arg` | `method` | `2166..2169` |
| `builder_arg_value_parser` | `tests/builder/utils.rs` | `value_parser` | `method` | `2792..2804` |
| `generic_arg_matches_unwrap` | `tests/builder/default_missing_vals.rs` | `unwrap` | `method` | `447..453` |
| `builder_try_get_matches_from` | `tests/builder/default_missing_vals.rs` | `try_get_matches_from` | `method` | `355..375` |
| `external_iterator_collect` | `tests/builder/utils.rs` | `collect` | `method` | `622..629` |

Fast discovery proof rows are candidate rows only, not `CBMResolvedCall` semantic edges. Full RA oracle remains available through ignored tests.
