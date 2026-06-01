use proc_macro::TokenStream;

#[proc_macro_derive(ProcMacroTarget)]
pub fn derive_proc_macro_target(input: TokenStream) -> TokenStream {
    let source = input.to_string();
    let parts = source.split_whitespace().collect::<Vec<_>>();
    let name = parts
        .windows(2)
        .find_map(|window| (window[0] == "struct").then_some(window[1]))
        .unwrap_or("GeneratedByDerive")
        .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_');

    format!("impl {name} {{ pub fn proc_macro_target(&self) -> i32 {{ 17 }} }}")
        .parse()
        .expect("generated proc macro target should parse")
}
