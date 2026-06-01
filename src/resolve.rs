use std::path::{Path, PathBuf};

use ra_ap_ide::{CallHierarchyConfig, FilePosition};
use ra_ap_syntax::{
    Edition, SourceFile, SyntaxKind, TextSize,
    ast::{self, AstNode},
};

use crate::{ResolvedCall, RustAnalyzerWorkspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallCandidate {
    pub path: PathBuf,
    pub callee_name: String,
    pub start_byte: u32,
    pub end_byte: u32,
    pub kind: CallCandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallCandidateKind {
    Function,
    Method,
    Macro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CbmCallInput<'a> {
    pub callee_name: &'a str,
    pub enclosing_func_qn: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustDefSite<'a> {
    pub qualified_name: &'a str,
    pub rel_path: &'a Path,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedRustDefSite {
    pub qualified_name: String,
    pub rel_path: PathBuf,
    pub start_line: u32,
    pub end_line: u32,
}

impl OwnedRustDefSite {
    #[must_use]
    pub fn as_borrowed(&self) -> RustDefSite<'_> {
        RustDefSite {
            qualified_name: &self.qualified_name,
            rel_path: &self.rel_path,
            start_line: self.start_line,
            end_line: self.end_line,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticResolveError {
    FileNotInWorkspace(PathBuf),
    Analysis(String),
}

pub fn resolve_cbm_calls_by_def_sites(
    workspace: &RustAnalyzerWorkspace,
    calls: &[CbmCallInput<'_>],
    def_sites: &[RustDefSite<'_>],
) -> Result<Vec<ResolvedCall>, SemanticResolveError> {
    let analysis = workspace.analysis();
    let config = CallHierarchyConfig {
        exclude_tests: false,
        ra_fixture: ra_ap_ide::RaFixtureConfig::default(),
    };
    let mut out = Vec::new();

    for caller_qn in unique_callers(calls) {
        let Some(caller_site) = def_sites
            .iter()
            .find(|site| site.qualified_name == caller_qn)
        else {
            continue;
        };
        let Some(file_id) = workspace.file_id_for_relative_path(caller_site.rel_path) else {
            continue;
        };
        let Some(offset) = def_site_name_offset(workspace, caller_site) else {
            continue;
        };
        let Some(items) = analysis
            .outgoing_calls(
                &config,
                FilePosition {
                    file_id,
                    offset: TextSize::from(offset),
                },
            )
            .map_err(|err| SemanticResolveError::Analysis(format!("{err:?}")))?
        else {
            continue;
        };

        append_call_items(
            workspace,
            &mut out,
            caller_qn,
            items,
            def_sites,
            "rust_analyzer_call_hierarchy",
        );

        if let Some(source) = workspace.source_for_relative_path(caller_site.rel_path) {
            for candidate in collect_file_call_candidates(caller_site.rel_path, &source)
                .into_iter()
                .filter(|candidate| candidate.kind == CallCandidateKind::Macro)
                .filter(|candidate| {
                    let line = one_based_line_for_source_offset(&source, candidate.start_byte);
                    caller_site.start_line <= line && line <= caller_site.end_line
                })
            {
                for offset in macro_probe_offsets(&candidate) {
                    let Some(items) = analysis
                        .outgoing_calls(&config, FilePosition { file_id, offset })
                        .map_err(|err| SemanticResolveError::Analysis(format!("{err:?}")))?
                    else {
                        continue;
                    };
                    append_call_items(
                        workspace,
                        &mut out,
                        caller_qn,
                        items,
                        def_sites,
                        "rust_analyzer_macro_call_hierarchy",
                    );
                }
                for offset in macro_probe_offsets(&candidate) {
                    let Some(expanded) = analysis
                        .expand_macro(FilePosition { file_id, offset })
                        .map_err(|err| SemanticResolveError::Analysis(format!("{err:?}")))?
                    else {
                        continue;
                    };
                    for callee_qn in resolve_cross_file_methods_from_macro_expansion(
                        &expanded.expansion,
                        def_sites,
                    ) {
                        out.push(ResolvedCall {
                            caller_qn: caller_qn.to_owned(),
                            callee_qn,
                            strategy: "rust_analyzer_macro_expansion".to_owned(),
                            confidence_bps: 9000,
                            reason: None,
                        });
                    }
                }
            }
        }
    }

    for callee_site in def_sites {
        if !def_site_has_unique_range(callee_site, def_sites) {
            continue;
        }
        let Some(file_id) = workspace.file_id_for_relative_path(callee_site.rel_path) else {
            continue;
        };
        let Some(offset) = def_site_name_offset(workspace, callee_site) else {
            continue;
        };
        let Some(items) = analysis
            .incoming_calls(
                &config,
                FilePosition {
                    file_id,
                    offset: TextSize::from(offset),
                },
            )
            .map_err(|err| SemanticResolveError::Analysis(format!("{err:?}")))?
        else {
            continue;
        };

        for item in items {
            let caller_range = item.target.focus_or_full_range();
            let Some(caller_qn) = map_nav_target_to_def_site(
                workspace,
                item.target.file_id,
                caller_range.start().into(),
                def_sites,
            ) else {
                continue;
            };
            if !calls.iter().any(|call| call.enclosing_func_qn == caller_qn) {
                continue;
            }
            out.push(ResolvedCall {
                caller_qn: caller_qn.to_owned(),
                callee_qn: callee_site.qualified_name.to_owned(),
                strategy: "rust_analyzer_call_hierarchy_incoming".to_owned(),
                confidence_bps: 9000,
                reason: None,
            });
        }
    }

    out.sort_by(|left, right| {
        left.caller_qn
            .cmp(&right.caller_qn)
            .then(left.callee_qn.cmp(&right.callee_qn))
    });
    out.dedup_by(|left, right| {
        left.caller_qn == right.caller_qn && left.callee_qn == right.callee_qn
    });
    Ok(out)
}

fn resolve_cross_file_methods_from_macro_expansion(
    expansion: &str,
    def_sites: &[RustDefSite<'_>],
) -> Vec<String> {
    let compact = expansion.replace(['\n', '\r', '\t'], " ");
    let mut out = Vec::new();
    for def_site in def_sites {
        let Some((type_qn, method_name)) = def_site.qualified_name.rsplit_once('.') else {
            continue;
        };
        let Some((module_qn, type_name)) = type_qn.rsplit_once('.') else {
            continue;
        };
        let module_path = module_qn.replace('.', "::");
        let relative_module_path = module_qn
            .split_once('.')
            .map(|(_, relative)| relative.replace('.', "::"));
        let type_bindings = [
            format!("crate::{module_path}::{type_name}"),
            relative_module_path
                .map(|relative| format!("crate::{relative}::{type_name}"))
                .unwrap_or_default(),
        ];
        let method_call = format!(".{method_name}(");
        if type_bindings
            .iter()
            .any(|binding| !binding.is_empty() && compact.contains(binding))
            && compact.contains(&method_call)
        {
            out.push(def_site.qualified_name.to_owned());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn one_based_line_for_source_offset(source: &str, offset: u32) -> u32 {
    let byte_offset = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(source.len());
    source[..byte_offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count() as u32
        + 1
}

fn macro_probe_offsets(candidate: &CallCandidate) -> Vec<TextSize> {
    let mut offsets = vec![candidate.start_byte];
    if candidate.end_byte > candidate.start_byte + 1 {
        offsets.push(candidate.start_byte + 1);
        offsets.push(candidate.end_byte - 1);
    }
    offsets.sort_unstable();
    offsets.dedup();
    offsets.into_iter().map(TextSize::from).collect()
}

fn append_call_items(
    workspace: &RustAnalyzerWorkspace,
    out: &mut Vec<ResolvedCall>,
    caller_qn: &str,
    items: Vec<ra_ap_ide::CallItem>,
    def_sites: &[RustDefSite<'_>],
    strategy: &str,
) {
    for item in items {
        let target_range = item.target.focus_or_full_range();
        let Some(callee_qn) = map_nav_target_to_def_site(
            workspace,
            item.target.file_id,
            target_range.start().into(),
            def_sites,
        ) else {
            continue;
        };
        out.push(ResolvedCall {
            caller_qn: caller_qn.to_owned(),
            callee_qn: callee_qn.to_owned(),
            strategy: strategy.to_owned(),
            confidence_bps: 9000,
            reason: None,
        });
    }
}

fn unique_callers<'a>(calls: &'a [CbmCallInput<'a>]) -> Vec<&'a str> {
    let mut callers = calls
        .iter()
        .map(|call| call.enclosing_func_qn)
        .collect::<Vec<_>>();
    callers.sort_unstable();
    callers.dedup();
    callers
}

fn def_site_has_unique_range(site: &RustDefSite<'_>, def_sites: &[RustDefSite<'_>]) -> bool {
    def_sites
        .iter()
        .filter(|candidate| {
            candidate.rel_path == site.rel_path
                && candidate.start_line == site.start_line
                && candidate.end_line == site.end_line
        })
        .count()
        == 1
}

fn def_site_name_offset(workspace: &RustAnalyzerWorkspace, site: &RustDefSite<'_>) -> Option<u32> {
    let name = short_name(site.qualified_name);
    workspace
        .byte_offset_for_one_based_line_name(site.rel_path, site.start_line, name)
        .or_else(|| workspace.byte_offset_for_one_based_line(site.rel_path, site.start_line))
}

fn map_nav_target_to_def_site<'a>(
    workspace: &RustAnalyzerWorkspace,
    file_id: ra_ap_ide::FileId,
    offset: u32,
    def_sites: &'a [RustDefSite<'_>],
) -> Option<&'a str> {
    let rel_path = workspace.relative_path_for_file_id(file_id)?;
    let line = workspace.one_based_line_for_offset(file_id, offset)?;
    let matches = def_sites
        .iter()
        .filter(|site| {
            site.rel_path == rel_path && site.start_line <= line && line <= site.end_line
        })
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        Some(matches[0].qualified_name)
    } else {
        None
    }
}

fn short_name(value: &str) -> &str {
    value
        .rsplit(['.', ':'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(value)
}

pub fn collect_workspace_call_candidates(root: &Path) -> std::io::Result<Vec<CallCandidate>> {
    let mut out = Vec::new();
    collect_workspace_call_candidates_inner(root, root, &mut out)?;
    Ok(out)
}

pub fn collect_workspace_def_sites(root: &Path) -> std::io::Result<Vec<OwnedRustDefSite>> {
    let crate_name = crate_name_for_root(root)?;
    let mut out = Vec::new();
    collect_workspace_def_sites_inner(root, root, &crate_name, &mut out)?;
    out.sort_by(|left, right| left.qualified_name.cmp(&right.qualified_name));
    out.dedup_by(|left, right| left.qualified_name == right.qualified_name);
    Ok(out)
}

fn crate_name_for_root(root: &Path) -> std::io::Result<String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_package = false;
        }
        if in_package && trimmed.starts_with("name") {
            let Some((_, value)) = trimmed.split_once('=') else {
                continue;
            };
            return Ok(value.trim().trim_matches('"').replace('-', "_"));
        }
    }
    Ok(root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("crate")
        .replace('-', "_"))
}

fn collect_workspace_def_sites_inner(
    root: &Path,
    path: &Path,
    crate_name: &str,
    out: &mut Vec<OwnedRustDefSite>,
) -> std::io::Result<()> {
    if path != root && should_skip_path(root, path) {
        return Ok(());
    }
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            collect_workspace_def_sites_inner(root, &entry.path(), crate_name, out)?;
        }
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
        let source = std::fs::read_to_string(path)?;
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let module_qn = module_qn_for_path(crate_name, &relative);
        out.extend(collect_file_def_sites(&relative, &module_qn, &source));
    }
    Ok(())
}

fn module_qn_for_path(crate_name: &str, relative: &Path) -> String {
    let mut parts = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    if parts.first() == Some(&"src") {
        parts.remove(0);
    }
    if parts.last() == Some(&"lib.rs") || parts.last() == Some(&"main.rs") {
        parts.pop();
    } else if parts.last() == Some(&"mod.rs") {
        parts.pop();
    } else if let Some(last) = parts.pop() {
        parts.push(last.strip_suffix(".rs").unwrap_or(last));
    }
    let mut qn = crate_name.to_owned();
    for part in parts {
        qn.push('.');
        qn.push_str(part);
    }
    qn
}

fn collect_file_def_sites(rel_path: &Path, module_qn: &str, source: &str) -> Vec<OwnedRustDefSite> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut impl_stack: Vec<(String, i32)> = Vec::new();
    let mut depth = 0_i32;

    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx as u32 + 1;
        let trimmed = line.trim();
        while impl_stack
            .last()
            .is_some_and(|(_, impl_depth)| depth < *impl_depth)
        {
            impl_stack.pop();
        }
        if let Some(type_name) = parse_impl_type(trimmed) {
            let next_depth = depth + brace_delta(trimmed).max(1);
            impl_stack.push((type_name, next_depth));
        }
        if let Some(fn_name) = parse_fn_name(trimmed) {
            let end_line = find_block_end_line(&lines, idx).unwrap_or(line_no);
            let qualified_name = if let Some((type_name, _)) = impl_stack.last() {
                format!("{module_qn}.{type_name}.{fn_name}")
            } else {
                format!("{module_qn}.{fn_name}")
            };
            out.push(OwnedRustDefSite {
                qualified_name,
                rel_path: rel_path.to_path_buf(),
                start_line: line_no,
                end_line,
            });
        }
        depth += brace_delta(trimmed);
    }
    out
}

fn parse_impl_type(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("impl ")?;
    let before_brace = rest.split('{').next().unwrap_or(rest).trim();
    let type_part = before_brace
        .rsplit_once(" for ")
        .map_or(before_brace, |(_, type_name)| type_name)
        .split_whitespace()
        .next()?;
    Some(
        type_part
            .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .to_owned(),
    )
}

fn parse_fn_name(trimmed: &str) -> Option<String> {
    let fn_pos = trimmed.find("fn ")?;
    let after = &trimmed[fn_pos + 3..];
    let name = after
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

fn find_block_end_line(lines: &[&str], start_idx: usize) -> Option<u32> {
    let mut depth = 0_i32;
    let mut seen_open = false;
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        let delta = brace_delta(line);
        if line.contains('{') {
            seen_open = true;
        }
        depth += delta;
        if seen_open && depth <= 0 {
            return Some(idx as u32 + 1);
        }
    }
    Some(start_idx as u32 + 1)
}

fn brace_delta(line: &str) -> i32 {
    line.bytes().filter(|byte| *byte == b'{').count() as i32
        - line.bytes().filter(|byte| *byte == b'}').count() as i32
}

fn collect_workspace_call_candidates_inner(
    root: &Path,
    path: &Path,
    out: &mut Vec<CallCandidate>,
) -> std::io::Result<()> {
    if path != root && should_skip_path(root, path) {
        return Ok(());
    }
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            collect_workspace_call_candidates_inner(root, &entry.path(), out)?;
        }
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
        let source = std::fs::read_to_string(path)?;
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        out.extend(collect_file_call_candidates(&relative, &source));
    }
    Ok(())
}

fn should_skip_path(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        matches!(text.as_ref(), ".git" | "target")
    })
}

pub fn collect_file_call_candidates(path: &Path, source: &str) -> Vec<CallCandidate> {
    let parse = SourceFile::parse(source, Edition::CURRENT);
    let syntax = parse.tree().syntax().clone();
    let mut out = Vec::new();
    for node in syntax.descendants() {
        match node.kind() {
            SyntaxKind::CALL_EXPR => {
                if let Some(call) = ast::CallExpr::cast(node.clone()) {
                    let expr = call.expr();
                    let name = expr
                        .as_ref()
                        .map(|expr| expr.syntax().text().to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned());
                    let range = expr
                        .as_ref()
                        .map(|expr| expr.syntax().text_range())
                        .unwrap_or_else(|| node.text_range());
                    out.push(CallCandidate {
                        path: path.to_path_buf(),
                        callee_name: name,
                        start_byte: range.start().into(),
                        end_byte: range.end().into(),
                        kind: CallCandidateKind::Function,
                    });
                }
            }
            SyntaxKind::METHOD_CALL_EXPR => {
                if let Some(call) = ast::MethodCallExpr::cast(node.clone()) {
                    let name_ref = call.name_ref();
                    let name = name_ref
                        .as_ref()
                        .map(|name| name.syntax().text().to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned());
                    let range = name_ref
                        .as_ref()
                        .map(|name| name.syntax().text_range())
                        .unwrap_or_else(|| node.text_range());
                    out.push(CallCandidate {
                        path: path.to_path_buf(),
                        callee_name: name,
                        start_byte: range.start().into(),
                        end_byte: range.end().into(),
                        kind: CallCandidateKind::Method,
                    });
                }
            }
            SyntaxKind::MACRO_CALL => {
                if let Some(call) = ast::MacroCall::cast(node.clone()) {
                    let path_node = call.path();
                    let name = path_node
                        .as_ref()
                        .map(|path| path.syntax().text().to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned());
                    let range = path_node
                        .as_ref()
                        .map(|path| path.syntax().text_range())
                        .unwrap_or_else(|| node.text_range());
                    out.push(CallCandidate {
                        path: path.to_path_buf(),
                        callee_name: short_name(&name).to_owned(),
                        start_byte: range.start().into(),
                        end_byte: range.end().into(),
                        kind: CallCandidateKind::Macro,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_candidate_collection_finds_function_and_method_calls() {
        let source = r#"
struct Thing;
impl Thing { fn work(&self) {} }
fn helper() {}
macro_rules! call_helper { () => { helper() } }
fn run(t: Thing) {
    helper();
    t.work();
    call_helper!();
}
"#;
        let candidates = collect_file_call_candidates(Path::new("src/lib.rs"), source);

        assert!(candidates.iter().any(|candidate| {
            candidate.callee_name == "helper" && candidate.kind == CallCandidateKind::Function
        }));
        assert!(candidates.iter().any(|candidate| {
            candidate.callee_name == "work" && candidate.kind == CallCandidateKind::Method
        }));
        assert!(candidates.iter().any(|candidate| {
            candidate.callee_name == "call_helper" && candidate.kind == CallCandidateKind::Macro
        }));
    }
}
