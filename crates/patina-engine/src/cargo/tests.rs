use super::*;
use serde_json::json;

fn message(level: &str, text: &str, file: &Path) -> Value {
    let span = json!({ "is_primary": true, "file_name": file, "line_start": 3, "column_start": 5, "line_end": 3, "column_end": 9, "label": "here" });
    json!({ "reason": "compiler-message", "message": { "level": level, "message": text, "rendered": text, "spans": [span] } })
}

#[test]
fn a_newer_rust_builds_a_project_of_an_older_rust() {
    for (release, rust, fits) in [
        ("1.100.0", "1.94.0", true),
        ("1.94.0", "1.94.0", true),
        ("1.94.1", "1.94", true),
        ("1.94.0", "1.100.0", false),
        ("1.95.0-nightly", "1.94.0", true),
        ("1.95.0-beta.2", "1.95.0", false),
        ("1.95.0", "1.95.0-beta.2", true),
        ("1.94.0", "x", false),
        ("x", "x", true),
        ("1.94.0.1", "1.94.0", false),
    ] {
        assert_eq!(release_fits(release, rust), fits, "{release} {rust}");
    }
}

#[test]
fn the_sdk_manifests_have_the_dependencies_of_the_crates() {
    let gpu = include_str!("../../../patina-gpu/Cargo.toml");
    let cuda = include_str!("../../../patina-cuda/Cargo.toml");
    let event = include_str!("../../../patina-event/Cargo.toml");
    for (name, own) in [("patina-gpu", gpu), ("patina-cuda", cuda), ("patina-event", event)] {
        let sdk = SDK_FILES.iter().find(|(path, _)| path.ends_with(&format!("{name}/Cargo.toml"))).unwrap().1;
        for line in own.lines().filter(|l| ["wgpu", "wesl", "cudarc", "tokio", "rust-version"].iter().any(|k| l.starts_with(k))) {
            assert!(sdk.contains(line), "{line}");
        }
    }
}

#[test]
fn project_workspaces_use_the_hash_seed_patch() {
    let manifest = workspace(Path::new("unused-test-project"));
    assert!(manifest.contains("foldhash = { path = \"build/sdk/patina-gpu/vendor/foldhash\" }"));
    assert!(SDK_FILES.iter().any(|(path, _)| *path == "build/sdk/patina-gpu/vendor/foldhash/src/fast.rs"));
}

#[test]
fn a_feature_change_gives_a_new_state_key() {
    let unit = |features: &[&str], opt: u32, debug: bool| {
        let profile = json!({ "opt_level": opt.to_string(), "debug_assertions": debug });
        unit_hash(&json!({ "reason": "compiler-artifact", "package_id": "dep 1.0", "features": features, "profile": profile }))
    };
    let edit = unit(&["std"], 0, false);
    assert_eq!(edit, unit(&["std"], 3, false), "a bake build keeps the states");
    assert_ne!(edit, unit(&["std", "extra"], 0, false), "a feature can change a layout");
    assert_ne!(edit, unit(&["std"], 0, true), "debug assertions can change a layout");
}

#[test]
fn a_crate_is_a_name_and_a_version() {
    for good in ["rand", "rand@0.9", "serde_json@=1.0.100", "a-b_c@^1.2-beta.1"] {
        assert!(check_crate(good).is_ok(), "{good}");
    }
    for bad in ["", "-x", "--path=..", "../x", "a b", "rand@", "rand@0.9 x", "1rand", "@1", "a@b@c"] {
        assert!(check_crate(bad).is_err(), "{bad}");
    }
}

/// Run it with `cargo test -p patina-engine -- --ignored`. It reads the index of crates.io.
#[test]
#[ignore = "needs the network"]
fn cargo_adds_a_crate_to_a_node() {
    let root = std::env::temp_dir().join(format!("patina-add-crate-{}", std::process::id()));
    let manifest = "[package]\nname = \"num\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n";
    write(&root, "nodes/num/Cargo.toml", manifest).unwrap();
    write(&root, "nodes/num/src/lib.rs", "").unwrap();
    let toolchain = Toolchain::detect().unwrap();
    let text = add_crate(&root, &toolchain, "nodes/num", "itoa@1", None).unwrap();
    assert!(text.contains("itoa = \"1"), "{text}");
    assert_eq!(std::fs::read_to_string(root.join("nodes/num/Cargo.toml")).unwrap(), manifest);
    let error = add_crate(&root, &toolchain, "nodes/num", "patina-no-such-crate-x", None).unwrap_err();
    assert!(error.contains("patina-no-such-crate-x"), "{error}");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_type_folder_goes_in_by_its_path() {
    let root = std::env::temp_dir().join(format!("patina-add-type-{}", std::process::id()));
    for name in ["nodes/num", "types/digit"] {
        let manifest = format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n", &name[6..]);
        write(&root, &format!("{name}/Cargo.toml"), &manifest).unwrap();
        write(&root, &format!("{name}/src/lib.rs"), "").unwrap();
    }
    // Cargo starts from the unsaved text, and the file keeps its old text.
    let unsaved = format!("{}# mine
", std::fs::read_to_string(root.join("nodes/num/Cargo.toml")).unwrap());
    let saved = std::fs::read_to_string(root.join("nodes/num/Cargo.toml")).unwrap();
    let text = add_crate(&root, &Toolchain::detect().unwrap(), "nodes/num", "digit", Some(&unsaved)).unwrap();
    assert!(text.contains("path = \"../../types/digit\"") && text.contains("# mine"), "{text}");
    assert_eq!(std::fs::read_to_string(root.join("nodes/num/Cargo.toml")).unwrap(), saved);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_own_toolchain_has_a_clean_environment() {
    let folder = std::env::temp_dir().join(format!("patina-toolchain-{}", std::process::id()));
    let names = |folder: &Path| -> Vec<String> {
        let env = own_env(folder, |name| Some(format!("old {name}").into())).unwrap();
        let path = &env.iter().find(|(name, _)| name == "PATH").unwrap().1;
        assert_eq!(std::env::split_paths(path).next(), Some(folder.join("cargo").join("bin")));
        env.into_iter().map(|(name, _)| name).collect()
    };
    let first = names(&folder);
    assert!(first.contains(&"CARGO_HOME".to_owned()) && !first.iter().any(|n| n.starts_with("CC_") || n == "RUSTFLAGS"));
    if cfg!(windows) {
        std::fs::create_dir_all(folder.join("zig")).unwrap();
        std::fs::write(folder.join("zig").join("zig-cc.exe"), "").unwrap();
        assert!(names(&folder).contains(&"CC_x86_64_pc_windows_gnu".to_owned()));
        assert!(!names(&folder).contains(&"CXXSTDLIB_x86_64_pc_windows_gnu".to_owned()));
        std::fs::write(folder.join("zig").join("zig-cxx.exe"), "").unwrap();
        let cxx = names(&folder);
        assert!(cxx.contains(&"CXX_x86_64_pc_windows_gnu".to_owned()) && cxx.contains(&"CXXSTDLIB_x86_64_pc_windows_gnu".to_owned()));
        std::fs::remove_dir_all(&folder).unwrap();
    }
    // A folder with no cargo gives the Rust of the system.
    assert_eq!(Toolchain::find(&folder).unwrap().own, None);
}

#[test]
fn build_scripts_of_the_own_toolchain_find_the_runtime() {
    let folder = std::env::temp_dir().join(format!("patina-runtime-{}", std::process::id()));
    let target = folder.join("rustlib").join("x");
    std::fs::create_dir_all(target.join("bin")).unwrap();
    let (cargo, rustc, release, host) = (PathBuf::new(), PathBuf::new(), String::new(), String::new());
    let toolchain = Toolchain { cargo, rustc, release, host, libdir: target.join("lib"), own: Some(folder.clone()) };
    let command = toolchain.command(Path::new("x")).unwrap();
    let path = command.get_envs().find(|(name, _)| *name == "PATH").and_then(|(_, value)| value).unwrap();
    assert_eq!(std::env::split_paths(path).last(), Some(target.join("bin")));
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn each_host_gets_its_link_flags() {
    let flags = |host: &str, own: Option<&str>| {
        let (cargo, rustc, release, libdir) = (PathBuf::new(), PathBuf::new(), String::new(), PathBuf::new());
        Toolchain { cargo, rustc, release, host: host.to_owned(), libdir, own: own.map(PathBuf::from) }.rustflags()
    };
    assert_eq!(flags("x86_64-pc-windows-msvc", None), "-Cprefer-dynamic\x1f-Clink-arg=/Brepro");
    assert_eq!(flags("x86_64-pc-windows-gnu", None), "-Cprefer-dynamic");
    let own = flags("x86_64-pc-windows-gnu", Some("t"));
    assert!(own.contains("\x1f-Clink-arg=--no-insert-timestamp\x1f-Cdlltool=t") && own.contains("zig-dlltool.exe\x1f"), "{own}");
    assert!(own.ends_with(&format!("\x1f-Lnative={}", Path::new("t").join("zig").join("cxx").display())), "{own}");
    assert!(flags("x86_64-unknown-linux-gnu", None).ends_with("\x1f-Clink-arg=-Wl,-rpath,$ORIGIN"));
    assert!(flags("aarch64-apple-darwin", None).ends_with("\x1f-Clink-arg=-Wl,-rpath,@loader_path"));
}

#[test]
fn a_member_glob_needs_a_folder() {
    let root = std::env::temp_dir().join(format!("patina-members-{}", std::process::id()));
    std::fs::create_dir_all(root.join("connections")).unwrap();
    assert!(workspace(&root).contains("members = []"));
    std::fs::create_dir_all(root.join("build/clusters/main")).unwrap();
    assert!(workspace(&root).contains("members = [\"build/clusters/*\"]"));
    assert!(workspace(&root).ends_with("\n[profile.edit.package.cluster-main]\ndebug = false\n"));
    std::fs::create_dir_all(root.join("nodes/num")).unwrap();
    assert!(workspace(&root).contains("members = [\"nodes/*\", \"build/clusters/*\"]"), "an empty folder matches no member");
    std::fs::create_dir_all(root.join("connections/byte")).unwrap();
    assert!(workspace(&root).contains("members = [\"nodes/*\", \"connections/*\", \"build/clusters/*\"]"));
    std::fs::create_dir_all(root.join("widgets/bar")).unwrap();
    assert!(workspace(&root).contains("members = [\"nodes/*\", \"widgets/*\", \"connections/*\", \"build/clusters/*\"]"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_newest_cluster_library_gives_the_built_mode() {
    let root = std::env::temp_dir().join(format!("patina-built-mode-{}", std::process::id()));
    assert_eq!(newest_mode(&root), None);
    let library = |mode: Mode, secs: u64| {
        let folder = root.join("build/target").join(mode.profile());
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(format!("cluster_batch.{}", std::env::consts::DLL_EXTENSION));
        let file = std::fs::File::create(&path).unwrap();
        file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs)).unwrap();
    };
    library(Mode::Edit, 100);
    std::fs::create_dir_all(root.join("build/target/bake")).unwrap();
    std::fs::write(root.join("build/target/bake/cluster_batch.d"), "").unwrap();
    assert_eq!(newest_mode(&root), Some(Mode::Edit), "only the edit library is there");
    library(Mode::Bake, 200);
    assert_eq!(newest_mode(&root), Some(Mode::Bake));
    library(Mode::Edit, 300);
    assert_eq!(newest_mode(&root), Some(Mode::Edit));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn only_the_clusters_of_the_plan_stay() {
    let root = std::env::temp_dir().join(format!("patina-old-clusters-{}", std::process::id()));
    for folder in ["main", "batch", "frame", "slot"] {
        std::fs::create_dir_all(root.join("build/clusters").join(folder)).unwrap();
    }
    // The order of read_dir depends on the file system, so the names are sorted.
    let left = || -> Vec<_> {
        let mut names: Vec<_> = std::fs::read_dir(root.join("build/clusters")).unwrap().flatten().map(|e| e.file_name()).collect();
        names.sort();
        names
    };
    remove_old_clusters(&root, &[Domain::Batch], true).unwrap();
    assert_eq!(left(), ["batch", "slot"]);
    remove_old_clusters(&root, &[Domain::Batch], false).unwrap();
    assert_eq!(left(), ["batch"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn compile_errors_point_to_project_files() {
    let root = std::env::temp_dir().join("patina-errors");
    let relative = Path::new("nodes").join("a").join("src").join("lib.rs");
    let lines = [
        message("error", "bad", &root.join(&relative)),
        message("error", "bad too", &relative),
        message("error", "outside", &std::env::temp_dir().join("crate").join("lib.rs")),
        message("warning", "unused", &relative),
        json!({ "reason": "compiler-message", "message": { "level": "error", "message": "aborting due to 2 previous errors", "spans": [] } }),
    ];
    let stdout = lines.iter().map(|l| format!("{l}\nnot json\n")).collect::<String>();
    let errors = compile_errors(&root, stdout.as_bytes());
    let files: Vec<Option<&str>> = errors.iter().map(|e| e.file.as_deref()).collect();
    assert_eq!(files, [Some("nodes/a/src/lib.rs"), Some("nodes/a/src/lib.rs"), None]);
    assert_eq!((errors[0].start, errors[0].end, errors[0].label.as_deref()), ([3, 5], [3, 9], Some("here")));
}

#[test]
fn a_stop_ends_cargo_and_its_build_script() {
    let root = std::env::temp_dir().join(format!("patina-stop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let manifest = "[package]\nname = \"slow\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n[profile.edit]\ninherits = \"dev\"\n";
    std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
    std::fs::write(root.join("src/lib.rs"), "").unwrap();
    let (started, done) = (root.join("started"), root.join("done"));
    let script = format!("fn main() {{ std::fs::write({started:?}, \"\").unwrap(); std::thread::sleep(std::time::Duration::from_secs(4)); std::fs::write({done:?}, \"\").unwrap(); }}");
    std::fs::write(root.join("build.rs"), script).unwrap();
    let stop = AtomicBool::new(false);
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            let until = std::time::Instant::now() + Duration::from_secs(120);
            while !started.exists() && std::time::Instant::now() < until {
                std::thread::sleep(Duration::from_millis(20));
            }
            stop.store(true, Ordering::Relaxed);
        });
        run_cargo(&root, &Toolchain::detect().unwrap(), &["slow".to_owned()], Mode::Edit, &|| {}, &stop)
    });
    assert!(started.exists(), "the build script did not start");
    assert!(matches!(result, Err(Error::Build(ref text)) if text == "the build stopped"));
    // On Linux and macOS, the build script of a killed cargo runs on (spec 13.5).
    if cfg!(windows) {
        std::thread::sleep(Duration::from_secs(5));
        assert!(!done.exists(), "the build script ran on after the stop");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_first_compiling_line_tells_and_fresh_crates_do_not() {
    let count = std::cell::Cell::new(0);
    let text = "   Compiling a v0.1.0\n   Compiling b v0.1.0\n    Finished `edit` profile\n";
    assert_eq!(status_lines(text.as_bytes(), &|| count.set(count.get() + 1)).unwrap(), text);
    assert_eq!(count.get(), 1);
    status_lines("    Finished `bake` profile\n".as_bytes(), &|| count.set(count.get() + 1)).unwrap();
    assert_eq!(count.get(), 1);
}
