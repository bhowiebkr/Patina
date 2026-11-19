//! The source of the SDK crates that the engine writes into each project (spec 8).

/// The SDK crates that a project builds with. The engine carries their source.
/// A node crate that uses the GPU depends on `patina-gpu` or `patina-cuda` (10), so other
/// projects do not build `wgpu` or `cudarc`.
pub(super) const SDK_FILES: [(&str, &str); 47] = [
    (
        "build/sdk/patina-sdk/Cargo.toml",
        "[package]\nname = \"patina-sdk\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\npatina-macros = { path = \"../patina-macros\" }\n",
    ),
    ("build/sdk/patina-sdk/src/lib.rs", include_str!("../../../patina-sdk/src/lib.rs")),
    ("build/sdk/patina-sdk/src/glue.rs", include_str!("../../../patina-sdk/src/glue.rs")),
    ("build/sdk/patina-sdk/src/glue/call.rs", include_str!("../../../patina-sdk/src/glue/call.rs")),
    ("build/sdk/patina-sdk/src/glue/catch.rs", include_str!("../../../patina-sdk/src/glue/catch.rs")),
    ("build/sdk/patina-sdk/src/glue/slot.rs", include_str!("../../../patina-sdk/src/glue/slot.rs")),
    ("build/sdk/patina-sdk/src/glue/probe.rs", include_str!("../../../patina-sdk/src/glue/probe.rs")),
    ("build/sdk/patina-sdk/src/glue/keep.rs", include_str!("../../../patina-sdk/src/glue/keep.rs")),
    ("build/sdk/patina-sdk/src/glue/view.rs", include_str!("../../../patina-sdk/src/glue/view.rs")),
    ("build/sdk/patina-sdk/src/glue/latest.rs", include_str!("../../../patina-sdk/src/glue/latest.rs")),
    ("build/sdk/patina-sdk/src/glue/atom.rs", include_str!("../../../patina-sdk/src/glue/atom.rs")),
    ("build/sdk/patina-sdk/src/glue/net.rs", include_str!("../../../patina-sdk/src/glue/net.rs")),
    ("build/sdk/patina-sdk/src/glue/abi.rs", include_str!("../../../patina-sdk/src/glue/abi.rs")),
    ("build/sdk/patina-sdk/src/glue/host.rs", include_str!("../../../patina-sdk/src/glue/host.rs")),
    ("build/sdk/patina-sdk/src/audio.rs", include_str!("../../../patina-sdk/src/audio.rs")),
    ("build/sdk/patina-sdk/src/graph.rs", include_str!("../../../patina-sdk/src/graph.rs")),
    ("build/sdk/patina-sdk/src/view.rs", include_str!("../../../patina-sdk/src/view.rs")),
    ("build/sdk/patina-sdk/src/ui.rs", include_str!("../../../patina-sdk/src/ui.rs")),
    ("build/sdk/patina-sdk/src/ui/code.rs", include_str!("../../../patina-sdk/src/ui/code.rs")),
    ("build/sdk/patina-sdk/src/ui/layers.rs", include_str!("../../../patina-sdk/src/ui/layers.rs")),
    ("build/sdk/patina-sdk/src/ui/layout.rs", include_str!("../../../patina-sdk/src/ui/layout.rs")),
    ("build/sdk/patina-sdk/src/ui/path.rs", include_str!("../../../patina-sdk/src/ui/path.rs")),
    ("build/sdk/patina-sdk/src/ui/ramp.rs", include_str!("../../../patina-sdk/src/ui/ramp.rs")),
    ("build/sdk/patina-sdk/src/ui/timeline.rs", include_str!("../../../patina-sdk/src/ui/timeline.rs")),
    ("build/sdk/patina-sdk/src/ui/transform.rs", include_str!("../../../patina-sdk/src/ui/transform.rs")),
    (
        "build/sdk/patina-macros/Cargo.toml",
        "[package]\nname = \"patina-macros\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [lib]\nproc-macro = true\n",
    ),
    ("build/sdk/patina-macros/src/lib.rs", include_str!("../../../patina-macros/src/lib.rs")),
    (
        "build/sdk/patina-gpu/Cargo.toml",
        "[package]\nname = \"patina-gpu\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\
         rust-version = \"1.100\"\n\n[dependencies]\npatina-sdk = { path = \"../patina-sdk\" }\n\
         wgpu = { version = \"30\", default-features = false, features = [\"std\", \"parking_lot\", \"dx12\", \"metal\", \"vulkan\", \"wgsl\"] }\n\
         wesl = { version = \"0.6\", optional = true }\n\n\
         [target.'cfg(all(windows, target_arch = \"x86_64\"))'.dependencies]\n\
         wgpu = { version = \"30\", default-features = false, features = [\"static-dxc\"] }\n\n\
         [features]\ndefault = [\"wesl\"]\n",
    ),
    ("build/sdk/patina-gpu/src/lib.rs", include_str!("../../../patina-gpu/src/lib.rs")),
    ("build/sdk/patina-gpu/build.rs", include_str!("../../../patina-gpu/build.rs")),
    ("build/sdk/patina-gpu/vendor/foldhash/Cargo.toml", include_str!("../../../patina-gpu/vendor/foldhash/Cargo.toml")),
    ("build/sdk/patina-gpu/vendor/foldhash/LICENSE", include_str!("../../../patina-gpu/vendor/foldhash/LICENSE")),
    ("build/sdk/patina-gpu/vendor/foldhash/README.md", include_str!("../../../patina-gpu/vendor/foldhash/README.md")),
    ("build/sdk/patina-gpu/vendor/foldhash/PATINA.md", include_str!("../../../patina-gpu/vendor/foldhash/PATINA.md")),
    ("build/sdk/patina-gpu/vendor/foldhash/src/lib.rs", include_str!("../../../patina-gpu/vendor/foldhash/src/lib.rs")),
    ("build/sdk/patina-gpu/vendor/foldhash/src/fast.rs", include_str!("../../../patina-gpu/vendor/foldhash/src/fast.rs")),
    ("build/sdk/patina-gpu/vendor/foldhash/src/quality.rs", include_str!("../../../patina-gpu/vendor/foldhash/src/quality.rs")),
    ("build/sdk/patina-gpu/vendor/foldhash/src/seed.rs", include_str!("../../../patina-gpu/vendor/foldhash/src/seed.rs")),
    ("build/sdk/patina-gpu/vendor/foldhash/src/convenience.rs", include_str!("../../../patina-gpu/vendor/foldhash/src/convenience.rs")),
    ("build/sdk/patina-gpu/src/image.rs", include_str!("../../../patina-gpu/src/image.rs")),
    ("build/sdk/patina-gpu/src/link.rs", include_str!("../../../patina-gpu/src/link.rs")),
    ("build/sdk/patina-gpu/src/viewport.rs", include_str!("../../../patina-gpu/src/viewport.rs")),
    ("build/sdk/patina-gpu/src/shaders/image.wesl", include_str!("../../../patina-gpu/src/shaders/image.wesl")),
    (
        "build/sdk/patina-cuda/Cargo.toml",
        "[package]\nname = \"patina-cuda\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\npatina-sdk = { path = \"../patina-sdk\" }\n\
         cudarc = { version = \"0.19\", default-features = false, features = [\"std\", \"driver\", \"nvrtc\", \"dynamic-loading\", \"cuda-12060\"] }\n",
    ),
    ("build/sdk/patina-cuda/src/lib.rs", include_str!("../../../patina-cuda/src/lib.rs")),
    (
        "build/sdk/patina-event/Cargo.toml",
        "[package]\nname = \"patina-event\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\npatina-sdk = { path = \"../patina-sdk\" }\n\
         tokio = { version = \"1\", default-features = false, features = [\"rt-multi-thread\", \"time\"] }\n",
    ),
    ("build/sdk/patina-event/src/lib.rs", include_str!("../../../patina-event/src/lib.rs")),
];

#[cfg(test)]
mod tests {
    use super::SDK_FILES;
    use std::path::Path;

    /// Each source file of an SDK crate goes into the project, or the project does not compile.
    /// The `tests.rs` files are only for the tests of the SDK.
    #[test]
    fn the_engine_carries_each_source_file_of_the_sdk_crates() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut dirs: Vec<_> = ["patina-sdk", "patina-macros", "patina-gpu", "patina-cuda", "patina-event"]
            .iter()
            .map(|name| (format!("build/sdk/{name}/src/"), crates.join(name).join("src")))
            .collect();
        while let Some((prefix, dir)) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                let file = path.file_name().unwrap().to_str().unwrap();
                let want = format!("{prefix}{file}");
                if path.is_dir() {
                    dirs.push((want + "/", path));
                } else if file != "tests.rs" {
                    assert!(SDK_FILES.iter().any(|(p, _)| *p == want), "{want} is not in SDK_FILES");
                }
            }
        }
    }
}
