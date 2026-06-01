use std::{
    io,
    path::{Path, PathBuf},
    time::Instant,
};

use camino::Utf8PathBuf;
use eyre::eyre;
use ra_ap_ide::{Analysis, AnalysisHost, FileId, RootDatabase};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, CargoFeatures, TargetDirectoryConfig};
use ra_ap_vfs::{Vfs, VfsPath};

use crate::{
    CbmCallInput, LoaderDiagnostics, ResolvedCallBatch, ResolverConfig,
    collect_workspace_call_candidates, collect_workspace_def_sites, resolve_cbm_calls_by_def_sites,
};

pub struct RustAnalyzerWorkspace {
    db: RootDatabase,
    vfs: Vfs,
    diagnostics: LoaderDiagnostics,
}

impl RustAnalyzerWorkspace {
    #[must_use]
    pub fn diagnostics(&self) -> &LoaderDiagnostics {
        &self.diagnostics
    }

    #[must_use]
    pub fn database(&self) -> &RootDatabase {
        &self.db
    }

    #[must_use]
    pub fn analysis(&self) -> Analysis {
        AnalysisHost::with_database(self.db.clone()).analysis()
    }

    #[must_use]
    pub fn file_id_for_relative_path(&self, rel_path: &Path) -> Option<FileId> {
        let abs_path = self.diagnostics.workspace_root.join(rel_path);
        let vfs_path = VfsPath::new_real_path(abs_path.to_string_lossy().into_owned());
        self.vfs.file_id(&vfs_path).map(|(file_id, _)| file_id)
    }

    #[must_use]
    pub fn relative_path_for_file_id(&self, file_id: FileId) -> Option<PathBuf> {
        let abs_path = self.vfs.file_path(file_id).as_path()?;
        let std_path: &Path = abs_path.as_ref();
        std_path
            .strip_prefix(&self.diagnostics.workspace_root)
            .ok()
            .map(Path::to_path_buf)
    }

    #[must_use]
    pub fn one_based_line_for_offset(&self, file_id: FileId, offset: u32) -> Option<u32> {
        let abs_path = self.vfs.file_path(file_id).as_path()?;
        let std_path: &Path = abs_path.as_ref();
        let source = std::fs::read_to_string(std_path).ok()?;
        let byte_offset = usize::try_from(offset).ok()?.min(source.len());
        Some(
            source[..byte_offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count() as u32
                + 1,
        )
    }

    #[must_use]
    pub fn byte_offset_for_one_based_line(&self, rel_path: &Path, line: u32) -> Option<u32> {
        if line == 0 {
            return None;
        }
        let source =
            std::fs::read_to_string(self.diagnostics.workspace_root.join(rel_path)).ok()?;
        if line == 1 {
            return Some(0);
        }
        let mut current_line = 1_u32;
        for (idx, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                current_line += 1;
                if current_line == line {
                    return u32::try_from(idx + 1).ok();
                }
            }
        }
        None
    }

    #[must_use]
    pub fn source_for_relative_path(&self, rel_path: &Path) -> Option<String> {
        std::fs::read_to_string(self.diagnostics.workspace_root.join(rel_path)).ok()
    }

    #[must_use]
    pub fn byte_offset_for_one_based_line_name(
        &self,
        rel_path: &Path,
        line: u32,
        name: &str,
    ) -> Option<u32> {
        let source =
            std::fs::read_to_string(self.diagnostics.workspace_root.join(rel_path)).ok()?;
        let line_start = self.byte_offset_for_one_based_line(rel_path, line)?;
        let line_index = usize::try_from(line.checked_sub(1)?).ok()?;
        let line_text = source.lines().nth(line_index)?;
        let in_line = line_text.find(name)?;
        Some(line_start + u32::try_from(in_line).ok()?)
    }

    pub fn collect_workspace_call_candidates(
        &self,
        root: &Path,
    ) -> std::io::Result<Vec<crate::CallCandidate>> {
        collect_workspace_call_candidates(root)
    }

    pub fn resolve_workspace_calls(&self, root: &Path) -> std::io::Result<ResolvedCallBatch> {
        let started = Instant::now();
        let candidates = self.collect_workspace_call_candidates(root)?;
        let owned_def_sites = collect_workspace_def_sites(root)?;
        let def_sites = owned_def_sites
            .iter()
            .map(crate::OwnedRustDefSite::as_borrowed)
            .collect::<Vec<_>>();
        let call_inputs = def_sites
            .iter()
            .map(|site| CbmCallInput {
                callee_name: "*",
                enclosing_func_qn: site.qualified_name,
            })
            .collect::<Vec<_>>();
        let calls = resolve_cbm_calls_by_def_sites(self, &call_inputs, &def_sites)
            .map_err(|err| std::io::Error::other(format!("semantic resolve failed: {err:?}")))?;
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.discovery_ms = Some(started.elapsed().as_millis());
        Ok(ResolvedCallBatch {
            diagnostics,
            candidates,
            calls,
            external_dependency_count: 0,
        })
    }
}

pub fn discover_workspace_calls(config: ResolverConfig) -> io::Result<ResolvedCallBatch> {
    let mut diagnostics = LoaderDiagnostics::from_config(&config);
    let started = Instant::now();
    let candidates = collect_workspace_call_candidates(&config.workspace_root)?;
    diagnostics.discovery_ms = Some(started.elapsed().as_millis());
    Ok(ResolvedCallBatch {
        diagnostics,
        candidates,
        calls: Vec::new(),
        external_dependency_count: 0,
    })
}

pub fn load_workspace(config: ResolverConfig) -> Result<RustAnalyzerWorkspace, LoaderDiagnostics> {
    let mut diagnostics = LoaderDiagnostics::from_config(&config);
    let started = Instant::now();

    let cargo_config = match cargo_config_from_resolver(&config, &diagnostics) {
        Ok(config) => config,
        Err(error) => {
            diagnostics.error = Some(error.to_string());
            return Err(diagnostics);
        }
    };
    let load_config = LoadCargoConfig {
        load_out_dirs_from_check: config.enable_build_scripts,
        with_proc_macro_server: if config.enable_proc_macros {
            ProcMacroServerChoice::Sysroot
        } else {
            ProcMacroServerChoice::None
        },
        prefill_caches: false,
        num_worker_threads: 1,
        proc_macro_processes: 1,
    };

    let root = config
        .cargo_manifest_path
        .as_deref()
        .unwrap_or(config.workspace_root.as_path());

    match load_workspace_at(root, &cargo_config, &load_config, &|_| {}) {
        Ok((db, vfs, _proc_macro_client)) => {
            diagnostics.load_workspace_ms = Some(started.elapsed().as_millis());
            Ok(RustAnalyzerWorkspace {
                db,
                vfs,
                diagnostics,
            })
        }
        Err(error) => {
            diagnostics.load_workspace_ms = Some(started.elapsed().as_millis());
            diagnostics.error = Some(error.to_string());
            Err(diagnostics)
        }
    }
}

pub(crate) fn cargo_config_from_resolver(
    config: &ResolverConfig,
    diagnostics: &LoaderDiagnostics,
) -> eyre::Result<CargoConfig> {
    let mut cargo = CargoConfig {
        all_targets: config.include_tests || config.include_examples || config.include_benches,
        features: if config.all_features {
            CargoFeatures::All
        } else {
            CargoFeatures::Selected {
                features: config.features.clone(),
                no_default_features: config.no_default_features,
            }
        },
        target: config.target_triple.clone(),
        no_deps: !config.include_dependencies,
        ..Default::default()
    };

    let utf8_target_dir = Utf8PathBuf::from_path_buf(diagnostics.cargo_target_dir.clone())
        .map_err(|path| eyre!("cargo target dir is not valid UTF-8: {}", path.display()))?;
    cargo.target_dir_config = TargetDirectoryConfig::Directory(utf8_target_dir);
    Ok(cargo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CargoTargetDirSource, ResolverAnalysisMode};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn cargo_config_maps_policy() {
        let mut config = ResolverConfig::trusted_full("/tmp/example");
        config.features = vec!["one".to_owned(), "two".to_owned()];
        config.no_default_features = true;
        config.target_triple = Some("x86_64-unknown-linux-gnu".to_owned());
        config.include_dependencies = false;
        let diagnostics = LoaderDiagnostics::from_config(&config);
        let cargo = cargo_config_from_resolver(&config, &diagnostics).unwrap();

        assert!(cargo.all_targets);
        assert_eq!(cargo.target.as_deref(), Some("x86_64-unknown-linux-gnu"));
        assert!(cargo.no_deps);
        match cargo.features {
            CargoFeatures::Selected {
                features,
                no_default_features,
            } => {
                assert_eq!(features, vec!["one", "two"]);
                assert!(no_default_features);
            }
            CargoFeatures::All => panic!("expected selected features"),
        }
    }

    #[test]
    fn discover_workspace_calls_skips_cargo_load() {
        let root = temp_workspace("discovery-no-cargo");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "struct Thing; impl Thing { fn work(&self) {} } fn run(t: Thing) { helper(); t.work(); } fn helper() {}",
        )
        .unwrap();

        let batch = discover_workspace_calls(ResolverConfig::discovery_only(&root)).unwrap();

        assert_eq!(
            batch.diagnostics.analysis_mode,
            ResolverAnalysisMode::DiscoveryOnly
        );
        assert!(batch.diagnostics.discovery_ms.is_some());
        assert!(batch.diagnostics.load_workspace_ms.is_none());
        assert!(
            batch
                .candidates
                .iter()
                .any(|candidate| candidate.callee_name == "helper")
        );
        assert!(
            batch
                .candidates
                .iter()
                .any(|candidate| candidate.callee_name == "work")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discover_workspace_calls_works_for_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate");
        let batch = discover_workspace_calls(ResolverConfig::discovery_only(&fixture)).unwrap();

        assert!(
            batch
                .candidates
                .iter()
                .any(|candidate| candidate.callee_name == "direct_target")
        );
        assert!(batch.diagnostics.error.is_none());
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cbm-rust-analyzer-{name}-{unique}"))
    }

    #[test]
    fn load_workspace_succeeds_for_basic_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate");
        let workspace = load_workspace(ResolverConfig::trusted_full(&fixture))
            .expect("basic fixture should load through rust-analyzer crates");

        assert_eq!(workspace.diagnostics().workspace_root, fixture);
        assert!(workspace.diagnostics().load_workspace_ms.is_some());
        assert!(workspace.diagnostics().error.is_none());
        assert!(
            workspace
                .diagnostics()
                .cargo_target_dir
                .ends_with("target/cbm-rust-analyzer")
        );
    }

    #[test]
    fn resolve_workspace_calls_emits_ra_backed_edges_for_fixture() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate");
        let workspace = load_workspace(ResolverConfig::trusted_full(&fixture))
            .expect("basic fixture should load through rust-analyzer crates");

        let batch = workspace
            .resolve_workspace_calls(&fixture)
            .expect("workspace batch resolution should succeed");
        let edges = batch
            .calls
            .iter()
            .map(|call| (call.caller_qn.as_str(), call.callee_qn.as_str()))
            .collect::<Vec<_>>();

        assert!(!batch.candidates.is_empty());
        assert!(batch.diagnostics.discovery_ms.is_some());
        assert!(edges.contains(&(
            "cbm_ra_fixture_basic.exercise_all_cases",
            "cbm_ra_fixture_basic.direct_target"
        )));
        assert!(edges.contains(&(
            "cbm_ra_fixture_basic.exercise_all_cases",
            "cbm_ra_fixture_basic.FriendlyGreeter.trait_target"
        )));
        assert!(edges.contains(&(
            "cbm_ra_fixture_basic.exercise_all_cases",
            "cbm_ra_fixture_basic.macro_target_mod.MacroReceiver.macro_cross_file_target"
        )));
        assert!(edges.contains(&(
            "cbm_ra_fixture_basic.exercise_all_cases",
            "cbm_ra_fixture_basic.macro_generated_method_mod.MacroGeneratedReceiver.macro_generated_cross_file_target"
        )));
        assert!(
            batch
                .calls
                .iter()
                .all(|call| call.confidence_bps >= 6000 && call.reason.is_none())
        );
    }

    #[test]
    fn load_workspace_failure_preserves_effective_policy() {
        let config = ResolverConfig::trusted_full("/definitely/not/a/workspace");
        let err = match load_workspace(config) {
            Ok(_) => panic!("invalid workspace should fail"),
            Err(err) => err,
        };

        assert!(err.enable_proc_macros);
        assert!(err.enable_build_scripts);
        assert_eq!(
            err.cargo_target_dir_source,
            CargoTargetDirSource::DefaultIsolated
        );
        assert!(err.cargo_target_dir.ends_with("target/cbm-rust-analyzer"));
        assert!(err.load_workspace_ms.is_some());
        assert!(err.error.is_some());
    }
}
