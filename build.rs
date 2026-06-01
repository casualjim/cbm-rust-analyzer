use std::{env, path::PathBuf};

fn main() {
    let cbm_root =
        env::var("CBM_MCP_ROOT").expect("CBM_MCP_ROOT must point to codebase-memory-mcp");
    let cbm_include = PathBuf::from(&cbm_root).join("internal/cbm");
    let tree_sitter_include = cbm_include.join("vendored/ts_runtime/include");

    println!("cargo:rerun-if-env-changed=CBM_MCP_ROOT");
    println!("cargo:rerun-if-changed=include/cbm_bindings.h");
    println!(
        "cargo:rerun-if-changed={}",
        cbm_include.join("cbm.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        cbm_include.join("lsp/go_lsp.h").display()
    );
    println!("cargo:rerun-if-changed=include/cbm_rust_lsp.h");

    let bindings = bindgen::Builder::default()
        .header("include/cbm_bindings.h")
        .clang_arg(format!("-I{}", cbm_include.display()))
        .clang_arg("-Iinclude")
        .clang_arg(format!("-I{}", tree_sitter_include.display()))
        .allowlist_type("CBMArena")
        .allowlist_type("CBMCallArg")
        .allowlist_type("CBMCall")
        .allowlist_type("CBMImport")
        .allowlist_type("CBMLSPDef")
        .allowlist_type("CBMResolvedCall")
        .allowlist_type("CBMResolvedCallArray")
        .allowlist_type("CBMRustLSPDefSite")
        .allowlist_type("CBMLanguage")
        .allowlist_var("CBM_MAX_CALL_ARGS")
        .layout_tests(false)
        .generate()
        .expect("failed to generate CBM bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR missing"));
    bindings
        .write_to_file(out_path.join("cbm_bindings.rs"))
        .expect("failed to write CBM bindings");
}
