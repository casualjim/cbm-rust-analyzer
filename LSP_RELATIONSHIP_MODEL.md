# LSP relationship model parity

Goal: Rust resolver should provide parity with CBM `go_lsp`, `ts_lsp`, `py_lsp`, etc. Parity means: build type-aware call relationships from the compiled/configured program model, then emit normalized resolved-call evidence for graph edge creation.

## Sources inspected

Read-only reference repo: `/Users/ivan/github/DeusData/codebase-memory-mcp`.

Key files:

- `docs/TS_LSP_INTEGRATION_PLAN.md`
- `PYTHON_LSP_PLAN.md`
- `internal/cbm/lsp/go_lsp.h`
- `internal/cbm/cbm.h`
- `src/pipeline/lsp_resolve.h`
- `src/pipeline/pass_lsp_cross.{c,h}`
- `src/pipeline/pass_calls.c`
- `tests/test_go_lsp.c`
- `tests/test_ts_lsp.c`
- `tests/test_py_lsp.c`

## Existing CBM LSP pattern

The existing CBM LSP layers are not editor-feature tests. They are graph-edge resolvers.

Pipeline shape:

1. Tree-sitter/unified extractor gets defs, imports, calls.
2. LSP/type-aware layer builds scopes + type registry.
3. Single-file pass resolves local type-aware calls.
4. Cross-file pass builds project summaries (`CBMLSPDef[]`) from all defs + import map.
5. Per-language resolver appends `CBMResolvedCall` records.
6. `pass_calls.c` checks LSP-resolved calls before text registry fallback.
7. Graph emits `CALLS` edges with `strategy` + `confidence`.

Common output contract from `internal/cbm/cbm.h`:

```c
typedef struct {
    const char *caller_qn;
    const char *callee_qn;
    const char *strategy;
    float confidence;
    const char *reason;
} CBMResolvedCall;
```

Pipeline admission rule from `src/pipeline/lsp_resolve.h`:

- confidence floor: `CBM_LSP_CONFIDENCE_FLOOR = 0.6f`
- LSP result wins before textual registry fallback
- below floor or no target → fallback/skip

## Parity target for Rust

Rust RA work should not stop at “LSP query returned something.” It should produce the same kind of normalized relationship evidence:

| field | Rust meaning |
|---|---|
| `caller_qn` | enclosing Rust function/method/module item qualified name |
| `callee_qn` | resolved target qualified name when exact; external/generated marker when not exact |
| `strategy` | `ra_definition`, `ra_call_hierarchy`, `ra_hover_type`, `ra_macro_expansion`, `ra_proc_macro`, `ra_deref_dispatch`, `ra_unresolved`, `ra_ambiguous` |
| `confidence` | edge admission confidence; exact local high, generated/ambiguous lower, unresolved zero |
| `reason` | unresolved/ambiguous/generated explanation |

Rust-specific additions needed beyond CBMResolvedCall:

- call site file + byte range
- target file + byte range when local
- Cargo config used: manifest, features, cfgs, target triple, proc-macro/build-script policy
- RA raw evidence payload for hard cases while POC learns behavior

## Compiled/configured-code-first rule

For Rust, relationship building must start from the code RA actually analyzes:

1. Cargo manifest/workspace root
2. active features and cfg flags
3. target triple if set
4. path dependencies/workspace members
5. proc-macro/build-script policy
6. opened documents and source roots

Then collect relationships. This mirrors CBM cross-file LSP: build project summary first, then resolve calls using only relevant defs/imports.

## Evidence ladder

For each call site, gather evidence in order:

1. `textDocument/definition` at call token
2. `callHierarchy/prepare` + incoming/outgoing calls for caller/target validation
3. `textDocument/hover` for type/signature fallback
4. document symbols for source/target range anchoring
5. macro/proc-macro provenance from RA locations/expansion-like ranges when available

Do not collapse hard cases into “limitation” only. Record raw RA response and graph decision.

## Graph decision classes

| class | emit edge? | confidence | condition |
|---|---:|---:|---|
| exact local | yes | `0.95–1.0` | target maps to local source def range |
| cross-crate workspace | yes | `0.90–1.0` | target maps to workspace/path dependency source def range |
| external dependency | optional/no | `0.70–0.85` | target outside workspace, qn known but graph node absent |
| generated macro | maybe annotate/no | `0.50–0.80` | target/generated provenance known but no stable source def |
| ambiguous | no | `<0.60` | multiple targets, no deterministic graph edge |
| unresolved | no | `0.0` | RA gives no usable target |

## Hard-case policy

The current T10 fixture exposed the right hard cases:

- `macro_rules!` generated function
- proc-macro-derived method
- auto-deref method dispatch

Next Rust work should dump per-case RA evidence and classify graph decision. The POC succeeds when we can say exactly which edges CBM would emit or skip and why, using a stable resolved-call contract.

## Next concrete task

Add a Rust relationship evidence report for fixture calls:

- source call range
- caller item
- all RA definition locations
- hover summary
- call-hierarchy result
- chosen strategy
- confidence
- graph decision
- unresolved/generated reason

This report is the Rust analogue of `resolved_calls` tests in `test_go_lsp.c`, `test_ts_lsp.c`, and `test_py_lsp.c`.
