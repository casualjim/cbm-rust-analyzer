# SPEC

## §G GOAL
rust-analyzer crate-backed resolver loads Cargo workspace & emits CBM-safe Rust call evidence.

## §C CONSTRAINTS
- Rust 2024 Cargo crate `cbm-rust-analyzer`.
- app code currently placeholder until T5 replaces `src/lib.rs::add`.
- production intent from `RUST_ANALYZER_RESOLVER_LIBRARY_PLAN.md` ! vendorable Rust library + stable C ABI for CBM later.
- first real milestone ! Rust API loader in `src/lib.rs`; C ABI later.
- resolution ! use rust-analyzer semantics, not raw string/tree-sitter matching.
- accuracy > recall: wrong edge ⊥; skip/mark unresolved when uncertain.
- LSP spike ! oracle only; production runtime uses pinned `ra_ap_*` crates, not stdio LSP.
- LSP oracle tests require `rust-analyzer` executable on PATH.
- LSP spike uses `async-lsp-client`, `tokio`, `tower-lsp`, `serde_json`, `eyre`, `url` as dev deps.
- `ra_ap_*` deps ! exact pin + version/license record before CBM integration.
- v1 resolver ! workspace scan first; selected files/positions API future-compatible.
- trusted-full default ! proc macros + build scripts enabled; effective policy recorded in diagnostics.
- fixture workspace lives under `tests/fixtures/basic_crate`.
- corpus repos ? external complex Rust repos; fetch/cache outside app code or as explicit test assets.
- POC corpus ! start small: `serde-rs/serde` + `tower-rs/tower`; larger repos later.
- clean corpus timing ! measure with repo `target/` absent before rust-analyzer start.
- request timeout = `20s`.
- parity target ! match CBM `go_lsp`/`ts_lsp`/`py_lsp` resolved-call contract, not editor-query smoke only.
- relationship graph ! start from compiled/configured Cargo workspace then emit typed call evidence.
- external deps/std/core targets ! diagnostics/counts only; CALLS edge output requires local/workspace target node.
- POC proof gates passed enough for real-code loader start; LSP/corpus reports stay oracle evidence.
- proof gate ! reliable macro_rules/proc-macro/generated/trait-generic batch relationships over real corpus.
- do not edit app code during spec distill.
- completion gate ! CBM integration contract inside this repo, not standalone resolver/perf path.
- product path ! exported C ABI → RA semantics → `CBMResolvedCall`; syntax discovery/prefilter ! proof ⊥.
- fake short-name matching ⊥ in product path; no edge unless RA target maps to stable local/workspace def.
- C ABI ! deterministic file identity: `rel_path` or batch file input; missing identity → no-edge, not guess.
- tests ! `#[ignore]` ⊥; relevant expensive tests run default or get replaced/deleted with stronger default proof.
- CBM-only scope ! support calls/fields needed by `codebase-memory-mcp`; non-CBM public modes ? internal helper only.

## §I INTERFACES
- pkg: `Cargo.toml` → crate `cbm-rust-analyzer`, edition `2024`.
- lib: `src/lib.rs` → Rust resolver API loader: config → workspace state + diagnostics/timings.
- api: resolver input → workspace scan now; selected files/positions later.
- cmd: `cargo test` → unit test + rust-analyzer LSP oracle tests + future loader tests pass.
- lsp oracle: `rust-analyzer` stdio harness → fixture/corpus validation only; production runtime ⊥.
- file: `RUST_ANALYZER_RESOLVER_LIBRARY_PLAN.md` → intended resolver requirements/plan ?
- file: `tests/fixtures/basic_crate/Cargo.toml` → synthetic fixture manifest.
- file: `tests/fixtures/basic_crate/src/lib.rs` → direct/method/trait/generic/macro call fixture.
- file: corpus manifest ? lists complex repos + revs + expected stress area.
- corpus active now: `serde-rs/serde` derive/proc-macro/generic/no_std; `tower-rs/tower` trait/generic services.
- corpus deferred: `clap-rs/clap`, `tokio-rs/axum`, `hyperium/hyper`, `tokio-rs/tokio`, `launchbadge/sqlx` or `diesel-rs/diesel`, `bevyengine/bevy`, `rust-lang/rust-analyzer`.
- cmd: small corpus smoke ? clean checkout pinned repos → rust-analyzer timings + semantic query result.
- file: corpus timing report ? cold load, first query, warm query, usable/failed notes.
- file: `LSP_RELATIONSHIP_MODEL.md` → parity model from `codebase-memory-mcp` LSP resolvers.
- report: Rust resolved call evidence ? caller_qn, callee_qn, strategy, confidence, reason, call/target ranges.
- report: complex proof ? macro_rules edges, proc-macro generated methods, real macro-heavy corpus, batch all-call extraction.
- output: production `CBMResolvedCall` rows for local/workspace call evidence; external deps/std/core → diagnostics/counts only.
- lsp: `initialize` + `initialized` → fixture workspace load.
- lsp: `textDocument/didOpen` → open fixture lib.
- lsp: `textDocument/documentSymbol` → symbol discovery.
- lsp: `textDocument/definition` → call target definition lookup.
- lsp: `textDocument/hover` → symbol signature/text check.
- lsp: `textDocument/prepareCallHierarchy` + `callHierarchy/incomingCalls` → caller/call-site check.
- lsp server requests: `workspace/configuration`, `window/workDoneProgress/create` handled; unknown requests → method-not-found error.

## §V INVARIANTS
V1: fixture manifest `tests/fixtures/basic_crate/Cargo.toml` ! exist before LSP spike.
V2: rust-analyzer initialize/open/query/shutdown ! complete within `REQUEST_TIMEOUT = 20s` or fail test.
V3: document symbols for `tests/fixtures/basic_crate/src/lib.rs` ! non-empty.
V4: ∀ non-limited fixture case direct/method/trait/generic → goto definition target range ! contain expected definition marker position.
V5: ∀ fixture case → hover result ! contain expected symbol text, unless documented limitation.
V6: ∀ non-limited fixture case → incoming call hierarchy ! map `exercise_all_cases` to original call-site range.
V7: macro fixture limitation ! recorded as documented limitation, not hidden false positive.
V8: `RustAnalyzerSession::shutdown` ! send shutdown, exit server, abort message task.
V9: future C ABI ? returned Rust-owned memory ! valid until explicit result/handle destroy; C frees Rust strings directly ⊥.
V10: future resolver ? no panic crosses FFI; failures return status + diagnostics.
V11: corpus manifest ? pin repo URL + commit/rev + stress tags for each repo.
V12: corpus run ? report clean-repo cold load time, first useful query latency, warm repeat latency, resolved local targets, external targets, unresolved/ambiguous count, sampled false positives.
V13: corpus selection ? cover derive/proc-macro, macro_rules, async/runtime, trait/generic, web, DB/query macro, large workspace/compiler-like, cfg/features.
V14: POC corpus scope ! active small repos only until macro/derive + trait/generic proof passes.
V15: macro/derive proof ! exercise proc-macro-derived code paths without false positive targets.
V16: clean corpus smoke ! start from pinned checkout with `target/` absent.
V17: POC proof ! rust-analyzer loads active repo + returns usable document symbols and ≥1 targeted semantic query result.
V18: LSP parity ! Rust emits normalized resolved-call evidence compatible with CBM `caller_qn`, `callee_qn`, `strategy`, `confidence`, `reason` model.
V19: compile config ! record Cargo manifest, features, cfg, target triple?, proc-macro/build-script policy before relationship resolution.
V20: relationship evidence ! ∀ call → source range + caller + raw RA evidence + target range or unresolved reason + confidence.
V21: graph safety ! emit edge only when target maps to stable local/workspace source range & confidence ≥ `0.6`; ambiguous/unresolved → no edge.
V22: hard cases ! macro/proc-macro/deref cases record raw RA responses and graph decision, not just limitation text.
V23: POC proof gate ! macro_rules generated call edges resolve to stable source/workspace target ranges with confidence ≥ `0.6`.
V24: POC proof gate ! proc-macro-derived/generated method edges resolve or classify generated provenance without false positive edges.
V25: macro-heavy corpus ! ≥1 real crate beyond smoke-level `serde-rs/serde` exercises macro/proc-macro relationship extraction.
V26: complex combo corpus ! large trait/generic/proc-macro patterns produce exact/generated/ambiguous/unresolved classifications.
V27: batch proof ! real project textual call sites inventoried in one run; report total/macro counts + sampled semantic rows classify exact/generated/ambiguous/unresolved.
V28: production output ! local/workspace calls emit `CBMResolvedCall` rows with `caller_qn`, `callee_qn`, `strategy`, `confidence`, `reason`; external deps/std/core → diagnostics/counts only.
V29: generated include proof ! `build.rs` writes `OUT_DIR` Rust file + `include!(concat!(env!("OUT_DIR"), "..."))`; RA resolves included calls and records generated-file provenance/edge policy.
V30: production resolver ! load Cargo workspace through pinned `ra_ap_*`; stdio LSP runtime ⊥.
V31: first milestone ! `src/lib.rs` exposes Rust API loader before C ABI.
V32: trusted-full default ! proc macros + build scripts enabled; effective policy recorded in diagnostics.
V33: resolver input ! workspace scan now & selected files/positions extension point later.
V34: RA deps ! exact pin + version/license record before CBM integration.
V35: LSP oracle tests ! validate behavior; production resolver load must not spawn `rust-analyzer` process.
V36: dependency policy ! target ∉ local/workspace graph node → no CALLS edge row; record reason/count only.
V37: feature complete ⇔ exported CBM ABI accepts CBM inputs + deterministic file identity and returns RA-backed `CBMResolvedCall` rows.
V38: product resolver ! never resolve by callee short-name alone; RA definition/target evidence required.
V39: ABI macro proof ! `macro_rules!` call through exported C ABI resolves stable local/workspace target or emits no-edge reason.
V40: ABI proc-macro proof ! generated method/call provenance classified; false positive edge ⊥.
V41: ABI trait/generic proof ! method target from RA semantics, not syntax method name.
V42: default test suite ! `#[ignore]` absent across repo.
V43: fast discovery ! prefilter/helper only; completion proof requires semantic ABI tests.
V44: file identity ! single-file ABI includes `rel_path` or production uses batch ABI; source/module guessing ⊥.
V45: safe failure ! RA load/query/map failure returns status/diagnostic or no-edge classification; panic/unsafe guessed edge ⊥.

## §T TASKS
id|status|task|cites
T1|x|create direct/method/trait/generic/macro fixture crate|V1,V3
T2|x|drive rust-analyzer LSP process over stdio|V2,I.lsp
T3|x|verify document symbols/goto definition/hover/incoming calls in fixture|V3,V4,V5,V6
T4|x|document macro case limitation path|V7
T5|x|replace placeholder `src/lib.rs::add` with Rust API loader config/result surface|I.lib,V31
T6|x|choose embedded pinned `ra_ap_*` crates; keep stdio LSP as oracle only|V30,V35
T7|x|generate CBM bindings, expose Rust-owned temp ABI, add CBM wrapper that copies CBMResolvedCall rows into arena|V9,V10,V18,V31
T8|x|implement Cargo workspace loader with trusted-full proc-macro/build-script policy|V19,V30,V32
T9|x|implement batch call collection/resolution: workspace scan first, selected inputs later|V28,V33,V21
T10|x|extend fixtures: associated fn, deref, async, cfg feature, cross-crate, proc macro ?|V4,V5,V6,V7
T11|x|add diagnostics/timing/timeout/cancellation result model ?|V2,V10
T12|x|build initial small corpus manifest + validate `serde-rs/serde`, `tower-rs/tower`; defer larger repos|V11,V12,V13,V14,V15,V4,V6,V7
T13|x|pin `ra_ap_*` deps + version/license notices|V30,V34
T14|x|add CI/env check for available `rust-analyzer` binary ?|V2
T15|x|choose corpus storage/fetch policy: local `target/corpus-checkouts` cache|V11
T16|x|add corpus metrics report format|V12
T17|x|rank corpus candidates by coverage/cost; choose small/medium/large tiers ?|V11,V12,V13
T18|.|later expand corpus to deferred medium/large repos after small proof passes|V12,V13,V14
T19|x|add clean-checkout small corpus RA smoke proof + timing report for `serde-rs/serde`, `tower-rs/tower`|V2,V11,V12,V14,V15,V16,V17,V4,V6,V7
T20|x|distill CBM LSP parity model from `codebase-memory-mcp` go/ts/py LSPs|V18,V19,V20,V21,V22
T21|x|add Rust fixture relationship evidence report matching resolved-call contract|V18,V19,V20,V21,V22
T22|x|classify macro/proc-macro/deref RA evidence into exact/generated/ambiguous/unresolved graph decisions|V20,V21,V22,V7
T23|x|adapt Rust API plan toward `CBMResolvedCall`-style output before C ABI expansion|V9,V10,V18,V20,V21
T24|x|prove `macro_rules!` invocation/source-target edges in fixture + real crate; expansion-internal graph out of scope|V23,V20,V21,V22
T25|x|prove proc-macro-derived/generated method edge handling without false positives|V24,V15,V20,V21,V22
T26|x|add macro-heavy real corpus beyond `serde-rs/serde` for proof gate|V25,V11,V12,V13
T27|x|prove curated broad semantic sample over trait/generic/proc-macro combos; exhaustive combo coverage out of scope|V26,V20,V21,V22
T28|x|batch inventory all textual call sites + sampled semantic proof counts|V27,V12,V18,V20,V28
T29|x|emit production-quality sampled semantic `CBMResolvedCall` JSONL rows; full textual inventory rows out of scope|V28,V18,V19,V20,V21
T30|x|add `OUT_DIR` `include!` generated-code fixture + RA relationship proof|V29,V19,V20,V21,V22
T31|x|choose external dependency policy: omit edge rows; diagnostics/counts only|V21,V28,V36
T32|x|define isolated Cargo target-dir diagnostics for trusted-full loader|V19,V32
T33|x|add/verify exported CBM-facing C ABI symbol and header contract for later `codebase-memory-mcp` link|V37,V44,V9,V10
T34|x|replace C ABI short-name resolver with RA-backed call target resolver|V37,V38,V20,V21
T35|x|thread deterministic file identity through ABI: add `rel_path` or require batch file input|V37,V44
T36|x|add default ABI contract test: CBM-style defs/calls → exported ABI → `CBMResolvedCall`|V37,V42,V43
T37|x|resolve macro-expanded cross-file method call via RA stable target; proc-macro/generated ambiguous → no-edge|V39,V40,V41,V45
T38|x|add guard/test or CI check that repo has no `#[ignore]` tests|V42

## §B BUGS
id|date|cause|fix
