# Rust LSP relationship evidence

Fixture relationship proof for CBM `CBMResolvedCall` parity.

## Compile config

- manifest: `tests/fixtures/basic_crate/Cargo.toml`
- linkedProjects: [`tests/fixtures/basic_crate/Cargo.toml`]
- features: [`cfg-fixture`]
- target triple: default host
- build scripts: enabled
- generated include: `build.rs` writes `OUT_DIR/generated_include.rs`; fixture uses `include!(concat!(env!("OUT_DIR"), "/generated_include.rs"))`
- proc macros: enabled
- request timeout: `20s`

## Timings

- cold start + first relationship pass: `2.849192666s`
- warm relationship pass: `9.790792ms`

## Resolved-call evidence

| case | caller_qn | callee_qn | strategy | confidence | graph_decision | reason | call_range | target_ranges | hover | call_hierarchy |
|---|---|---|---|---:|---|---|---|---|---|---|
| `direct` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::direct_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `118:23` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@7:7-7:20 | true | true |
| `method` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::method_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `119:34` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@14:11-14:24 | true | true |
| `trait` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::trait_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `120:36` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@34:7-34:19 | true | true |
| `generic` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::generic_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `121:25` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@39:7-39:21 | true | true |
| `macro` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::macro_target` | `ra_macro_expansion` | 0.70 | `emit_call_edge` | macro_rules target has stable source range; call hierarchy provenance recorded separately | `122:28` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@72:15-72:27; file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@78:0-78:20 | true | false |
| `associated` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::associated_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `125:47` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@22:11-22:28 | true | true |
| `deref` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::deref_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `126:36` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@46:11-46:23 | true | true |
| `cfg_feature` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::cfg_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `127:25` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@66:7-66:17 | true | true |
| `cross_crate` | `cbm_ra_fixture_basic::exercise_all_cases` | `fixture_cross_crate::cross_crate_target` | `ra_definition_cross_crate+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `128:31` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/fixture-cross-crate/src/lib.rs@0:7-0:25 | true | true |
| `proc_macro_derive` | `cbm_ra_fixture_basic::exercise_all_cases` | `cbm_ra_fixture_basic::GeneratedByDerive::proc_macro_target` | `ra_proc_macro` | 0.50 | `no_emit_generated` | proc-macro generated method points at derive macro/proc-macro provenance, not stable method source range | `129:39` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/fixture-derive/src/lib.rs@2:32-3:11; file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@95:2-95:25 | true | false |
| `async` | `cbm_ra_fixture_basic::exercise_async_case` | `cbm_ra_fixture_basic::async_target` | `ra_definition+ra_call_hierarchy` | 0.95 | `emit_call_edge` | stable local/workspace target range and caller call-site evidence | `148:10` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@61:13-61:25 | true | true |
| `include_out_dir_source_to_generated` | `cbm_ra_fixture_basic::exercise_out_dir_include_case` | `cbm_ra_fixture_basic::out_dir_generated::generated_out_dir_target` | `ra_include_out_dir` | 0.50 | `no_emit_generated_file` | source call resolves to build.rs OUT_DIR include file; generated target provenance recorded and edge suppressed | `152:51` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/target/debug/build/cbm-ra-fixture-basic-49f24fc9b0582bcf/out/generated_include.rs@1:7-1:31 | true | false |
| `include_out_dir_generated_to_workspace` | `cbm_ra_fixture_basic::out_dir_generated::generated_calls_workspace` | `cbm_ra_fixture_basic::out_dir_workspace_target` | `ra_include_out_dir` | 0.50 | `no_emit_generated_file` | generated OUT_DIR include caller resolves to workspace target; generated caller provenance recorded and edge suppressed | `6:23` | file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@91:7-91:31 | true | false |

## Hard-case graph decisions

| case | class | emit? | reason |
|---|---|---|---|
| `macro` | `exact` | true | macro_rules target has stable source range; call hierarchy provenance recorded separately |
| `proc_macro_derive` | `generated` | false | proc-macro generated method points at derive macro/proc-macro provenance, not stable method source range |
| `deref` | `exact` | true | stable local/workspace target range and caller call-site evidence |
| `include_out_dir_source_to_generated` | `no_emit_generated_file` | false | source call resolves to build.rs OUT_DIR include file; generated target provenance recorded and edge suppressed |
| `include_out_dir_generated_to_workspace` | `no_emit_generated_file` | false | generated OUT_DIR include caller resolves to workspace target; generated caller provenance recorded and edge suppressed |

## Raw RA evidence

### direct

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@7:7-7:20
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [118:17-118:30]

### method

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@14:11-14:24
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [119:28-119:41]

### trait

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@34:7-34:19
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [120:30-120:42]

### generic

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@39:7-39:21
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [121:18-121:32]

### macro

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@72:15-72:27; file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@78:0-78:20
- incoming calls: []

### associated

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@22:11-22:28
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [125:39-125:56]

### deref

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@46:11-46:23
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [126:30-126:42]

### cfg_feature

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@66:7-66:17
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [127:20-127:30]

### cross_crate

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/fixture-cross-crate/src/lib.rs@0:7-0:25
- incoming calls: exercise_all_cases file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [128:22-128:40]

### proc_macro_derive

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/fixture-derive/src/lib.rs@2:32-3:11; file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@95:2-95:25
- incoming calls: []

### async

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@61:13-61:25
- incoming calls: exercise_async_case file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs [148:4-148:16]

### include_out_dir_source_to_generated

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/target/debug/build/cbm-ra-fixture-basic-49f24fc9b0582bcf/out/generated_include.rs@1:7-1:31
- incoming calls: generated target call hierarchy not trusted for graph edge

### include_out_dir_generated_to_workspace

- definitions: file:///Users/ivan/github/casualjim/cbm-rust-analyzer/tests/fixtures/basic_crate/src/lib.rs@91:7-91:31
- incoming calls: generated caller call hierarchy not trusted for graph edge

