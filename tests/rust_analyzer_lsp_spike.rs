use std::{
    collections::HashSet,
    fmt::Write as _,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use async_lsp_client::{LspServer, ServerMessage};
use eyre::{Result, WrapErr, bail, eyre};
use serde_json::json;
use tokio::{
    sync::mpsc::Receiver,
    task::JoinHandle,
    time::{sleep, timeout},
};
use tower_lsp::{
    jsonrpc,
    lsp_types::{
        CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyPrepareParams,
        ClientCapabilities, DidOpenTextDocumentParams, DocumentSymbolParams,
        DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
        InitializeParams, Location, Position, PositionEncodingKind, Range, ServerCapabilities,
        TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Url, WorkspaceFolder,
        notification::DidOpenTextDocument,
        request::{
            CallHierarchyIncomingCalls, CallHierarchyPrepare, DocumentSymbolRequest,
            GotoDefinition, HoverRequest, Request, WorkDoneProgressCreate, WorkspaceConfiguration,
        },
    },
};

const FIXTURE_MANIFEST: &str = "tests/fixtures/basic_crate/Cargo.toml";
const FIXTURE_LIB: &str = "tests/fixtures/basic_crate/src/lib.rs";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy)]
struct FixtureCase {
    name: &'static str,
    caller_name: &'static str,
    call_marker: &'static str,
    call_symbol: &'static str,
    definition_file: &'static str,
    definition_marker: &'static str,
    definition_symbol: &'static str,
    known_limitation: Option<&'static str>,
}

const FIXTURE_CASES: &[FixtureCase] = &[
    FixtureCase {
        name: "direct",
        caller_name: "exercise_all_cases",
        call_marker: "direct_target(10)",
        call_symbol: "direct_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn direct_target",
        definition_symbol: "direct_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "method",
        caller_name: "exercise_all_cases",
        call_marker: "calculator.method_target(11)",
        call_symbol: "method_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn method_target",
        definition_symbol: "method_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "trait",
        caller_name: "exercise_all_cases",
        call_marker: "greeter.trait_target()",
        call_symbol: "trait_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "fn trait_target(&self) -> &'static str {",
        definition_symbol: "trait_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "generic",
        caller_name: "exercise_all_cases",
        call_marker: "generic_target::<u32>(12)",
        call_symbol: "generic_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn generic_target",
        definition_symbol: "generic_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "macro",
        caller_name: "exercise_all_cases",
        call_marker: "macro_target(13)",
        call_symbol: "macro_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn macro_target",
        definition_symbol: "macro_target",
        known_limitation: Some(
            "macro-generated definition or call-hierarchy ranges may point at expansion or macro source depending on rust-analyzer behavior",
        ),
    },
    FixtureCase {
        name: "associated",
        caller_name: "exercise_all_cases",
        call_marker: "AssociatedTarget::associated_target(14)",
        call_symbol: "associated_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn associated_target",
        definition_symbol: "associated_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "deref",
        caller_name: "exercise_all_cases",
        call_marker: "wrapper.deref_target(15)",
        call_symbol: "deref_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn deref_target",
        definition_symbol: "deref_target",
        known_limitation: Some(
            "auto-deref method lookup may not return a definition in rust-analyzer LSP spike",
        ),
    },
    FixtureCase {
        name: "cfg_feature",
        caller_name: "exercise_all_cases",
        call_marker: "cfg_target(16)",
        call_symbol: "cfg_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub fn cfg_target",
        definition_symbol: "cfg_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "cross_crate",
        caller_name: "exercise_all_cases",
        call_marker: "cross_crate_target(17)",
        call_symbol: "cross_crate_target",
        definition_file: "tests/fixtures/basic_crate/fixture-cross-crate/src/lib.rs",
        definition_marker: "pub fn cross_crate_target",
        definition_symbol: "cross_crate_target",
        known_limitation: None,
    },
    FixtureCase {
        name: "proc_macro_derive",
        caller_name: "exercise_all_cases",
        call_marker: "generated.proc_macro_target()",
        call_symbol: "proc_macro_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub struct GeneratedByDerive",
        definition_symbol: "GeneratedByDerive",
        known_limitation: Some(
            "proc-macro generated method definition may point at expansion or be unavailable depending on rust-analyzer proc-macro support",
        ),
    },
    FixtureCase {
        name: "async",
        caller_name: "exercise_async_case",
        call_marker: "async_target(18).await",
        call_symbol: "async_target",
        definition_file: FIXTURE_LIB,
        definition_marker: "pub async fn async_target",
        definition_symbol: "async_target",
        known_limitation: None,
    },
];

struct RustAnalyzerSession {
    server: LspServer,
    message_task: JoinHandle<()>,
    opened_files: Mutex<HashSet<PathBuf>>,
    _capabilities: ServerCapabilities,
}

impl RustAnalyzerSession {
    async fn start() -> Result<Self> {
        // The installed rust-analyzer in this environment rejects an explicit `--stdio` flag;
        // async-lsp-client still drives it over stdio when launched without arguments.
        let (server, rx) = LspServer::new("rust-analyzer", std::iter::empty::<&str>());
        let message_task = spawn_server_message_handler(server.clone(), rx);

        let workspace_root = fixture_root()?;
        let root_uri = Url::from_directory_path(&workspace_root).map_err(|()| {
            eyre!(
                "fixture root is not a valid file URL: {}",
                workspace_root.display()
            )
        })?;

        let initialize_result = timeout(
            REQUEST_TIMEOUT,
            server.initialize(InitializeParams {
                process_id: Some(std::process::id()),
                root_uri: Some(root_uri.clone()),
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: root_uri,
                    name: "cbm-ra-fixture-basic".to_owned(),
                }]),
                capabilities: ClientCapabilities {
                    general: Some(tower_lsp::lsp_types::GeneralClientCapabilities {
                        position_encodings: Some(vec![PositionEncodingKind::UTF8]),
                        ..Default::default()
                    }),
                    text_document: Some(tower_lsp::lsp_types::TextDocumentClientCapabilities {
                        document_symbol: Some(
                            tower_lsp::lsp_types::DocumentSymbolClientCapabilities {
                                hierarchical_document_symbol_support: Some(true),
                                ..Default::default()
                            },
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                initialization_options: Some(json!({
                    "linkedProjects": [manifest_path()?.to_string_lossy()],
                    "checkOnSave": false,
                    "cargo": {
                        "features": ["cfg-fixture"],
                        "buildScripts": { "enable": true }
                    },
                    "procMacro": { "enable": true }
                })),
                ..Default::default()
            }),
        )
        .await
        .wrap_err("timed out initializing rust-analyzer")?
        .wrap_err("rust-analyzer initialize failed")?;

        server.initialized().await;

        Ok(Self {
            server,
            message_task,
            opened_files: Mutex::new(HashSet::new()),
            _capabilities: initialize_result.capabilities,
        })
    }

    async fn open_fixture_lib(&self) -> Result<Url> {
        self.open_rust_file(lib_path()?).await
    }

    async fn open_rust_file(&self, path: PathBuf) -> Result<Url> {
        let uri = Url::from_file_path(&path)
            .map_err(|()| eyre!("rust file is not a valid file URL: {}", path.display()))?;
        let should_open = self
            .opened_files
            .lock()
            .map_err(|_| eyre!("opened files lock poisoned"))?
            .insert(path.clone());
        if !should_open {
            return Ok(uri);
        }

        let text = std::fs::read_to_string(&path)
            .wrap_err_with(|| format!("failed to read Rust file at {}", path.display()))?;

        self.server
            .send_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "rust".to_owned(),
                    version: 1,
                    text,
                },
            })
            .await;

        Ok(uri)
    }

    async fn document_symbols(&self, uri: Url) -> Result<DocumentSymbolResponse> {
        timeout(
            REQUEST_TIMEOUT,
            self.server
                .send_request::<DocumentSymbolRequest>(DocumentSymbolParams {
                    text_document: TextDocumentIdentifier { uri },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                }),
        )
        .await
        .wrap_err("timed out requesting document symbols")?
        .wrap_err("document symbol request failed")?
        .ok_or_else(|| eyre!("rust-analyzer returned no document symbols for {FIXTURE_LIB}"))
    }

    async fn goto_definition(
        &self,
        uri: Url,
        position: Position,
    ) -> Result<GotoDefinitionResponse> {
        for _ in 0..50 {
            let response = timeout(
                REQUEST_TIMEOUT,
                self.server
                    .send_request::<GotoDefinition>(GotoDefinitionParams {
                        text_document_position_params: TextDocumentPositionParams {
                            text_document: TextDocumentIdentifier { uri: uri.clone() },
                            position,
                        },
                        work_done_progress_params: Default::default(),
                        partial_result_params: Default::default(),
                    }),
            )
            .await
            .wrap_err("timed out requesting definition")?;

            match response {
                Ok(Some(definition)) => return Ok(definition),
                Ok(None) => {}
                Err(err) if is_content_modified_error(&err.message) => {}
                Err(err) => return Err(err).wrap_err("definition request failed"),
            }

            sleep(Duration::from_millis(100)).await;
        }

        Err(eyre!(
            "rust-analyzer returned no definition at {position:?}"
        ))
    }

    async fn hover(&self, uri: Url, position: Position) -> Result<Option<Hover>> {
        timeout(
            REQUEST_TIMEOUT,
            self.server.send_request::<HoverRequest>(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position,
                },
                work_done_progress_params: Default::default(),
            }),
        )
        .await
        .wrap_err("timed out requesting hover")?
        .wrap_err("hover request failed")
    }

    async fn prepare_call_hierarchy(
        &self,
        uri: Url,
        position: Position,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        timeout(
            REQUEST_TIMEOUT,
            self.server
                .send_request::<CallHierarchyPrepare>(CallHierarchyPrepareParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier { uri },
                        position,
                    },
                    work_done_progress_params: Default::default(),
                }),
        )
        .await
        .wrap_err("timed out preparing call hierarchy")?
        .wrap_err("prepare call hierarchy request failed")
    }

    async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::CallHierarchyIncomingCall>>> {
        timeout(
            REQUEST_TIMEOUT,
            self.server.send_request::<CallHierarchyIncomingCalls>(
                CallHierarchyIncomingCallsParams {
                    item,
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                },
            ),
        )
        .await
        .wrap_err("timed out requesting incoming calls")?
        .wrap_err("incoming call hierarchy request failed")
    }

    async fn shutdown(self) -> Result<()> {
        let _ = timeout(REQUEST_TIMEOUT, self.server.shutdown())
            .await
            .wrap_err("timed out shutting down rust-analyzer")?;
        self.server.exit().await;
        self.message_task.abort();
        Ok(())
    }
}

fn spawn_server_message_handler(
    server: LspServer,
    mut rx: Receiver<ServerMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            match message {
                ServerMessage::Notification(_) => {}
                ServerMessage::Request(req) => {
                    let Some(id) = req.id().cloned() else {
                        continue;
                    };

                    match req.method() {
                        WorkspaceConfiguration::METHOD => {
                            let manifest = std::env::current_dir()
                                .map(|cwd| {
                                    cwd.join(FIXTURE_MANIFEST).to_string_lossy().into_owned()
                                })
                                .unwrap_or_else(|_| FIXTURE_MANIFEST.to_owned());
                            server
                                .send_response::<WorkspaceConfiguration>(
                                    id,
                                    vec![json!({
                                        "linkedProjects": [manifest],
                                        "checkOnSave": false,
                                        "cargo": {
                                            "features": ["cfg-fixture"],
                                            "buildScripts": { "enable": true }
                                        },
                                        "procMacro": { "enable": true }
                                    })],
                                )
                                .await;
                        }
                        WorkDoneProgressCreate::METHOD => {
                            server.send_response::<WorkDoneProgressCreate>(id, ()).await;
                        }
                        _ => {
                            server
                                .send_error_response(
                                    id,
                                    jsonrpc::Error {
                                        code: jsonrpc::ErrorCode::MethodNotFound,
                                        message: std::borrow::Cow::Borrowed("method not found"),
                                        data: req.params().cloned(),
                                    },
                                )
                                .await;
                        }
                    }
                }
            }
        }
    })
}

fn fixture_root() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join("tests/fixtures/basic_crate"))
}

fn manifest_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(FIXTURE_MANIFEST))
}

fn lib_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()?.join(FIXTURE_LIB))
}

fn generated_include_path() -> Result<PathBuf> {
    let target = fixture_root()?.join("target");
    find_named_file(&target, "generated_include.rs")?.ok_or_else(|| {
        eyre!(
            "generated include file not found under {}",
            target.display()
        )
    })
}

fn find_named_file(root: &std::path::Path, name: &str) -> Result<Option<PathBuf>> {
    if !root.exists() {
        return Ok(None);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = std::fs::metadata(&path)?;
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                stack.push(entry?.path());
            }
        } else if path.file_name().and_then(|file| file.to_str()) == Some(name) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn symbol_position(source: &str, marker: &str, symbol: &str) -> Result<Position> {
    let marker_offset = source
        .find(marker)
        .ok_or_else(|| eyre!("marker not found in fixture source: {marker}"))?;
    let symbol_offset = source[marker_offset..]
        .find(symbol)
        .ok_or_else(|| eyre!("symbol {symbol} not found inside marker {marker}"))?;
    position_at_byte_offset(source, marker_offset + symbol_offset + symbol.len() / 2)
}

fn position_at_byte_offset(source: &str, byte_offset: usize) -> Result<Position> {
    let prefix = source
        .get(..byte_offset)
        .ok_or_else(|| eyre!("byte offset {byte_offset} is not on a UTF-8 boundary"))?;
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, tail)| tail.len()) as u32;
    Ok(Position { line, character })
}

fn range_contains_position(range: Range, position: Position) -> bool {
    (range.start.line, range.start.character) <= (position.line, position.character)
        && (position.line, position.character) <= (range.end.line, range.end.character)
}

fn is_content_modified_error(message: &str) -> bool {
    message.to_ascii_lowercase().contains("content modified")
}

fn definition_locations(response: GotoDefinitionResponse) -> Vec<Location> {
    match response {
        GotoDefinitionResponse::Scalar(location) => vec![location],
        GotoDefinitionResponse::Array(locations) => locations,
        GotoDefinitionResponse::Link(links) => links
            .into_iter()
            .map(|link| Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect(),
    }
}

fn hover_contains_signature(hover: Option<Hover>, expected_name: &str) -> bool {
    format!("{hover:?}").contains(expected_name)
}

#[derive(Debug, Default)]
struct SpikeReport {
    limitations: Vec<String>,
}

impl SpikeReport {
    fn record_limitation(&mut self, case: FixtureCase, detail: impl Into<String>) {
        let detail = detail.into();
        if let Some(limitation) = case.known_limitation {
            let message = format!("{}: {limitation}; {detail}", case.name);
            if !self.limitations.contains(&message) {
                eprintln!("documented limitation: {message}");
                self.limitations.push(message);
            }
            return;
        }
        panic!("{} failed: {detail}", case.name);
    }

    fn assert_or_record(
        &mut self,
        case: FixtureCase,
        condition: bool,
        detail: impl FnOnce() -> String,
    ) {
        if !condition {
            self.record_limitation(case, detail());
        }
    }

    fn print_summary(&self, cold: Duration, warm: Duration) {
        eprintln!("rust-analyzer cold start + first query: {cold:?}");
        eprintln!("rust-analyzer warm semantic query pass: {warm:?}");
        if self.limitations.is_empty() {
            eprintln!("rust-analyzer LSP spike limitations: none");
        } else {
            eprintln!("rust-analyzer LSP spike limitations:");
            for limitation in &self.limitations {
                eprintln!("- {limitation}");
            }
        }
    }
}

#[derive(Debug)]
struct RelationshipEvidence {
    case_name: &'static str,
    caller_qn: String,
    callee_qn: String,
    call_range: String,
    target_ranges: String,
    strategy: &'static str,
    confidence: f32,
    reason: String,
    graph_decision: &'static str,
    raw_definition: String,
    hover_usable: bool,
    call_hierarchy_matched: bool,
    raw_call_hierarchy: String,
}

fn fixture_qn(symbol: &str) -> String {
    format!("cbm_ra_fixture_basic::{symbol}")
}

fn target_qn(case: FixtureCase, resolved_to_expected_definition: bool) -> String {
    if case.name == "cross_crate" {
        return format!("fixture_cross_crate::{}", case.definition_symbol);
    }
    if case.name == "proc_macro_derive" {
        return "cbm_ra_fixture_basic::GeneratedByDerive::proc_macro_target".to_owned();
    }
    if resolved_to_expected_definition || case.definition_symbol != "GeneratedByDerive" {
        return fixture_qn(case.definition_symbol);
    }
    fixture_qn(case.call_symbol)
}

fn format_position(position: Position) -> String {
    format!("{}:{}", position.line, position.character)
}

fn format_range(range: Range) -> String {
    format!(
        "{}:{}-{}:{}",
        range.start.line, range.start.character, range.end.line, range.end.character
    )
}

fn format_locations(locations: &[Location]) -> String {
    if locations.is_empty() {
        return "[]".to_owned();
    }
    locations
        .iter()
        .map(|location| format!("{}@{}", location.uri, format_range(location.range)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_incoming_calls(calls: &[tower_lsp::lsp_types::CallHierarchyIncomingCall]) -> String {
    if calls.is_empty() {
        return "[]".to_owned();
    }
    calls
        .iter()
        .map(|call| {
            let ranges = call
                .from_ranges
                .iter()
                .map(|range| format_range(*range))
                .collect::<Vec<_>>()
                .join(",");
            format!("{} {} [{}]", call.from.name, call.from.uri, ranges)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn markdown_cell(text: impl AsRef<str>) -> String {
    text.as_ref()
        .replace('|', "\\|")
        .replace('\n', "<br>")
        .replace('\r', "")
}

fn classify_relationship(
    case: FixtureCase,
    resolved_to_expected_definition: bool,
    hover_usable: bool,
    incoming_from_fixture_call: bool,
    locations: &[Location],
) -> (&'static str, f32, &'static str, String) {
    match case.name {
        "macro" if resolved_to_expected_definition => (
            "ra_macro_expansion",
            0.70,
            "emit_call_edge",
            "macro_rules target has stable source range; call hierarchy provenance recorded separately".to_owned(),
        ),
        "macro" => (
            "ra_macro_expansion",
            0.50,
            "no_emit_generated",
            "macro-generated target lacks stable source evidence".to_owned(),
        ),
        "proc_macro_derive" => (
            "ra_proc_macro",
            0.50,
            "no_emit_generated",
            "proc-macro generated method points at derive macro/proc-macro provenance, not stable method source range".to_owned(),
        ),
        "deref" if !resolved_to_expected_definition || !hover_usable || !incoming_from_fixture_call => (
            "ra_deref_dispatch",
            0.0,
            "no_emit_unresolved",
            "auto-deref dispatch lacks usable RA definition/hover/call-hierarchy evidence in LSP spike".to_owned(),
        ),
        _ if resolved_to_expected_definition && incoming_from_fixture_call => (
            if case.name == "cross_crate" {
                "ra_definition_cross_crate+ra_call_hierarchy"
            } else {
                "ra_definition+ra_call_hierarchy"
            },
            0.95,
            "emit_call_edge",
            "stable local/workspace target range and caller call-site evidence".to_owned(),
        ),
        _ if locations.len() > 1 => (
            "ra_ambiguous",
            0.0,
            "no_emit_ambiguous",
            "multiple definition locations without stable graph target".to_owned(),
        ),
        _ => (
            "ra_unresolved",
            0.0,
            "no_emit_unresolved",
            "missing stable target or caller evidence".to_owned(),
        ),
    }
}

fn write_relationship_evidence_report(
    evidence: &[RelationshipEvidence],
    cold: Duration,
    warm: Duration,
) -> Result<()> {
    let mut out = String::new();
    writeln!(out, "# Rust LSP relationship evidence")?;
    writeln!(out)?;
    writeln!(
        out,
        "Fixture relationship proof for CBM `CBMResolvedCall` parity."
    )?;
    writeln!(out)?;
    writeln!(out, "## Compile config")?;
    writeln!(out)?;
    writeln!(out, "- manifest: `{FIXTURE_MANIFEST}`")?;
    writeln!(out, "- linkedProjects: [`{FIXTURE_MANIFEST}`]")?;
    writeln!(out, "- features: [`cfg-fixture`]")?;
    writeln!(out, "- target triple: default host")?;
    writeln!(out, "- build scripts: enabled")?;
    writeln!(
        out,
        "- generated include: `build.rs` writes `OUT_DIR/generated_include.rs`; fixture uses `include!(concat!(env!(\"OUT_DIR\"), \"/generated_include.rs\"))`"
    )?;
    writeln!(out, "- proc macros: enabled")?;
    writeln!(out, "- request timeout: `{}s`", REQUEST_TIMEOUT.as_secs())?;
    writeln!(out)?;
    writeln!(out, "## Timings")?;
    writeln!(out)?;
    writeln!(out, "- cold start + first relationship pass: `{cold:?}`")?;
    writeln!(out, "- warm relationship pass: `{warm:?}`")?;
    writeln!(out)?;
    writeln!(out, "## Resolved-call evidence")?;
    writeln!(out)?;
    writeln!(
        out,
        "| case | caller_qn | callee_qn | strategy | confidence | graph_decision | reason | call_range | target_ranges | hover | call_hierarchy |"
    )?;
    writeln!(out, "|---|---|---|---|---:|---|---|---|---|---|---|")?;
    for item in evidence {
        writeln!(
            out,
            "| `{}` | `{}` | `{}` | `{}` | {:.2} | `{}` | {} | `{}` | {} | {} | {} |",
            markdown_cell(item.case_name),
            markdown_cell(&item.caller_qn),
            markdown_cell(&item.callee_qn),
            markdown_cell(item.strategy),
            item.confidence,
            markdown_cell(item.graph_decision),
            markdown_cell(&item.reason),
            markdown_cell(&item.call_range),
            markdown_cell(&item.target_ranges),
            item.hover_usable,
            item.call_hierarchy_matched,
        )?;
    }
    writeln!(out)?;
    writeln!(out, "## Hard-case graph decisions")?;
    writeln!(out)?;
    writeln!(out, "| case | class | emit? | reason |")?;
    writeln!(out, "|---|---|---|---|")?;
    for hard_case in [
        "macro",
        "proc_macro_derive",
        "deref",
        "include_out_dir_source_to_generated",
        "include_out_dir_generated_to_workspace",
    ] {
        if let Some(item) = evidence.iter().find(|item| item.case_name == hard_case) {
            let class = match item.graph_decision {
                "emit_call_edge" => "exact",
                "no_emit_generated" => "generated",
                "no_emit_ambiguous" => "ambiguous",
                "no_emit_unresolved" => "unresolved",
                other => other,
            };
            let emit = item.graph_decision == "emit_call_edge";
            writeln!(
                out,
                "| `{}` | `{}` | {} | {} |",
                markdown_cell(item.case_name),
                markdown_cell(class),
                emit,
                markdown_cell(&item.reason)
            )?;
        }
    }
    writeln!(out)?;
    writeln!(out, "## Raw RA evidence")?;
    writeln!(out)?;
    for item in evidence {
        writeln!(out, "### {}", item.case_name)?;
        writeln!(out)?;
        writeln!(
            out,
            "- definitions: {}",
            markdown_cell(&item.raw_definition)
        )?;
        writeln!(
            out,
            "- incoming calls: {}",
            markdown_cell(&item.raw_call_hierarchy)
        )?;
        writeln!(out)?;
    }
    std::fs::write("RELATIONSHIP_EVIDENCE.md", out)?;
    Ok(())
}

async fn exercise_semantic_queries(
    session: &RustAnalyzerSession,
    uri: Url,
    source: &str,
    report: &mut SpikeReport,
) -> Result<Vec<RelationshipEvidence>> {
    let mut evidence = Vec::new();

    for case in FIXTURE_CASES {
        let call_position = symbol_position(source, case.call_marker, case.call_symbol)?;
        let definition_path = std::env::current_dir()?.join(case.definition_file);
        let definition_uri = Url::from_file_path(&definition_path).map_err(|()| {
            eyre!(
                "definition file is not a valid file URL: {}",
                definition_path.display()
            )
        })?;
        let definition_source = std::fs::read_to_string(&definition_path).wrap_err_with(|| {
            format!(
                "failed to read definition source at {}",
                definition_path.display()
            )
        })?;
        let definition_position = symbol_position(
            &definition_source,
            case.definition_marker,
            case.definition_symbol,
        )?;

        let locations =
            definition_locations(session.goto_definition(uri.clone(), call_position).await?);
        let resolved_to_expected_definition = locations.iter().any(|location| {
            location.uri == definition_uri
                && range_contains_position(location.range, definition_position)
        });
        report.assert_or_record(*case, resolved_to_expected_definition, || {
            format!(
                "definition did not map back to symbol {} inside marker {}; locations: {locations:#?}",
                case.definition_symbol, case.definition_marker
            )
        });

        let hover = session.hover(uri.clone(), call_position).await?;
        let hover_usable = hover_contains_signature(hover, case.call_symbol);
        report.assert_or_record(*case, hover_usable, || {
            "hover did not include expected symbol text".to_owned()
        });

        let prepared = session
            .prepare_call_hierarchy(definition_uri.clone(), definition_position)
            .await?
            .unwrap_or_default();
        let incoming = if let Some(item) = prepared.first().cloned() {
            session.incoming_calls(item).await?.unwrap_or_default()
        } else {
            Vec::new()
        };
        let incoming_from_fixture_call = incoming.iter().any(|call| {
            call.from.name == case.caller_name
                && call.from.uri == uri
                && call
                    .from_ranges
                    .iter()
                    .any(|range| range_contains_position(*range, call_position))
        });
        report.assert_or_record(*case, incoming_from_fixture_call, || {
            format!(
                "call hierarchy did not map {} back to call-site symbol {}: {incoming:#?}",
                case.caller_name, case.call_symbol
            )
        });

        let (strategy, confidence, graph_decision, reason) = classify_relationship(
            *case,
            resolved_to_expected_definition,
            hover_usable,
            incoming_from_fixture_call,
            &locations,
        );
        evidence.push(RelationshipEvidence {
            case_name: case.name,
            caller_qn: fixture_qn(case.caller_name),
            callee_qn: target_qn(*case, resolved_to_expected_definition),
            call_range: format_position(call_position),
            target_ranges: format_locations(&locations),
            strategy,
            confidence,
            reason,
            graph_decision,
            raw_definition: format_locations(&locations),
            hover_usable,
            call_hierarchy_matched: incoming_from_fixture_call,
            raw_call_hierarchy: format_incoming_calls(&incoming),
        });
    }

    evidence.extend(exercise_out_dir_include_queries(session, uri, source).await?);

    Ok(evidence)
}

async fn exercise_out_dir_include_queries(
    session: &RustAnalyzerSession,
    fixture_uri: Url,
    fixture_source: &str,
) -> Result<Vec<RelationshipEvidence>> {
    let mut evidence = Vec::new();
    let generated_path = generated_include_path()?;
    let generated_uri = session.open_rust_file(generated_path.clone()).await?;
    let generated_source = std::fs::read_to_string(&generated_path)?;

    let call_position = symbol_position(
        fixture_source,
        "out_dir_generated::generated_out_dir_target(30)",
        "generated_out_dir_target",
    )?;
    let generated_definition_position = symbol_position(
        &generated_source,
        "pub fn generated_out_dir_target",
        "generated_out_dir_target",
    )?;
    let locations = definition_locations(
        session
            .goto_definition(fixture_uri.clone(), call_position)
            .await?,
    );
    let resolved_to_generated = locations.iter().any(|location| {
        location.uri == generated_uri
            && range_contains_position(location.range, generated_definition_position)
    });
    if !resolved_to_generated {
        bail!(
            "OUT_DIR source-to-generated definition did not map to generated file; locations: {locations:#?}"
        );
    }
    let hover = session.hover(fixture_uri.clone(), call_position).await?;
    evidence.push(RelationshipEvidence {
        case_name: "include_out_dir_source_to_generated",
        caller_qn: fixture_qn("exercise_out_dir_include_case"),
        callee_qn: fixture_qn("out_dir_generated::generated_out_dir_target"),
        call_range: format_position(call_position),
        target_ranges: format_locations(&locations),
        strategy: "ra_include_out_dir",
        confidence: 0.50,
        reason: "source call resolves to build.rs OUT_DIR include file; generated target provenance recorded and edge suppressed".to_owned(),
        graph_decision: "no_emit_generated_file",
        raw_definition: format_locations(&locations),
        hover_usable: hover_contains_signature(hover, "generated_out_dir_target"),
        call_hierarchy_matched: false,
        raw_call_hierarchy: "generated target call hierarchy not trusted for graph edge".to_owned(),
    });

    let generated_call_position = symbol_position(
        &generated_source,
        "crate::out_dir_workspace_target(input)",
        "out_dir_workspace_target",
    )?;
    let workspace_definition_position = symbol_position(
        fixture_source,
        "pub fn out_dir_workspace_target",
        "out_dir_workspace_target",
    )?;
    let workspace_locations = definition_locations(
        session
            .goto_definition(generated_uri.clone(), generated_call_position)
            .await?,
    );
    let resolved_to_workspace = workspace_locations.iter().any(|location| {
        location.uri == fixture_uri
            && range_contains_position(location.range, workspace_definition_position)
    });
    if !resolved_to_workspace {
        bail!(
            "OUT_DIR generated-to-workspace definition did not map to fixture source; locations: {workspace_locations:#?}"
        );
    }
    let generated_hover = session
        .hover(generated_uri, generated_call_position)
        .await?;
    evidence.push(RelationshipEvidence {
        case_name: "include_out_dir_generated_to_workspace",
        caller_qn: fixture_qn("out_dir_generated::generated_calls_workspace"),
        callee_qn: fixture_qn("out_dir_workspace_target"),
        call_range: format_position(generated_call_position),
        target_ranges: format_locations(&workspace_locations),
        strategy: "ra_include_out_dir",
        confidence: 0.50,
        reason: "generated OUT_DIR include caller resolves to workspace target; generated caller provenance recorded and edge suppressed".to_owned(),
        graph_decision: "no_emit_generated_file",
        raw_definition: format_locations(&workspace_locations),
        hover_usable: hover_contains_signature(generated_hover, "out_dir_workspace_target"),
        call_hierarchy_matched: false,
        raw_call_hierarchy: "generated caller call hierarchy not trusted for graph edge".to_owned(),
    });

    Ok(evidence)
}

#[tokio::test]
async fn rust_analyzer_lsp_spike_can_start_and_read_fixture_symbols() -> Result<()> {
    if !manifest_path()?.exists() {
        bail!("fixture manifest missing at {FIXTURE_MANIFEST}");
    }

    let session = RustAnalyzerSession::start().await?;
    let uri = session.open_fixture_lib().await?;
    let symbols = session.document_symbols(uri).await?;

    let symbol_count = match symbols {
        DocumentSymbolResponse::Nested(symbols) => symbols.len(),
        DocumentSymbolResponse::Flat(symbols) => symbols.len(),
    };

    assert!(
        symbol_count > 0,
        "rust-analyzer returned no document symbols for {FIXTURE_LIB}"
    );

    session.shutdown().await
}

#[tokio::test]
async fn rust_analyzer_lsp_spike_resolves_definitions_and_call_hierarchy() -> Result<()> {
    let cold_start = Instant::now();
    let session = RustAnalyzerSession::start().await?;
    let uri = session.open_fixture_lib().await?;
    let source = std::fs::read_to_string(lib_path()?)?;
    sleep(Duration::from_secs(2)).await;

    let mut report = SpikeReport::default();
    let evidence = exercise_semantic_queries(&session, uri.clone(), &source, &mut report).await?;
    let cold = cold_start.elapsed();

    let warm_start = Instant::now();
    let _warm_evidence = exercise_semantic_queries(&session, uri, &source, &mut report).await?;
    let warm = warm_start.elapsed();

    write_relationship_evidence_report(&evidence, cold, warm)?;
    report.print_summary(cold, warm);
    session.shutdown().await
}
