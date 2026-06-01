use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let generated = out_dir.join("generated_include.rs");
    fs::write(
        generated,
        r#"
pub fn generated_out_dir_target(input: i32) -> i32 {
    input + 30
}

pub fn generated_calls_workspace(input: i32) -> i32 {
    crate::out_dir_workspace_target(input)
}
"#,
    )
    .expect("write generated_include.rs");
}
