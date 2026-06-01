use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use cbm_rust_analyzer::{
    CallCandidate, CallCandidateKind, ResolverAnalysisMode, ResolverConfig,
    discover_workspace_calls,
};
use eyre::{Result, WrapErr, bail};

const PROOF_MANIFEST: &str = "corpus/proof.toml";

#[derive(Debug, Clone, Copy)]
struct ProofRepo {
    name: &'static str,
    slug: &'static str,
    url: &'static str,
    rev: &'static str,
}

const CLAP: ProofRepo = ProofRepo {
    name: "clap",
    slug: "clap-rs/clap",
    url: "https://github.com/clap-rs/clap.git",
    rev: "71a7213d6e45758bb8c9c5f7c36ed768edef872e",
};

#[derive(Debug, Clone, Copy)]
struct DiscoveryProbe {
    label: &'static str,
    relative_path: &'static str,
    symbol: &'static str,
    kind: Option<CallCandidateKind>,
    callee_contains: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct DiscoveryRow {
    label: &'static str,
    file: String,
    callee: String,
    kind: CallCandidateKind,
    start_byte: u32,
    end_byte: u32,
}

fn assert_manifest() -> Result<()> {
    let manifest = fs::read_to_string(PROOF_MANIFEST).wrap_err("failed to read proof manifest")?;
    for expected in [
        CLAP.slug,
        CLAP.rev,
        "macro_rules",
        "proc-macro",
        "CBMResolvedCall",
    ] {
        if !manifest.contains(expected) {
            bail!("proof manifest missing {expected}");
        }
    }
    Ok(())
}

fn discovery_probes() -> Vec<DiscoveryProbe> {
    vec![
        DiscoveryProbe {
            label: "macro_rules_arg",
            relative_path: "tests/macros.rs",
            symbol: "arg",
            kind: Some(CallCandidateKind::Macro),
            callee_contains: None,
        },
        DiscoveryProbe {
            label: "proc_macro_command",
            relative_path: "tests/derive/app_name.rs",
            symbol: "command",
            kind: None,
            callee_contains: Some("MyApp::command"),
        },
        DiscoveryProbe {
            label: "trait_generic_mkeymap_get",
            relative_path: "clap_builder/src/mkeymap.rs",
            symbol: "get",
            kind: None,
            callee_contains: None,
        },
        DiscoveryProbe {
            label: "builder_command_new",
            relative_path: "tests/builder/utils.rs",
            symbol: "new",
            kind: None,
            callee_contains: Some("Command::new"),
        },
        DiscoveryProbe {
            label: "builder_command_arg",
            relative_path: "tests/builder/utils.rs",
            symbol: "arg",
            kind: None,
            callee_contains: None,
        },
        DiscoveryProbe {
            label: "builder_arg_value_parser",
            relative_path: "tests/builder/utils.rs",
            symbol: "value_parser",
            kind: Some(CallCandidateKind::Method),
            callee_contains: None,
        },
        DiscoveryProbe {
            label: "generic_arg_matches_unwrap",
            relative_path: "tests/builder/default_missing_vals.rs",
            symbol: "unwrap",
            kind: Some(CallCandidateKind::Method),
            callee_contains: None,
        },
        DiscoveryProbe {
            label: "builder_try_get_matches_from",
            relative_path: "tests/builder/default_missing_vals.rs",
            symbol: "try_get_matches_from",
            kind: Some(CallCandidateKind::Method),
            callee_contains: None,
        },
        DiscoveryProbe {
            label: "external_iterator_collect",
            relative_path: "tests/builder/utils.rs",
            symbol: "collect",
            kind: Some(CallCandidateKind::Method),
            callee_contains: None,
        },
    ]
}

fn checkout_repo(repo: ProofRepo) -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("corpus-checkouts")
        .join(repo.name);

    if root.join(".git").exists() {
        run_git(&root, &["fetch", "--quiet", "origin", repo.rev])?;
    } else {
        fs::create_dir_all(root.parent().expect("checkout parent missing"))?;
        run_git(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &[
                "clone",
                "--quiet",
                "--no-checkout",
                repo.url,
                root.to_str().expect("checkout path is not UTF-8"),
            ],
        )?;
        run_git(&root, &["fetch", "--quiet", "origin", repo.rev])?;
    }

    run_git(&root, &["checkout", "--quiet", repo.rev])?;
    run_git(&root, &["clean", "-fdx", "--quiet"])?;
    if root.join("target").exists() {
        bail!("{} still has target/ before fast discovery", root.display());
    }
    Ok(root)
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .wrap_err_with(|| format!("failed to run git {args:?} in {}", cwd.display()))?;
    if !output.status.success() {
        bail!(
            "git {args:?} failed in {}\nstdout:\n{}\nstderr:\n{}",
            cwd.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn find_probe_candidate(
    candidates: &[CallCandidate],
    probe: DiscoveryProbe,
) -> Option<&CallCandidate> {
    candidates.iter().find(|candidate| {
        candidate.path == Path::new(probe.relative_path)
            && matches_symbol(&candidate.callee_name, probe.symbol)
            && probe.kind.is_none_or(|kind| candidate.kind == kind)
            && probe
                .callee_contains
                .is_none_or(|expected| candidate.callee_name.contains(expected))
    })
}

fn matches_symbol(callee: &str, symbol: &str) -> bool {
    callee == symbol
        || callee.ends_with(&format!("::{symbol}"))
        || callee.ends_with(&format!(".{symbol}"))
        || callee.contains(&format!("::{symbol}"))
        || callee.contains(&format!(".{symbol}"))
}

fn kind_name(kind: CallCandidateKind) -> &'static str {
    match kind {
        CallCandidateKind::Function => "function",
        CallCandidateKind::Method => "method",
        CallCandidateKind::Macro => "macro",
    }
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn row_json(row: &DiscoveryRow) -> String {
    format!(
        "{{\"label\":\"{}\",\"file\":\"{}\",\"callee\":\"{}\",\"kind\":\"{}\",\"start_byte\":{},\"end_byte\":{}}}",
        json_escape(row.label),
        json_escape(&row.file),
        json_escape(&row.callee),
        kind_name(row.kind),
        row.start_byte,
        row.end_byte
    )
}

fn write_outputs(
    all_candidates: &[CallCandidate],
    proof_rows: &[DiscoveryRow],
    discovery_ms: u128,
) -> Result<()> {
    let macro_candidates = all_candidates
        .iter()
        .filter(|candidate| candidate.kind == CallCandidateKind::Macro)
        .count();
    let function_candidates = all_candidates
        .iter()
        .filter(|candidate| candidate.kind == CallCandidateKind::Function)
        .count();
    let method_candidates = all_candidates
        .iter()
        .filter(|candidate| candidate.kind == CallCandidateKind::Method)
        .count();

    let jsonl = proof_rows
        .iter()
        .map(row_json)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        "corpus/proof-discovery-candidates.jsonl",
        format!("{jsonl}\n"),
    )?;

    let mut report = String::from("# Proof Corpus Fast Discovery Report\n\n");
    report.push_str("Fast path uses syntax-only `discover_workspace_calls(ResolverConfig::discovery_only(...))`. It does not start `rust-analyzer`, load Cargo metadata, run `cargo check`, execute build scripts, or expand proc macros.\n\n");
    report.push_str("| discovery_ms | textual_call_candidates | function_candidates | method_candidates | macro_candidates | proof_rows |\n");
    report.push_str("|--------------|-------------------------|---------------------|-------------------|------------------|------------|\n");
    report.push_str(&format!(
        "| {discovery_ms} | {} | {function_candidates} | {method_candidates} | {macro_candidates} | {} |\n\n",
        all_candidates.len(),
        proof_rows.len()
    ));

    report.push_str("## Proof rows\n\n");
    report.push_str("| label | file | callee | kind | byte_range |\n");
    report.push_str("|-------|------|--------|------|------------|\n");
    for row in proof_rows {
        report.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}..{}` |\n",
            row.label,
            row.file,
            row.callee,
            kind_name(row.kind),
            row.start_byte,
            row.end_byte
        ));
    }

    report.push_str("\nFast discovery proof rows are candidate rows only, not `CBMResolvedCall` semantic edges. Full RA oracle remains available through ignored tests.\n");
    fs::write("corpus/proof-relationship-report.md", report)?;
    Ok(())
}

#[test]
fn complex_proof_corpus_fast_discovers_call_candidates() -> Result<()> {
    assert_manifest()?;
    let repo_root = checkout_repo(CLAP)?;
    let batch = discover_workspace_calls(ResolverConfig::discovery_only(&repo_root))?;

    assert_eq!(
        batch.diagnostics.analysis_mode,
        ResolverAnalysisMode::DiscoveryOnly
    );
    assert!(batch.diagnostics.load_workspace_ms.is_none());
    assert!(batch.diagnostics.discovery_ms.is_some());
    assert!(!batch.diagnostics.enable_proc_macros);
    assert!(!batch.diagnostics.enable_build_scripts);
    assert!(!batch.diagnostics.include_dependencies);
    assert!(batch.diagnostics.error.is_none());
    assert!(
        !repo_root.join("target").exists(),
        "fast discovery must not create target/"
    );
    assert!(
        batch.candidates.len() > 1_000,
        "expected broad textual call inventory, got {}",
        batch.candidates.len()
    );

    let mut proof_rows = Vec::new();
    for probe in discovery_probes() {
        let candidate = find_probe_candidate(&batch.candidates, probe).ok_or_else(|| {
            let sample = batch
                .candidates
                .iter()
                .filter(|candidate| candidate.path == Path::new(probe.relative_path))
                .take(25)
                .map(|candidate| format!("{}:{:?}", candidate.callee_name, candidate.kind))
                .collect::<Vec<_>>()
                .join(", ");
            eyre::eyre!("missing fast discovery candidate for {probe:?}; sample in file: {sample}")
        })?;
        proof_rows.push(DiscoveryRow {
            label: probe.label,
            file: candidate.path.display().to_string(),
            callee: candidate.callee_name.clone(),
            kind: candidate.kind,
            start_byte: candidate.start_byte,
            end_byte: candidate.end_byte,
        });
    }

    let macro_rows = proof_rows
        .iter()
        .filter(|row| row.kind == CallCandidateKind::Macro)
        .count();
    assert!(macro_rows >= 1, "expected at least one macro proof row");
    assert!(
        proof_rows.len() >= 8,
        "expected broad fast discovery sample, got {} rows: {proof_rows:#?}",
        proof_rows.len()
    );

    write_outputs(
        &batch.candidates,
        &proof_rows,
        batch.diagnostics.discovery_ms.unwrap_or_default(),
    )?;

    Ok(())
}
