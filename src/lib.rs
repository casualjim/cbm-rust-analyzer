mod capi;
pub mod config;
pub mod error;
pub mod loader;
pub mod resolve;
pub mod result;

pub use config::{CargoTargetDirSource, ResolverAnalysisMode, ResolverConfig};
pub use loader::{RustAnalyzerWorkspace, discover_workspace_calls, load_workspace};
pub use resolve::{
    CallCandidate, CallCandidateKind, CbmCallInput, OwnedRustDefSite, RustDefSite,
    SemanticResolveError, collect_file_call_candidates, collect_workspace_call_candidates,
    collect_workspace_def_sites, resolve_cbm_calls_by_def_sites,
};
pub use result::{LoaderDiagnostics, ResolvedCall, ResolvedCallBatch};

pub const RA_AP_VERSION: &str = "0.0.334";
