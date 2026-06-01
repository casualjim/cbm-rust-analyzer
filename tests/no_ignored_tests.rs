use std::{fs, path::Path};

#[test]
fn repository_contains_no_ignored_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();
    scan_rs_files(root, root, &mut offenders);
    assert!(
        offenders.is_empty(),
        "ignored tests forbidden by SPEC V42:\n{}",
        offenders.join("\n")
    );
}

fn scan_rs_files(root: &Path, path: &Path, offenders: &mut Vec<String>) {
    if should_skip(root, path) {
        return;
    }
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            scan_rs_files(root, &entry.path(), offenders);
        }
        return;
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let forbidden = format!("#{}", "[ignore");
    for (idx, line) in text.lines().enumerate() {
        if line.contains(&forbidden) {
            let rel = path.strip_prefix(root).unwrap_or(path);
            offenders.push(format!("{}:{}: {line}", rel.display(), idx + 1));
        }
    }
}

fn should_skip(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .any(|component| {
            let text = component.as_os_str().to_string_lossy();
            matches!(text.as_ref(), ".git" | "target")
        })
}
