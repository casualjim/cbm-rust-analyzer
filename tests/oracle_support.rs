mod support {
    pub mod oracle;
}

use support::oracle::check_lsp_oracle_binary;

#[test]
fn lsp_oracle_binary_check_reports_version() {
    let oracle = check_lsp_oracle_binary().expect("rust-analyzer must be on PATH for oracle tests");

    assert!(oracle.version.contains("rust-analyzer"));
}
