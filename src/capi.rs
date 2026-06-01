use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_float, c_int},
    panic::{UnwindSafe, catch_unwind},
    path::Path,
    ptr,
};

use crate::{
    CbmCallInput, ResolvedCall, ResolverConfig, RustDefSite, SemanticResolveError, load_workspace,
    resolve_cbm_calls_by_def_sites,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, non_camel_case_types)]
pub enum RustAnalyzerStatus {
    RUST_ANALYZER_OK = 0,
    RUST_ANALYZER_INVALID_ARGUMENT = 1,
    RUST_ANALYZER_WORKSPACE_LOAD = 2,
    RUST_ANALYZER_ANALYSIS = 3,
    RUST_ANALYZER_TIMEOUT = 4,
    RUST_ANALYZER_CANCELLED = 5,
    RUST_ANALYZER_UNSUPPORTED = 6,
    RUST_ANALYZER_INTERNAL = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RustAnalyzerCall {
    pub callee_name: *const c_char,
    pub enclosing_func_qn: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RustAnalyzerImport {
    pub local_name: *const c_char,
    pub module_path: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RustAnalyzerFile {
    pub source: *const c_char,
    pub source_len: c_int,
    pub rel_path: *const c_char,
    pub module_qn: *const c_char,
    pub calls: *const RustAnalyzerCall,
    pub call_count: c_int,
    pub imports: *const RustAnalyzerImport,
    pub import_count: c_int,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RustAnalyzerDefSite {
    pub qualified_name: *const c_char,
    pub rel_path: *const c_char,
    pub start_line: u32,
    pub end_line: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RustAnalyzerResolvedCall {
    pub caller_qn: *const c_char,
    pub callee_qn: *const c_char,
    pub strategy: *const c_char,
    pub confidence: c_float,
    pub reason: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RustAnalyzerResolvedCallArray {
    pub items: *mut RustAnalyzerResolvedCall,
    pub count: c_int,
    pub cap: c_int,
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_analyzer_resolve_batch(
    workspace_root: *const c_char,
    def_sites: *const RustAnalyzerDefSite,
    def_site_count: c_int,
    files: *const RustAnalyzerFile,
    file_count: c_int,
    out: *mut RustAnalyzerResolvedCallArray,
) -> RustAnalyzerStatus {
    catch_ffi_status(|| unsafe {
        let workspace_root =
            c_str(workspace_root).ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
        let def_sites = c_slice(def_sites, def_site_count)?;
        let files = c_slice(files, file_count)?;
        if out.is_null() {
            return Err(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT);
        }
        let workspace = load_workspace(ResolverConfig::trusted_full(workspace_root))
            .map_err(|_| RustAnalyzerStatus::RUST_ANALYZER_WORKSPACE_LOAD)?;
        let def_sites = rust_def_sites_from_c(def_sites)?;
        let mut calls = Vec::new();
        for file in files {
            c_str(file.rel_path).ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
            c_str(file.module_qn).ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
            c_source(file.source, file.source_len)?;
            validate_imports(file.imports, file.import_count)?;
            calls.extend(call_inputs_from_c(c_slice(file.calls, file.call_count)?));
        }
        let resolved = resolve_cbm_calls_by_def_sites(&workspace, &calls, &def_sites)
            .map_err(|err| semantic_error_to_status(&err))?;
        write_resolved_calls(out, resolved)?;
        Ok(RustAnalyzerStatus::RUST_ANALYZER_OK)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_analyzer_free_resolved_call_array(out: *mut RustAnalyzerResolvedCallArray) {
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
    def_sites: &'a [RustAnalyzerDefSite],
) -> Result<Vec<RustDefSite<'a>>, RustAnalyzerStatus> {
    def_sites
        .iter()
        .map(|def| unsafe {
            let qualified_name = c_str(def.qualified_name)
                .ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
            let rel_path =
                c_str(def.rel_path).ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
            if def.start_line == 0 || def.end_line < def.start_line {
                return Err(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT);
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

unsafe fn call_inputs_from_c(calls: &[RustAnalyzerCall]) -> Vec<CbmCallInput<'_>> {
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

fn semantic_error_to_status(error: &SemanticResolveError) -> RustAnalyzerStatus {
    match error {
        SemanticResolveError::FileNotInWorkspace(_) => RustAnalyzerStatus::RUST_ANALYZER_ANALYSIS,
        SemanticResolveError::Analysis(_) => RustAnalyzerStatus::RUST_ANALYZER_ANALYSIS,
    }
}

fn catch_ffi_status(
    f: impl FnOnce() -> Result<RustAnalyzerStatus, RustAnalyzerStatus> + UnwindSafe,
) -> RustAnalyzerStatus {
    match catch_unwind(f) {
        Ok(Ok(status)) => status,
        Ok(Err(status)) => status,
        Err(_) => RustAnalyzerStatus::RUST_ANALYZER_INTERNAL,
    }
}

unsafe fn c_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

unsafe fn c_source<'a>(ptr: *const c_char, len: c_int) -> Result<&'a str, RustAnalyzerStatus> {
    if len < 0 || (len > 0 && ptr.is_null()) {
        return Err(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT);
    }
    if len == 0 {
        return Ok("");
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len as usize) };
    std::str::from_utf8(bytes).map_err(|_| RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)
}

unsafe fn c_slice<'a, T>(ptr: *const T, count: c_int) -> Result<&'a [T], RustAnalyzerStatus> {
    if count < 0 {
        return Err(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT);
    }
    if count == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT);
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, count as usize) })
}

unsafe fn validate_imports(
    imports: *const RustAnalyzerImport,
    import_count: c_int,
) -> Result<(), RustAnalyzerStatus> {
    for import in unsafe { c_slice(imports, import_count)? } {
        unsafe {
            c_str(import.local_name).ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
            c_str(import.module_path).ok_or(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
        }
    }
    Ok(())
}

unsafe fn write_resolved_calls(
    out: *mut RustAnalyzerResolvedCallArray,
    calls: Vec<ResolvedCall>,
) -> Result<(), RustAnalyzerStatus> {
    if out.is_null() {
        return Err(RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT);
    }
    unsafe {
        let mut items = Vec::with_capacity(calls.len());
        for call in calls {
            let caller_qn = into_c_string(call.caller_qn)?;
            let callee_qn = into_c_string(call.callee_qn)?;
            let strategy = into_c_string(call.strategy)?;
            let reason = match call.reason {
                Some(reason) => into_c_string(reason)?,
                None => ptr::null(),
            };
            items.push(RustAnalyzerResolvedCall {
                caller_qn,
                callee_qn,
                strategy,
                confidence: f32::from(call.confidence_bps) / 10_000.0,
                reason,
            });
        }
        let count = c_int::try_from(items.len())
            .map_err(|_| RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
        let cap = c_int::try_from(items.capacity())
            .map_err(|_| RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)?;
        let ptr = items.as_mut_ptr();
        std::mem::forget(items);
        *out = RustAnalyzerResolvedCallArray {
            items: ptr,
            count,
            cap,
        };
        Ok(())
    }
}

unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

fn into_c_string(value: String) -> Result<*const c_char, RustAnalyzerStatus> {
    CString::new(value)
        .map(CString::into_raw)
        .map(|ptr| ptr.cast_const())
        .map_err(|_| RustAnalyzerStatus::RUST_ANALYZER_INVALID_ARGUMENT)
}

const fn empty_array() -> RustAnalyzerResolvedCallArray {
    RustAnalyzerResolvedCallArray {
        items: ptr::null_mut(),
        count: 0,
        cap: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::raw::c_uint, path::PathBuf};

    fn c(value: &str) -> CString {
        CString::new(value).unwrap()
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic_crate")
    }

    fn call(callee: &CString, caller: &CString) -> RustAnalyzerCall {
        RustAnalyzerCall {
            callee_name: callee.as_ptr(),
            enclosing_func_qn: caller.as_ptr(),
        }
    }

    fn def_site(qn: &CString, rel_path: &CString, line: c_uint) -> RustAnalyzerDefSite {
        def_site_range(qn, rel_path, line, line)
    }

    fn def_site_range(
        qn: &CString,
        rel_path: &CString,
        start_line: c_uint,
        end_line: c_uint,
    ) -> RustAnalyzerDefSite {
        RustAnalyzerDefSite {
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
    fn capi_resolves_edges_by_ra_target_ranges() {
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
        let file = RustAnalyzerFile {
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

        let status = rust_analyzer_resolve_batch(
            root_c.as_ptr(),
            sites.as_ptr(),
            sites.len() as c_int,
            &file,
            1,
            &mut out,
        );

        assert_eq!(status, RustAnalyzerStatus::RUST_ANALYZER_OK);
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

        rust_analyzer_free_resolved_call_array(&mut out);
    }

    #[test]
    fn capi_duplicate_def_range_emits_no_ambiguous_edge() {
        let (_root, root_c, rel_path, source) = fixture_source();
        let module_qn = c("cbm_ra_fixture_basic");
        let source_text = source.to_str().unwrap();
        let caller = c("cbm_ra_fixture_basic.exercise_all_cases");
        let trigger = c("direct_target");
        let call = call(&trigger, &caller);
        let file = RustAnalyzerFile {
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

        let status = rust_analyzer_resolve_batch(
            root_c.as_ptr(),
            sites.as_ptr(),
            sites.len() as c_int,
            &file,
            1,
            &mut out,
        );

        assert_eq!(status, RustAnalyzerStatus::RUST_ANALYZER_OK);
        assert!(!resolved_callees(&out).contains(&"cbm_ra_fixture_basic.direct_target"));
        rust_analyzer_free_resolved_call_array(&mut out);
    }

    fn resolved_callees(out: &RustAnalyzerResolvedCallArray) -> Vec<&'static str> {
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
