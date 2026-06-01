use std::path::PathBuf;

use crate::{CargoTargetDirSource, RA_AP_VERSION, ResolverAnalysisMode, ResolverConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderDiagnostics {
    pub analysis_mode: ResolverAnalysisMode,
    pub workspace_root: PathBuf,
    pub cargo_manifest_path: Option<PathBuf>,
    pub cargo_target_dir: PathBuf,
    pub cargo_target_dir_source: CargoTargetDirSource,
    pub enable_proc_macros: bool,
    pub enable_build_scripts: bool,
    pub timeout_ms: Option<u64>,
    pub features: Vec<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub target_triple: Option<String>,
    pub include_tests: bool,
    pub include_examples: bool,
    pub include_benches: bool,
    pub include_dependencies: bool,
    pub ra_ap_version: &'static str,
    pub load_workspace_ms: Option<u128>,
    pub discovery_ms: Option<u128>,
    pub error: Option<String>,
}

impl LoaderDiagnostics {
    #[must_use]
    pub fn from_config(config: &ResolverConfig) -> Self {
        let (cargo_target_dir, cargo_target_dir_source) = config.effective_cargo_target_dir();
        Self {
            analysis_mode: config.analysis_mode,
            workspace_root: config.workspace_root.clone(),
            cargo_manifest_path: config.cargo_manifest_path.clone(),
            cargo_target_dir,
            cargo_target_dir_source,
            enable_proc_macros: config.enable_proc_macros,
            enable_build_scripts: config.enable_build_scripts,
            timeout_ms: config.timeout_ms,
            features: config.features.clone(),
            all_features: config.all_features,
            no_default_features: config.no_default_features,
            target_triple: config.target_triple.clone(),
            include_tests: config.include_tests,
            include_examples: config.include_examples,
            include_benches: config.include_benches,
            include_dependencies: config.include_dependencies,
            ra_ap_version: RA_AP_VERSION,
            load_workspace_ms: None,
            discovery_ms: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCallBatch {
    pub diagnostics: LoaderDiagnostics,
    pub candidates: Vec<crate::CallCandidate>,
    pub calls: Vec<ResolvedCall>,
    pub external_dependency_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCall {
    pub caller_qn: String,
    pub callee_qn: String,
    pub strategy: String,
    pub confidence_bps: u16,
    pub reason: Option<String>,
}
