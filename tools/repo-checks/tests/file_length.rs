//! The file length rule of `AGENTS.md`: a source file has 1000 lines or fewer.

use std::fs;
use std::path::Path;

use repo_checks::{MAX_LINES, repo_root, source_files, too_long};

#[test]
fn no_source_file_has_more_than_the_limit() {
    let files = source_files(&repo_root()).expect("read the repository");
    let this_file = Path::new("tools/repo-checks/tests/file_length.rs");
    assert!(
        files.iter().any(|f| f.path == this_file),
        "the check did not find its own test file, so it does not read the repository"
    );

    let long = too_long(&files, MAX_LINES);
    let list: Vec<String> = long
        .iter()
        .map(|f| format!("  {} lines: {}", f.lines, f.path.display()))
        .collect();
    assert!(
        long.is_empty(),
        "these source files have more than {MAX_LINES} lines:\n{}\n\
         Refactor each file, and look at the code around it (AGENTS.md, File length).",
        list.join("\n")
    );
}

#[test]
fn the_check_finds_a_long_file_and_skips_build_output() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("file-length");
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove the old test folder");
    }
    let write = |path: &str, lines: usize| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x\n".repeat(lines)).unwrap();
    };
    write("src/at_limit.rs", MAX_LINES);
    write("src/too_long.rs", MAX_LINES + 1);
    write("web/src/too_long.ts", MAX_LINES + 1);
    write("docs/long.md", MAX_LINES + 1);
    write("target/debug/generated.rs", MAX_LINES + 1);
    write("web/node_modules/dep/index.js", MAX_LINES + 1);
    write(".git/hook.sh", MAX_LINES + 1);
    write("toolchain/lib.rs", MAX_LINES + 1);

    let files = source_files(&root).unwrap();
    let mut found: Vec<String> = too_long(&files, MAX_LINES)
        .iter()
        .map(|f| f.path.to_string_lossy().replace('\\', "/"))
        .collect();
    found.sort();
    assert_eq!(found, ["src/too_long.rs", "web/src/too_long.ts"]);
    assert_eq!(files.len(), 3, "only the three source files outside the skipped folders");

    fs::remove_dir_all(&root).unwrap();
}
