# Corpus candidates

Purpose: rank well-known Rust repos for resolver POC coverage/cost. Revs stay `TBD by T12 manifest`; this file chooses tiers only.

## Chosen tiers

| tier | repos | why |
|---|---|---|
| small/fast | `serde-rs/serde`, `tower-rs/tower` | high signal, manageable load; derive/generic/no_std + trait-heavy service abstractions |
| medium | `clap-rs/clap`, `tokio-rs/axum`, `hyperium/hyper` | proc-macro derive, web extractors/routes, async protocol stack |
| large/stress | `tokio-rs/tokio`, `bevyengine/bevy`, `rust-lang/rust-analyzer` | cfg/features, async runtime, ECS/proc macros, huge compiler-like workspace |
| DB macro track | primary: `launchbadge/sqlx`; alternate: `diesel-rs/diesel` | SQL macro/query stress vs trait/type-heavy ORM stress |

## Candidate ranking

| repo | URL | stress areas | cost | include? | notes |
|---|---|---|---|---|---|
| `serde-rs/serde` | https://github.com/serde-rs/serde | derive/proc-macro, traits, generics, no_std, cfg/features | small | yes | baseline macro/generic crate; widely used |
| `tower-rs/tower` | https://github.com/tower-rs/tower | trait/generic, service/layer abstractions, async-adjacent | small | yes | good false-positive detector for trait dispatch |
| `clap-rs/clap` | https://github.com/clap-rs/clap | derive/proc-macro, builder API, cfg/features | medium | yes | exercises proc macro + fluent call chains |
| `tokio-rs/axum` | https://github.com/tokio-rs/axum | web, routes, extractors, async, tower integration | medium | yes | real web framework surface |
| `hyperium/hyper` | https://github.com/hyperium/hyper | async networking, protocol stack, traits | medium | yes | protocol code without full Tokio workspace cost |
| `tokio-rs/tokio` | https://github.com/tokio-rs/tokio | async/runtime, macros, cfg/features, large workspace | large | yes | core async runtime stress; may be slower |
| `launchbadge/sqlx` | https://github.com/launchbadge/sqlx | DB/query macros, async, feature matrix | large | primary DB | compile-time SQL macro behavior; environment sensitivity risk |
| `diesel-rs/diesel` | https://github.com/diesel-rs/diesel | DB/query DSL, traits/types, macros | large | alternate DB | less async, very type-heavy; good alternate to `sqlx` |
| `bevyengine/bevy` | https://github.com/bevyengine/bevy | ECS, proc macros, large workspace, plugin patterns | huge | stress | expensive; use after small/medium pass |
| `rust-lang/rust-analyzer` | https://github.com/rust-lang/rust-analyzer | compiler-like workspace, salsa/hir, macros, cfg/features | huge | stress | closest domain stress; highest cost |

## Coverage matrix

| V13 dimension | primary repos | backup repos |
|---|---|---|
| derive/proc-macro | `serde-rs/serde`, `clap-rs/clap` | `bevyengine/bevy` |
| macro_rules | `tokio-rs/tokio`, `serde-rs/serde` | `rust-lang/rust-analyzer` |
| async/runtime | `tokio-rs/tokio`, `hyperium/hyper` | `tokio-rs/axum`, `launchbadge/sqlx` |
| trait/generic | `tower-rs/tower`, `serde-rs/serde` | `diesel-rs/diesel`, `rust-lang/rust-analyzer` |
| web | `tokio-rs/axum` | `hyperium/hyper` |
| DB/query macro | `launchbadge/sqlx` | `diesel-rs/diesel` |
| large workspace/compiler-like | `rust-lang/rust-analyzer` | `bevyengine/bevy`, `tokio-rs/tokio` |
| cfg/features | `tokio-rs/tokio`, `clap-rs/clap` | `serde-rs/serde`, `launchbadge/sqlx` |

## Suggested execution order

1. small/fast: `serde-rs/serde`, `tower-rs/tower`.
2. medium: `clap-rs/clap`, `tokio-rs/axum`, `hyperium/hyper`.
3. DB macro track: `launchbadge/sqlx`; if env/proc-macro setup blocks, use `diesel-rs/diesel`.
4. large/stress: `tokio-rs/tokio`, then `bevyengine/bevy`, then `rust-lang/rust-analyzer`.

## Risks

- Revs not pinned here; T12 manifest must pin URL + commit/rev.
- Large repos may exceed local cold-load budget; T16 metrics should capture cost before pass/fail judgments.
- Proc macros/build scripts may execute code; T15 storage/fetch policy must define isolation.
- DB macro crates may need database/offline metadata; treat missing env as corpus limitation, not resolver failure.

## Next tasks

- T15: choose corpus storage/fetch policy.
- T16: define metrics report format.
- T12: create pinned corpus manifest and run validation.
