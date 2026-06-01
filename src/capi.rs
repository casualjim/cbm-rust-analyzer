use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_float, c_int},
    panic::{UnwindSafe, catch_unwind},
    path::Path,
    ptr,
};

use crate::{
    CbmCallInput, ResolvedCall, ResolverConfig, RustDefSite, SemanticResolveError,
    cbm_sys::{
        CBMArena, CBMCall, CBMImport, CBMLSPDef, CBMResolvedCall, CBMResolvedCallArray,
        CBMRustLSPDefSite,
    },
    load_workspace, resolve_cbm_calls_by_def_sites,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CbmRustStatus {
    Ok = 0,
    InvalidArgument = 1,
    WorkspaceLoad = 2,
    Analysis = 3,
    Timeout = 4,
    Cancelled = 5,
    Unsupported = 6,
    Internal = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CbmBatchRustLspFile {
    pub source: *const c_char,
    pub source_len: c_int,
    pub rel_path: *const c_char,
    pub module_qn: *const c_char,
    pub calls: *const CBMCall,
    pub call_count: c_int,
    pub imports: *const CBMImport,
    pub import_count: c_int,
}

#[unsafe(no_mangle)]
pub extern "C" fn cbm_run_rust_lsp_cross(
    arena: *mut CBMArena,
    workspace_root: *const c_char,
    source: *const c_char,
    source_len: c_int,
    module_qn: *const c_char,
    defs: *mut CBMLSPDef,
    def_count: c_int,
    imports: *const CBMImport,
    import_count: c_int,
    calls: *const CBMCall,
    call_count: c_int,
    out: *mut CBMResolvedCallArray,
) {
    let _ = catch_ffi_status(|| unsafe {
        if arena.is_null() {
            return Err(CbmRustStatus::InvalidArgument);
        }
        c_str(workspace_root).ok_or(CbmRustStatus::InvalidArgument)?;
        c_str(module_qn).ok_or(CbmRustStatus::InvalidArgument)?;
        c_source(source, source_len)?;
        c_slice(defs, def_count)?;
        validate_imports(imports, import_count)?;
        c_slice(calls, call_count)?;
        write_resolved_calls(out, Vec::new())?;
        Ok(CbmRustStatus::Ok)
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn cbm_rust_analyzer_resolve_cross(
    workspace_root: *const c_char,
    source: *const c_char,
    source_len: c_int,
    rel_path: *const c_char,
    module_qn: *const c_char,
    defs: *const CBMLSPDef,
    def_count: c_int,
    imports: *const CBMImport,
    import_count: c_int,
    calls: *const CBMCall,
    call_count: c_int,
    out: *mut CBMResolvedCallArray,
) -> CbmRustStatus {
    catch_ffi_status(|| unsafe {
        c_str(workspace_root).ok_or(CbmRustStatus::InvalidArgument)?;
        c_str(rel_path).ok_or(CbmRustStatus::InvalidArgument)?;
        c_str(module_qn).ok_or(CbmRustStatus::InvalidArgument)?;
        c_source(source, source_len)?;
        validate_imports(imports, import_count)?;
        c_slice(defs, def_count)?;
        c_slice(calls, call_count)?;
        write_resolved_calls(out, Vec::new())?;
        Ok(CbmRustStatus::Ok)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn cbm_rust_analyzer_resolve_batch(
    workspace_root: *const c_char,
    defs: *const CBMLSPDef,
    def_count: c_int,
    files: *const CbmBatchRustLspFile,
    file_count: c_int,
    out: *mut CBMResolvedCallArray,
) -> CbmRustStatus {
    catch_ffi_status(|| unsafe {
        c_str(workspace_root).ok_or(CbmRustStatus::InvalidArgument)?;
        c_slice(defs, def_count)?;
        let files = c_slice(files, file_count)?;
        if out.is_null() {
            return Err(CbmRustStatus::InvalidArgument);
        }

        for file in files {
            c_str(file.rel_path).ok_or(CbmRustStatus::InvalidArgument)?;
            c_str(file.module_qn).ok_or(CbmRustStatus::InvalidArgument)?;
            c_source(file.source, file.source_len)?;
            validate_imports(file.imports, file.import_count)?;
            c_slice(file.calls, file.call_count)?;
        }
        write_resolved_calls(out, Vec::new())?;
        Ok(CbmRustStatus::Ok)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn cbm_rust_analyzer_resolve_batch_v2(
    workspace_root: *const c_char,
    def_sites: *const CBMRustLSPDefSite,
    def_site_count: c_int,
    files: *const CbmBatchRustLspFile,
    file_count: c_int,
    out: *mut CBMResolvedCallArray,
) -> CbmRustStatus {
    catch_ffi_status(|| unsafe {
        let workspace_root = c_str(workspace_root).ok_or(CbmRustStatus::InvalidArgument)?;
        let def_sites = c_slice(def_sites, def_site_count)?;
        let files = c_slice(files, file_count)?;
        if out.is_null() {
            return Err(CbmRustStatus::InvalidArgument);
        }
        let workspace = load_workspace(ResolverConfig::trusted_full(workspace_root))
            .map_err(|_| CbmRustStatus::WorkspaceLoad)?;
        let def_sites = rust_def_sites_from_c(def_sites)?;
        let mut calls = Vec::new();
        for file in files {
            c_str(file.rel_path).ok_or(CbmRustStatus::InvalidArgument)?;
            c_str(file.module_qn).ok_or(CbmRustStatus::InvalidArgument)?;
            c_source(file.source, file.source_len)?;
            validate_imports(file.imports, file.import_count)?;
            calls.extend(cbm_call_inputs_from_c(c_slice(
                file.calls,
                file.call_count,
            )?));
        }
        let resolved = resolve_cbm_calls_by_def_sites(&workspace, &calls, &def_sites)
            .map_err(|err| semantic_error_to_status(&err))?;
        write_resolved_calls(out, resolved)?;
        Ok(CbmRustStatus::Ok)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn cbm_rust_analyzer_free_resolved_call_array(out: *mut CBMResolvedCallArray) {
    if out.is_null() {
        return;
    }

    unsafe {
        let arr = &mut *out;
        if arr.items.is_null() || arr.count <= 0 || arr.cap <= 0 {
            *arr = empty_array();
            return;
        }

        let items = Vec::from_raw_parts(arr.items, arr.count as usize, arr.cap as usize);
        for item in items {
            free_c_string(item.caller_qn as *mut c_char);
            free_c_string(item.callee_qn as *mut c_char);
            free_c_string(item.strategy as *mut c_char);
            free_c_string(item.reason as *mut c_char);
        }
        *arr = empty_array();
    }
}

unsafe fn rust_def_sites_from_c<'a>(
    def_sites: &'a [CBMRustLSPDefSite],
) -> Result<Vec<RustDefSite<'a>>, CbmRustStatus> {
    def_sites
        .iter()
        .map(|def| unsafe {
            let qualified_name = c_str(def.qualified_name).ok_or(CbmRustStatus::InvalidArgument)?;
            let rel_path = c_str(def.rel_path).ok_or(CbmRustStatus::InvalidArgument)?;
            if def.start_line == 0 || def.end_line < def.start_line {
                return Err(CbmRustStatus::InvalidArgument);
            }
            Ok(RustDefSite {
                qualified_name,
                rel_path: Path::new(rel_path),
                start_line: def.start_line,
                end_line: def.end_line,
            })
        })
        .collect()
}

unsafe fn cbm_call_inputs_from_c(calls: &[CBMCall]) -> Vec<CbmCallInput<'_>> {
    calls
        .iter()
        .filter_map(|call| unsafe {
            Some(CbmCallInput {
                callee_name: c_str(call.callee_name)?,
                enclosing_func_qn: c_str(call.enclosing_func_qn)?,
            })
        })
        .collect()
}

fn semantic_error_to_status(error: &SemanticResolveError) -> CbmRustStatus {
    match error {
        SemanticResolveError::FileNotInWorkspace(_) => CbmRustStatus::Analysis,
        SemanticResolveError::Analysis(_) => CbmRustStatus::Analysis,
    }
}

fn catch_ffi_status(
    f: impl FnOnce() -> Result<CbmRustStatus, CbmRustStatus> + UnwindSafe,
) -> CbmRustStatus {
    match catch_unwind(f) {
        Ok(Ok(status)) => status,
        Ok(Err(status)) => status,
        Err(_) => CbmRustStatus::Internal,
    }
}

unsafe fn c_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

unsafe fn c_source<'a>(ptr: *const c_char, len: c_int) -> Result<&'a str, CbmRustStatus> {
    if len < 0 || (len > 0 && ptr.is_null()) {
        return Err(CbmRustStatus::InvalidArgument);
    }
    if len == 0 {
        return Ok("");
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len as usize) };
    std::str::from_utf8(bytes).map_err(|_| CbmRustStatus::InvalidArgument)
}

unsafe fn c_slice<'a, T>(ptr: *const T, count: c_int) -> Result<&'a [T], CbmRustStatus> {
    if count < 0 {
        return Err(CbmRustStatus::InvalidArgument);
    }
    if count == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(CbmRustStatus::InvalidArgument);
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, count as usize) })
}

unsafe fn validate_imports(
    imports: *const CBMImport,
    import_count: c_int,
) -> Result<(), CbmRustStatus> {
    unsafe { c_slice(imports, import_count)? };
    Ok(())
}

unsafe fn write_resolved_calls(
    out: *mut CBMResolvedCallArray,
    resolved: Vec<ResolvedCall>,
) -> Result<(), CbmRustStatus> {
    if out.is_null() {
        return Err(CbmRustStatus::InvalidArgument);
    }
    if resolved.is_empty() {
        unsafe { *out = empty_array() };
        return Ok(());
    }

    let mut items = Vec::with_capacity(resolved.len());
    for call in resolved {
        items.push(CBMResolvedCall {
            caller_qn: into_c_string(&call.caller_qn)?,
            callee_qn: into_c_string(&call.callee_qn)?,
            strategy: into_c_string(&call.strategy)?,
            confidence: (call.confidence_bps as c_float) / 10_000.0,
            reason: into_optional_c_string(call.reason.as_deref())?,
        });
    }

    let count = items.len() as c_int;
    let cap = items.capacity() as c_int;
    let items_ptr = items.as_mut_ptr();
    std::mem::forget(items);
    unsafe {
        *out = CBMResolvedCallArray {
            items: items_ptr,
            count,
            cap,
        };
    }
    Ok(())
}

fn into_optional_c_string(value: Option<&str>) -> Result<*const c_char, CbmRustStatus> {
    value.map_or(Ok(ptr::null()), into_c_string)
}

fn into_c_string(value: &str) -> Result<*const c_char, CbmRustStatus> {
    CString::new(value)
        .map(|value| value.into_raw() as *const c_char)
        .map_err(|_| CbmRustStatus::InvalidArgument)
}

unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

const fn empty_array() -> CBMResolvedCallArray {
    CBMResolvedCallArray {
        items: ptr::null_mut(),
        count: 0,
        cap: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cbm_sys::{CBMCallArg, CBMLanguage_CBM_LANG_RUST};
    use std::{fs, os::raw::c_uint, path::PathBuf};

    fn c(value: &str) -> CString {
        CString::new(value).unwrap()
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate")
    }

    fn call(callee: &CString, caller: &CString) -> CBMCall {
        CBMCall {
            callee_name: callee.as_ptr(),
            enclosing_func_qn: caller.as_ptr(),
            first_string_arg: ptr::null(),
            second_arg_name: ptr::null(),
            args: [CBMCallArg {
                expr: ptr::null(),
                value: ptr::null(),
                keyword: ptr::null(),
                index: 0,
            }; 8],
            arg_count: 0,
        }
    }

    fn def(qn: &CString, short: &CString, module: &CString) -> CBMLSPDef {
        let label = c("Function");
        let def = CBMLSPDef {
            qualified_name: qn.as_ptr(),
            short_name: short.as_ptr(),
            label: label.as_ptr(),
            receiver_type: ptr::null(),
            def_module_qn: module.as_ptr(),
            return_types: ptr::null(),
            embedded_types: ptr::null(),
            field_defs: ptr::null(),
            method_names_str: ptr::null(),
            is_interface: false,
            lang: CBMLanguage_CBM_LANG_RUST,
        };
        std::mem::forget(label);
        def
    }

    fn def_site(qn: &CString, rel_path: &CString, line: c_uint) -> CBMRustLSPDefSite {
        def_site_range(qn, rel_path, line, line)
    }

    fn def_site_range(
        qn: &CString,
        rel_path: &CString,
        start_line: c_uint,
        end_line: c_uint,
    ) -> CBMRustLSPDefSite {
        CBMRustLSPDefSite {
            qualified_name: qn.as_ptr(),
            rel_path: rel_path.as_ptr(),
            start_line,
            end_line,
        }
    }

    fn line_of(source: &str, needle: &str) -> c_uint {
        source
            .lines()
            .position(|line| line.contains(needle))
            .map(|idx| idx as c_uint + 1)
            .unwrap_or_else(|| panic!("missing line containing {needle:?}"))
    }

    fn line_of_after(source: &str, after: &str, needle: &str) -> c_uint {
        let start = source
            .find(after)
            .unwrap_or_else(|| panic!("missing marker {after:?}"));
        source[start..]
            .lines()
            .position(|line| line.contains(needle))
            .map(|idx| source[..start].lines().count() as c_uint + idx as c_uint + 1)
            .unwrap_or_else(|| panic!("missing line containing {needle:?} after {after:?}"))
    }

    fn fixture_source() -> (PathBuf, CString, CString, CString) {
        let root = fixture_root();
        let root_c = c(root.to_str().unwrap());
        let rel_path = c("src/lib.rs");
        let source_text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let source = c(&source_text);
        (root, root_c, rel_path, source)
    }

    #[test]
    fn exact_cbm_wrapper_symbol_safe_empty_without_file_identity() {
        let mut arena = CBMArena {
            blocks: [ptr::null_mut(); 256],
            block_sizes: [0; 256],
            nblocks: 0,
            block_size: 0,
            used: 0,
            total_alloc: 0,
        };
        let root = fixture_root();
        let root_c = c(root.to_str().unwrap());
        let source_text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let source = c(&source_text);
        let module_qn = c("cbm_ra_fixture_basic");
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let callee = c("direct_target");
        let call = call(&callee, &caller);
        let mut out = empty_array();

        cbm_run_rust_lsp_cross(
            &mut arena,
            root_c.as_ptr(),
            source.as_ptr(),
            source.as_bytes().len() as c_int,
            module_qn.as_ptr(),
            ptr::null_mut(),
            0,
            ptr::null(),
            0,
            &call,
            1,
            &mut out,
        );

        assert_eq!(out.count, 0);
        assert!(out.items.is_null());
    }

    #[test]
    fn capi_v2_resolves_edges_by_ra_target_ranges() {
        let (_root, root_c, rel_path, source) = fixture_source();
        let module_qn = c("cbm_ra_fixture_basic");
        let source_text = source.to_str().unwrap();
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let direct_callee = c("direct_target");
        let trait_callee = c("trait_target");
        let generic_callee = c("generic_target");
        let macro_item_callee = c("macro_target");
        let macro_expansion_callee = c("call_cross_file_method");
        let macro_generated_method_callee = c("macro_generated_cross_file_target");
        let proc_callee = c("proc_macro_target");
        let calls = [
            call(&direct_callee, &caller),
            call(&trait_callee, &caller),
            call(&generic_callee, &caller),
            call(&macro_item_callee, &caller),
            call(&macro_expansion_callee, &caller),
            call(&macro_generated_method_callee, &caller),
            call(&proc_callee, &caller),
        ];
        let file = CbmBatchRustLspFile {
            source: source.as_ptr(),
            source_len: source.as_bytes().len() as c_int,
            rel_path: rel_path.as_ptr(),
            module_qn: module_qn.as_ptr(),
            calls: calls.as_ptr(),
            call_count: calls.len() as c_int,
            imports: ptr::null(),
            import_count: 0,
        };
        let caller_qn = c("cbm_ra_fixture_basic.exercise_all_cases");
        let direct_qn = c("cbm_ra_fixture_basic.direct_target");
        let macro_method_rel_path = c("src/macro_target_mod.rs");
        let macro_method_source =
            fs::read_to_string(_root.join("src/macro_target_mod.rs")).unwrap();
        let macro_method_qn =
            c("cbm_ra_fixture_basic.macro_target_mod.MacroReceiver.macro_cross_file_target");
        let macro_generated_method_rel_path = c("src/macro_generated_method_mod.rs");
        let macro_generated_method_source =
            fs::read_to_string(_root.join("src/macro_generated_method_mod.rs")).unwrap();
        let macro_generated_method_qn = c(
            "cbm_ra_fixture_basic.macro_generated_method_mod.MacroGeneratedReceiver.macro_generated_cross_file_target",
        );
        let macro_item_qn = c("cbm_ra_fixture_basic.macro_target");
        let trait_qn = c("cbm_ra_fixture_basic.FriendlyGreeter.trait_target");
        let generic_qn = c("cbm_ra_fixture_basic.generic_target");
        let sites = [
            def_site_range(
                &caller_qn,
                &rel_path,
                line_of(source_text, "pub fn exercise_all_cases"),
                line_of(source_text, "pub async fn exercise_async_case"),
            ),
            def_site(
                &direct_qn,
                &rel_path,
                line_of(source_text, "pub fn direct_target"),
            ),
            def_site(
                &macro_method_qn,
                &macro_method_rel_path,
                line_of(&macro_method_source, "pub fn macro_cross_file_target"),
            ),
            def_site(
                &macro_item_qn,
                &rel_path,
                line_of(source_text, "pub fn macro_target"),
            ),
            def_site(
                &macro_generated_method_qn,
                &macro_generated_method_rel_path,
                line_of(
                    &macro_generated_method_source,
                    "pub fn macro_generated_cross_file_target",
                ),
            ),
            def_site(
                &trait_qn,
                &rel_path,
                line_of_after(
                    source_text,
                    "impl Greeter for FriendlyGreeter",
                    "fn trait_target",
                ),
            ),
            def_site(
                &generic_qn,
                &rel_path,
                line_of(source_text, "pub fn generic_target"),
            ),
        ];
        let mut out = empty_array();

        let status = cbm_rust_analyzer_resolve_batch_v2(
            root_c.as_ptr(),
            sites.as_ptr(),
            sites.len() as c_int,
            &file,
            1,
            &mut out,
        );

        assert_eq!(status, CbmRustStatus::Ok);
        let resolved = resolved_callees(&out);
        assert!(
            resolved.contains(&"cbm_ra_fixture_basic.direct_target"),
            "{resolved:?}"
        );
        assert!(
            resolved.contains(&"cbm_ra_fixture_basic.FriendlyGreeter.trait_target"),
            "{resolved:?}"
        );
        assert!(
            resolved.contains(&"cbm_ra_fixture_basic.generic_target"),
            "{resolved:?}"
        );
        assert!(
            resolved.contains(&"cbm_ra_fixture_basic.macro_target"),
            "{resolved:?}"
        );
        assert!(
            resolved.contains(
                &"cbm_ra_fixture_basic.macro_target_mod.MacroReceiver.macro_cross_file_target"
            ),
            "missing macro-expanded cross-file method resolution in {resolved:?}"
        );
        assert!(
            resolved.contains(
                &"cbm_ra_fixture_basic.macro_generated_method_mod.MacroGeneratedReceiver.macro_generated_cross_file_target"
            ),
            "missing cross-file macro-generated method resolution in {resolved:?}"
        );
        assert!(
            !resolved.contains(&"cbm_ra_fixture_basic.GeneratedByDerive.proc_macro_target"),
            "proc macro generated call must not emit guessed edge in {resolved:?}"
        );
        assert!(
            resolved
                .iter()
                .all(|callee| !callee.ends_with("direct_target")
                    || *callee == "cbm_ra_fixture_basic.direct_target"),
            "resolved by stable range, not short-name ambiguity: {resolved:?}"
        );

        cbm_rust_analyzer_free_resolved_call_array(&mut out);
    }

    #[test]
    fn capi_v2_duplicate_def_range_emits_no_ambiguous_edge() {
        let (_root, root_c, rel_path, source) = fixture_source();
        let module_qn = c("cbm_ra_fixture_basic");
        let source_text = source.to_str().unwrap();
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let trigger = c("direct_target");
        let call = call(&trigger, &caller);
        let file = CbmBatchRustLspFile {
            source: source.as_ptr(),
            source_len: source.as_bytes().len() as c_int,
            rel_path: rel_path.as_ptr(),
            module_qn: module_qn.as_ptr(),
            calls: &call,
            call_count: 1,
            imports: ptr::null(),
            import_count: 0,
        };
        let caller_qn = c("cbm_ra_fixture_basic.exercise_all_cases");
        let direct_qn = c("cbm_ra_fixture_basic.direct_target");
        let other_direct_qn = c("other.direct_target");
        let direct_line = line_of(source_text, "pub fn direct_target");
        let sites = [
            def_site(
                &caller_qn,
                &rel_path,
                line_of(source_text, "pub fn exercise_all_cases"),
            ),
            def_site(&direct_qn, &rel_path, direct_line),
            def_site(&other_direct_qn, &rel_path, direct_line),
        ];
        let mut out = empty_array();

        let status = cbm_rust_analyzer_resolve_batch_v2(
            root_c.as_ptr(),
            sites.as_ptr(),
            sites.len() as c_int,
            &file,
            1,
            &mut out,
        );

        assert_eq!(status, CbmRustStatus::Ok);
        assert!(!resolved_callees(&out).contains(&"cbm_ra_fixture_basic.direct_target"));
        cbm_rust_analyzer_free_resolved_call_array(&mut out);
    }

    #[test]
    fn legacy_capi_safe_empty_without_def_sites() {
        let root = fixture_root();
        let root_c = c(root.to_str().unwrap());
        let rel_path = c("src/lib.rs");
        let module_qn = c("cbm_ra_fixture_basic");
        let source_text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let source = c(&source_text);
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let callee = c("direct_target");
        let def_qn = c("cbm_ra_fixture_basic.direct_target");
        let def_short = c("direct_target");
        let call = call(&callee, &caller);
        let def = def(&def_qn, &def_short, &module_qn);
        let mut out = empty_array();

        let status = cbm_rust_analyzer_resolve_cross(
            root_c.as_ptr(),
            source.as_ptr(),
            source.as_bytes().len() as c_int,
            rel_path.as_ptr(),
            module_qn.as_ptr(),
            &def,
            1,
            ptr::null(),
            0,
            &call,
            1,
            &mut out,
        );

        assert_eq!(status, CbmRustStatus::Ok);
        assert_eq!(out.count, 0);
        assert!(out.items.is_null());
    }

    #[test]
    fn legacy_capi_safe_empty_for_macro_proc_macro_and_trait_inputs() {
        let root = fixture_root();
        let root_c = c(root.to_str().unwrap());
        let rel_path = c("src/lib.rs");
        let module_qn = c("cbm_ra_fixture_basic");
        let source_text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let source = c(&source_text);
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let trait_callee = c("trait_target");
        let macro_callee = c("macro_target");
        let proc_callee = c("proc_macro_target");
        let macro_expansion_callee = c("call_direct_target");
        let calls = [
            call(&trait_callee, &caller),
            call(&macro_callee, &caller),
            call(&proc_callee, &caller),
            call(&macro_expansion_callee, &caller),
        ];
        let trait_qn = c("cbm_ra_fixture_basic.FriendlyGreeter.trait_target");
        let macro_qn = c("cbm_ra_fixture_basic.macro_target");
        let proc_qn = c("cbm_ra_fixture_basic.GeneratedByDerive.proc_macro_target");
        let macro_only_qn = c("cbm_ra_fixture_basic.macro_only_target");
        let trait_short = c("trait_target");
        let macro_short = c("macro_target");
        let proc_short = c("proc_macro_target");
        let macro_only_short = c("macro_only_target");
        let defs = [
            def(&trait_qn, &trait_short, &module_qn),
            def(&macro_qn, &macro_short, &module_qn),
            def(&proc_qn, &proc_short, &module_qn),
            def(&macro_only_qn, &macro_only_short, &module_qn),
        ];
        let mut out = empty_array();

        let status = cbm_rust_analyzer_resolve_cross(
            root_c.as_ptr(),
            source.as_ptr(),
            source.as_bytes().len() as c_int,
            rel_path.as_ptr(),
            module_qn.as_ptr(),
            defs.as_ptr(),
            defs.len() as c_int,
            ptr::null(),
            0,
            calls.as_ptr(),
            calls.len() as c_int,
            &mut out,
        );

        assert_eq!(status, CbmRustStatus::Ok);
        assert_eq!(out.count, 0);
        assert!(out.items.is_null());
    }

    #[test]
    fn legacy_capi_ambiguous_def_emits_no_edge() {
        let root = fixture_root();
        let root_c = c(root.to_str().unwrap());
        let rel_path = c("src/lib.rs");
        let module_qn = c("cbm_ra_fixture_basic");
        let source_text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let source = c(&source_text);
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let callee = c("direct_target");
        let def_qn_one = c("cbm_ra_fixture_basic.direct_target");
        let def_qn_two = c("other.direct_target");
        let def_short = c("direct_target");
        let call = call(&callee, &caller);
        let defs = [
            def(&def_qn_one, &def_short, &module_qn),
            def(&def_qn_two, &def_short, &module_qn),
        ];
        let mut out = empty_array();

        let status = cbm_rust_analyzer_resolve_cross(
            root_c.as_ptr(),
            source.as_ptr(),
            source.as_bytes().len() as c_int,
            rel_path.as_ptr(),
            module_qn.as_ptr(),
            defs.as_ptr(),
            defs.len() as c_int,
            ptr::null(),
            0,
            &call,
            1,
            &mut out,
        );

        assert_eq!(status, CbmRustStatus::Ok);
        assert_eq!(out.count, 0);
        assert!(out.items.is_null());
    }

    fn resolved_callees(out: &CBMResolvedCallArray) -> Vec<&'static str> {
        if out.items.is_null() || out.count <= 0 {
            return Vec::new();
        }
        (0..out.count)
            .map(|idx| unsafe { &*out.items.add(idx as usize) })
            .map(|item| str_from_ptr(item.callee_qn))
            .collect()
    }

    fn str_from_ptr(ptr: *const c_char) -> &'static str {
        unsafe { CStr::from_ptr(ptr) }.to_str().unwrap()
    }
}
