# Small corpus validation

Scope: initial POC corpus only. Larger repos deferred until macro/derive + trait/generic proof passes.

## Active repos

| repo | rev | stress tags | pin check |
|---|---|---|---|
| `serde-rs/serde` | `fa7da4a93567ed347ad0735c28e439fca688ef26` | derive/proc-macro, traits, generics, no_std, cfg/features | `git ls-remote ... HEAD` matched |
| `tower-rs/tower` | `251296dc54a044383dffd16d2179b443e2615672` | traits, generics, service/layer, async-adjacent, cfg/features | `git ls-remote ... HEAD` matched |

## Coverage vs invariants

| invariant | status | evidence |
|---|---|---|
| V11 | pass | `corpus/small.toml` pins URL + rev + stress tags for each active repo |
| V12 | ready | metrics schema listed below; runner/output pending resolver corpus command |
| V13 | partial-small | covers derive/proc-macro, trait/generic, cfg/features; async/runtime/web/DB/large deferred by V14/T18 |
| V14 | pass | manifest includes only `serde-rs/serde` + `tower-rs/tower` |
| V15 | ready | `serde-rs/serde` selected as macro/derive proof repo; false-positive sampling required during run |

## Metrics report schema

When corpus runner exists, emit one row per repo:

| field | meaning |
|---|---|
| `repo` | repo slug |
| `rev` | pinned commit |
| `load_time_ms` | rust-analyzer/project load time |
| `resolved_local_targets` | count of resolved workspace-local call targets |
| `external_targets` | count of dependency/external targets |
| `unresolved_count` | unresolved call/target count |
| `ambiguous_count` | ambiguous target count |
| `sampled_false_positives` | manually sampled wrong target count |
| `notes` | macro/derive/trait caveats |

## Current run results

| repo | load_time_ms | resolved_local_targets | external_targets | unresolved_count | ambiguous_count | sampled_false_positives | status |
|---|---:|---:|---:|---:|---:|---:|---|
| `serde-rs/serde` | - | - | - | - | - | - | manifest-pinned; runner pending |
| `tower-rs/tower` | - | - | - | - | - | - | manifest-pinned; runner pending |

## Verification commands used

```sh
git ls-remote https://github.com/serde-rs/serde.git HEAD
git ls-remote https://github.com/tower-rs/tower.git HEAD
test -f corpus/small.toml
test -f corpus/small-validation.md
rg 'serde-rs/serde|tower-rs/tower' corpus/small.toml
! rg 'clap-rs/clap|tokio-rs/axum|hyperium/hyper|tokio-rs/tokio|launchbadge/sqlx|diesel-rs/diesel|bevyengine/bevy|rust-lang/rust-analyzer' corpus/small.toml
```

## Deferred by V14/T18

- `clap-rs/clap`
- `tokio-rs/axum`
- `hyperium/hyper`
- `tokio-rs/tokio`
- `launchbadge/sqlx` or `diesel-rs/diesel`
- `bevyengine/bevy`
- `rust-lang/rust-analyzer`
