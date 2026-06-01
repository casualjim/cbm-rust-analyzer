use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverAnalysisMode {
    TrustedFull,
    DiscoveryOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoTargetDirSource {
    Configured,
    DefaultIsolated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverConfig {
    pub analysis_mode: ResolverAnalysisMode,
    pub workspace_root: PathBuf,
    pub cargo_manifest_path: Option<PathBuf>,
    pub cargo_target_dir: Option<PathBuf>,
    pub target_triple: Option<String>,
    pub features: Vec<String>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub include_tests: bool,
    pub include_examples: bool,
    pub include_benches: bool,
    pub include_dependencies: bool,
    pub enable_proc_macros: bool,
    pub enable_build_scripts: bool,
    pub timeout_ms: Option<u64>,
}

impl ResolverConfig {
    #[must_use]
    pub fn trusted_full(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            analysis_mode: ResolverAnalysisMode::TrustedFull,
            workspace_root: workspace_root.into(),
            cargo_manifest_path: None,
            cargo_target_dir: None,
            target_triple: None,
            features: Vec::new(),
            all_features: false,
            no_default_features: false,
            include_tests: true,
            include_examples: true,
            include_benches: true,
            include_dependencies: true,
            enable_proc_macros: true,
            enable_build_scripts: true,
            timeout_ms: Some(20_000),
        }
    }

    #[must_use]
    pub fn discovery_only(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            analysis_mode: ResolverAnalysisMode::DiscoveryOnly,
            workspace_root: workspace_root.into(),
            cargo_manifest_path: None,
            cargo_target_dir: None,
            target_triple: None,
            features: Vec::new(),
            all_features: false,
            no_default_features: false,
            include_tests: false,
            include_examples: false,
            include_benches: false,
            include_dependencies: false,
            enable_proc_macros: false,
            enable_build_scripts: false,
            timeout_ms: None,
        }
    }

    #[must_use]
    pub fn effective_cargo_target_dir(&self) -> (PathBuf, CargoTargetDirSource) {
        if let Some(dir) = &self.cargo_target_dir {
            return (dir.clone(), CargoTargetDirSource::Configured);
        }
        (
            self.workspace_root.join("target").join("cbm-rust-analyzer"),
            CargoTargetDirSource::DefaultIsolated,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LoaderDiagnostics;
    use std::path::{Path, PathBuf};

    #[test]
    fn trusted_full_defaults_record_execution_policy() {
        let config = ResolverConfig::trusted_full("/tmp/example");
        let diagnostics = LoaderDiagnostics::from_config(&config);

        assert_eq!(diagnostics.analysis_mode, ResolverAnalysisMode::TrustedFull);
        assert!(diagnostics.enable_proc_macros);
        assert!(diagnostics.enable_build_scripts);
        assert_eq!(diagnostics.timeout_ms, Some(20_000));
        assert!(diagnostics.include_tests);
        assert!(diagnostics.include_examples);
        assert!(diagnostics.include_benches);
        assert!(diagnostics.include_dependencies);
    }

    #[test]
    fn discovery_only_defaults_skip_cargo_heavy_work() {
        let config = ResolverConfig::discovery_only("/tmp/example");
        let diagnostics = LoaderDiagnostics::from_config(&config);

        assert_eq!(
            diagnostics.analysis_mode,
            ResolverAnalysisMode::DiscoveryOnly
        );
        assert!(!diagnostics.enable_proc_macros);
        assert!(!diagnostics.enable_build_scripts);
        assert_eq!(diagnostics.timeout_ms, None);
        assert!(!diagnostics.include_tests);
        assert!(!diagnostics.include_examples);
        assert!(!diagnostics.include_benches);
        assert!(!diagnostics.include_dependencies);
    }

    #[test]
    fn default_target_dir_is_isolated_and_reported() {
        let config = ResolverConfig::trusted_full("/tmp/example");
        let diagnostics = LoaderDiagnostics::from_config(&config);

        assert_eq!(
            diagnostics.cargo_target_dir,
            Path::new("/tmp/example")
                .join("target")
                .join("cbm-rust-analyzer")
        );
        assert_eq!(
            diagnostics.cargo_target_dir_source,
            CargoTargetDirSource::DefaultIsolated
        );
    }

    #[test]
    fn configured_target_dir_is_reported() {
        let mut config = ResolverConfig::trusted_full("/tmp/example");
        config.cargo_target_dir = Some(PathBuf::from("/tmp/cbm-ra-target"));
        let diagnostics = LoaderDiagnostics::from_config(&config);

        assert_eq!(
            diagnostics.cargo_target_dir,
            PathBuf::from("/tmp/cbm-ra-target")
        );
        assert_eq!(
            diagnostics.cargo_target_dir_source,
            CargoTargetDirSource::Configured
        );
    }
}
