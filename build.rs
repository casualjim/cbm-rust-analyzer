use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir missing"));
    if env::var_os("RUST_ANALYZER_GENERATE_HEADER").is_some() {
        generate_c_header(&manifest_dir);
    }

    println!("cargo:rerun-if-env-changed=RUST_ANALYZER_GENERATE_HEADER");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/capi.rs");
}

fn generate_c_header(manifest_dir: &PathBuf) {
    let config_path = manifest_dir.join("cbindgen.toml");
    let config = cbindgen::Config::from_file(&config_path).expect("failed to load cbindgen.toml");
    cbindgen::generate_with_config(manifest_dir, config)
        .expect("failed to generate Rust C ABI header")
        .write_to_file(manifest_dir.join("include/rust_analyzer_lsp.h"));
}
