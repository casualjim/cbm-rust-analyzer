#include <rust_analyzer_lsp.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *join_path(const char *root, const char *rel) {
    size_t root_len = strlen(root);
    size_t rel_len = strlen(rel);
    int needs_slash = root_len > 0 && root[root_len - 1] != '/';
    char *out = (char *)malloc(root_len + (size_t)needs_slash + rel_len + 1);
    if (!out)
        return NULL;
    memcpy(out, root, root_len);
    size_t pos = root_len;
    if (needs_slash)
        out[pos++] = '/';
    memcpy(out + pos, rel, rel_len);
    out[pos + rel_len] = '\0';
    return out;
}

static char *read_file(const char *path, int *out_len) {
    FILE *file = fopen(path, "rb");
    if (!file)
        return NULL;
    if (fseek(file, 0, SEEK_END) != 0) {
        fclose(file);
        return NULL;
    }
    long size = ftell(file);
    if (size < 0) {
        fclose(file);
        return NULL;
    }
    rewind(file);
    char *buf = (char *)malloc((size_t)size + 1);
    if (!buf) {
        fclose(file);
        return NULL;
    }
    size_t nread = fread(buf, 1, (size_t)size, file);
    fclose(file);
    buf[nread] = '\0';
    *out_len = (int)nread;
    return buf;
}

static unsigned line_of(const char *source, const char *needle) {
    unsigned line = 1;
    const char *cursor = source;
    size_t needle_len = strlen(needle);
    while (*cursor) {
        const char *line_end = strchr(cursor, '\n');
        size_t len = line_end ? (size_t)(line_end - cursor) : strlen(cursor);
        if (len >= needle_len && strstr(cursor, needle) && strstr(cursor, needle) < cursor + len)
            return line;
        if (!line_end)
            break;
        cursor = line_end + 1;
        line++;
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: %s <fixture-root>\n", argv[0]);
        return 2;
    }

    const char *root = argv[1];
    const char *rel_path = "src/lib.rs";
    char *lib_path = join_path(root, rel_path);
    if (!lib_path) {
        fprintf(stderr, "failed to allocate path\n");
        return 2;
    }

    int source_len = 0;
    char *source = read_file(lib_path, &source_len);
    free(lib_path);
    if (!source) {
        fprintf(stderr, "failed to read fixture source\n");
        return 2;
    }

    unsigned caller_start = line_of(source, "pub fn exercise_all_cases");
    unsigned caller_end = line_of(source, "pub async fn exercise_async_case");
    unsigned direct_line = line_of(source, "pub fn direct_target");
    if (!caller_start || !caller_end || !direct_line) {
        fprintf(stderr, "failed to locate fixture markers\n");
        free(source);
        return 2;
    }

    RustAnalyzerCall calls[] = {
        {
            .callee_name = "direct_target",
            .enclosing_func_qn = "cbm_ra_fixture_basic.exercise_all_cases",
        },
    };
    RustAnalyzerFile files[] = {
        {
            .source = source,
            .source_len = source_len,
            .rel_path = rel_path,
            .module_qn = "cbm_ra_fixture_basic",
            .calls = calls,
            .call_count = 1,
            .imports = NULL,
            .import_count = 0,
        },
    };
    RustAnalyzerDefSite def_sites[] = {
        {
            .qualified_name = "cbm_ra_fixture_basic.exercise_all_cases",
            .rel_path = rel_path,
            .start_line = caller_start,
            .end_line = caller_end,
        },
        {
            .qualified_name = "cbm_ra_fixture_basic.direct_target",
            .rel_path = rel_path,
            .start_line = direct_line,
            .end_line = direct_line,
        },
    };
    RustAnalyzerResolvedCallArray out = {0};
    RustAnalyzerStatus status = rust_analyzer_resolve_batch(root, def_sites, 2, files, 1, &out);
    free(source);

    if (status != RUST_ANALYZER_OK) {
        fprintf(stderr, "rust_analyzer_resolve_batch status=%d\n", status);
        return 1;
    }

    int found_direct = 0;
    for (int i = 0; i < out.count; i++) {
        const RustAnalyzerResolvedCall *edge = &out.items[i];
        if (edge->caller_qn && edge->callee_qn)
            printf("%s -> %s\n", edge->caller_qn, edge->callee_qn);
        if (edge->caller_qn && edge->callee_qn &&
            strcmp(edge->caller_qn, "cbm_ra_fixture_basic.exercise_all_cases") == 0 &&
            strcmp(edge->callee_qn, "cbm_ra_fixture_basic.direct_target") == 0) {
            found_direct = 1;
        }
    }

    rust_analyzer_free_resolved_call_array(&out);
    if (!found_direct) {
        fprintf(stderr, "missing direct edge\n");
        return 1;
    }
    return 0;
}
