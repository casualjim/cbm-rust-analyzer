use std::{fs, path::Path};

#[test]
fn generated_c_header_matches_checked_in_artifact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config =
        cbindgen::Config::from_file(root.join("cbindgen.toml")).expect("cbindgen.toml should load");
    let generated_path = root.join("target/tmp/rust_analyzer_lsp.generated.h");
    fs::create_dir_all(generated_path.parent().unwrap()).unwrap();
    cbindgen::generate_with_config(root, config)
        .expect("C ABI header generation should succeed")
        .write_to_file(&generated_path);

    let generated = fs::read_to_string(&generated_path).unwrap();
    let checked_in = fs::read_to_string(root.join("include/rust_analyzer_lsp.h")).unwrap();
    assert_eq!(
        generated, checked_in,
        "include/rust_analyzer_lsp.h drifted; run `mise run abi:generate-header` and commit regenerated artifact"
    );

    assert!(generated.contains("rust_analyzer_resolve_batch"));
    assert!(generated.contains("rust_analyzer_free_resolved_call_array"));
    for forbidden in [
        "CBM",
        "cbm_",
        "CBM_MCP_ROOT",
        "codebase-memory-mcp",
        "cbm.h",
        "go_lsp.h",
        "rust_analyzer_resolve_batch_v2",
    ] {
        assert!(
            !generated.contains(forbidden),
            "generated header contains forbidden CBM/legacy surface {forbidden}"
        );
    }
}
