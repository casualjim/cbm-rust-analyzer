use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use cbm_rust_analyzer::{
    CallCandidateKind, ResolverAnalysisMode, ResolverConfig, discover_workspace_calls,
};
use eyre::{Result, WrapErr, bail};

const MANIFEST_PATH: &str = "corpus/small.toml";

#[derive(Debug, Clone, Copy)]
struct CorpusRepo {
    name: &'static str,
    slug: &'static str,
    url: &'static str,
    rev: &'static str,
}

const REPOS: &[CorpusRepo] = &[
    CorpusRepo {
        name: "serde",
        slug: "serde-rs/serde",
        url: "https://github.com/serde-rs/serde.git",
        rev: "fa7da4a93567ed347ad0735c28e439fca688ef26",
    },
    CorpusRepo {
        name: "tower",
        slug: "tower-rs/tower",
        url: "https://github.com/tower-rs/tower.git",
        rev: "251296dc54a044383dffd16d2179b443e2615672",
    },
];

#[derive(Debug)]
struct RepoTiming {
    repo: &'static str,
    rev: &'static str,
    discovery_ms: u128,
    candidates: usize,
    functions: usize,
    methods: usize,
    macros: usize,
}

fn assert_manifest_scope() -> Result<()> {
    let manifest = fs::read_to_string(MANIFEST_PATH).wrap_err("failed to read corpus manifest")?;
    for repo in REPOS {
        if !manifest.contains(repo.slug) || !manifest.contains(repo.rev) {
            bail!("corpus manifest missing {} {}", repo.slug, repo.rev);
        }
    }

    Ok(())
}

fn checkout_repo(repo: CorpusRepo) -> Result<PathBuf> {
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

fn smoke_repo(repo: CorpusRepo) -> Result<RepoTiming> {
    let repo_root = checkout_repo(repo)?;
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
        !batch.candidates.is_empty(),
        "{} yielded no fast discovery candidates",
        repo.slug
    );
    assert!(
        !repo_root.join("target").exists(),
        "fast discovery must not create target/ for {}",
        repo.slug
    );

    let functions = batch
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == CallCandidateKind::Function)
        .count();
    let methods = batch
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == CallCandidateKind::Method)
        .count();
    let macros = batch
        .candidates
        .iter()
        .filter(|candidate| candidate.kind == CallCandidateKind::Macro)
        .count();

    assert!(functions + methods + macros == batch.candidates.len());
    assert!(
        methods > 0 || functions > 0 || macros > 0,
        "{} yielded no typed fast discovery candidates",
        repo.slug
    );

    Ok(RepoTiming {
        repo: repo.slug,
        rev: repo.rev,
        discovery_ms: batch.diagnostics.discovery_ms.unwrap_or_default(),
        candidates: batch.candidates.len(),
        functions,
        methods,
        macros,
    })
}

fn write_report(timings: &[RepoTiming]) -> Result<()> {
    let mut report = String::from(
        "# Small Corpus Fast Discovery Report\n\n\
         Clean-checkout syntax-only proof. Each repo starts with `target/` absent. Fast path uses `discover_workspace_calls(ResolverConfig::discovery_only(...))`, not rust-analyzer LSP, Cargo metadata, `cargo check`, build scripts, or proc macros.\n\n\
         | repo | rev | discovery_ms | candidates | functions | methods | macros |\n\
         |------|-----|--------------|------------|-----------|---------|--------|\n",
    );
    for timing in timings {
        report.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} | {} |\n",
            timing.repo,
            timing.rev,
            timing.discovery_ms,
            timing.candidates,
            timing.functions,
            timing.methods,
            timing.macros
        ));
    }

    report.push_str(
        "\nNotes:\n\
         - `discovery_ms` = syntax scan time only.\n\
         - no semantic resolution, goto-definition, hover, call hierarchy, proc-macro expansion, or OUT_DIR include proof in this fast path.\n",
    );
    fs::write("corpus/small-timing-report.md", report)?;
    Ok(())
}

#[test]
fn small_corpus_clean_checkouts_fast_discover_call_candidates() -> Result<()> {
    assert_manifest_scope()?;
    let timings = REPOS
        .iter()
        .copied()
        .map(smoke_repo)
        .collect::<Result<Vec<_>>>()?;

    for timing in &timings {
        eprintln!(
            "{}: discovery={}ms candidates={} functions={} methods={} macros={}",
            timing.repo,
            timing.discovery_ms,
            timing.candidates,
            timing.functions,
            timing.methods,
            timing.macros
        );
    }

    write_report(&timings)?;
    Ok(())
}
