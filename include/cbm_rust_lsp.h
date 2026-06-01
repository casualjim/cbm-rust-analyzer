#ifndef CBM_RUST_LSP_H
#define CBM_RUST_LSP_H

#include "cbm.h"
#include "lsp/go_lsp.h" /* CBMLSPDef, shared by existing cross-LSP resolvers */

#ifdef __cplusplus
extern "C" {
#endif

/* Low-level Rust-owned temp ABI. CBM wrapper copies rows into CBMArena, then frees temp out. */

typedef enum {
    CBM_RUST_OK = 0,
    CBM_RUST_INVALID_ARGUMENT = 1,
    CBM_RUST_WORKSPACE_LOAD = 2,
    CBM_RUST_ANALYSIS = 3,
    CBM_RUST_TIMEOUT = 4,
    CBM_RUST_CANCELLED = 5,
    CBM_RUST_UNSUPPORTED = 6,
    CBM_RUST_INTERNAL = 7,
} CbmRustStatus;

typedef struct {
    const char *source;
    int source_len;
    const char *rel_path;
    const char *module_qn;
    const CBMCall *calls;
    int call_count;
    const CBMImport *imports;
    int import_count;
} CBMBatchRustLSPFile;

typedef struct {
    const char *qualified_name;
    const char *rel_path;
    unsigned int start_line;
    unsigned int end_line;
} CBMRustLSPDefSite;

/* Exact CBM-side wrapper symbol. Safe-empty when caller cannot provide deterministic rel_path. */
void cbm_run_rust_lsp_cross(CBMArena *arena,
                            const char *workspace_root,
                            const char *source, int source_len,
                            const char *module_qn,
                            CBMLSPDef *defs, int def_count,
                            const CBMImport *imports, int import_count,
                            const CBMCall *calls, int call_count,
                            CBMResolvedCallArray *out);

/* Rust-owned semantic ABI. Requires rel_path for deterministic RA file identity. */
CbmRustStatus cbm_rust_analyzer_resolve_cross(const char *workspace_root,
                                               const char *source, int source_len,
                                               const char *rel_path,
                                               const char *module_qn,
                                               const CBMLSPDef *defs, int def_count,
                                               const CBMImport *imports, int import_count,
                                               const CBMCall *calls, int call_count,
                                               CBMResolvedCallArray *out);

CbmRustStatus cbm_rust_analyzer_resolve_batch(const char *workspace_root,
                                               const CBMLSPDef *defs, int def_count,
                                               const CBMBatchRustLSPFile *files, int file_count,
                                               CBMResolvedCallArray *out);

CbmRustStatus cbm_rust_analyzer_resolve_batch_v2(const char *workspace_root,
                                                  const CBMRustLSPDefSite *def_sites,
                                                  int def_site_count,
                                                  const CBMBatchRustLSPFile *files, int file_count,
                                                  CBMResolvedCallArray *out);

void cbm_rust_analyzer_free_resolved_call_array(CBMResolvedCallArray *out);

#ifdef __cplusplus
}
#endif

#endif /* CBM_RUST_LSP_H */
