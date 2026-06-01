use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspOracleBinary {
    pub path: PathBuf,
    pub version: String,
}

pub fn check_lsp_oracle_binary() -> std::io::Result<LspOracleBinary> {
    let output = std::process::Command::new("rust-analyzer")
        .arg("--version")
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("rust-analyzer --version failed"));
    }

    Ok(LspOracleBinary {
        path: PathBuf::from("rust-analyzer"),
        version: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    })
}
