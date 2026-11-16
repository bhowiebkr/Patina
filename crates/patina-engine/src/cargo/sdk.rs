//! The source of the SDK crates that the engine writes into each project (spec 8).

/// The SDK crates that a project builds with. The engine carries their source.
/// A node crate that uses the GPU depends on `patina-gpu` or `patina-cuda` (10), so other
/// projects do not build `wgpu` or `cudarc`.
pub(super) const SDK_FILES: [(&str, &str); 25] = [
    (
        "build/sdk/patina-sdk/Cargo.toml",
        "[package]\nname = \"patina-sdk\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\npatina-macros = { path = \"../patina-macros\" }\n",
    ),
    ("build/sdk/patina-sdk/src/lib.rs", include_str!("../../../patina-sdk/src/lib.rs")),
    ("build/sdk/patina-sdk/src/glue.rs", include_str!("../../../patina-sdk/src/glue.rs")),
    ("build/sdk/patina-sdk/src/glue/call.rs", include_str!("../../../patina-sdk/src/glue/call.rs")),
    ("build/sdk/patina-sdk/src/glue/slot.rs", include_str!("../../../patina-sdk/src/glue/slot.rs")),
    ("build/sdk/patina-sdk/src/glue/probe.rs", include_str!("../../../patina-sdk/src/glue/probe.rs")),
    ("build/sdk/patina-sdk/src/glue/keep.rs", include_str!("../../../patina-sdk/src/glue/keep.rs")),
    ("build/sdk/patina-sdk/src/glue/view.rs", include_str!("../../../patina-sdk/src/glue/view.rs")),
    ("build/sdk/patina-sdk/src/glue/latest.rs", include_str!("../../../patina-sdk/src/glue/latest.rs")),
    ("build/sdk/patina-sdk/src/audio.rs", include_str!("../../../patina-sdk/src/audio.rs")),
    ("build/sdk/patina-sdk/src/graph.rs", include_str!("../../../patina-sdk/src/graph.rs")),
    ("build/sdk/patina-sdk/src/view.rs", include_str!("../../../patina-sdk/src/view.rs")),
    ("build/sdk/patina-sdk/src/ui.rs", include_str!("../../../patina-sdk/src/ui.rs")),
    (
        "build/sdk/patina-macros/Cargo.toml",
        "[package]\nname = \"patina-macros\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [lib]\nproc-macro = true\n",
    ),
    ("build/sdk/patina-macros/src/lib.rs", include_str!("../../../patina-macros/src/lib.rs")),
    (
        "build/sdk/patina-gpu/Cargo.toml",
        "[package]\nname = \"patina-gpu\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\
         rust-version = \"1.97.1\"\n\n[dependencies]\npatina-sdk = { path = \"../patina-sdk\" }\n\
         wgpu = { version = \"30\", default-features = false, features = [\"std\", \"parking_lot\", \"dx12\", \"metal\", \"vulkan\", \"wgsl\"] }\n\
         wesl = \"0.6\"\n",
    ),
    ("build/sdk/patina-gpu/src/lib.rs", include_str!("../../../patina-gpu/src/lib.rs")),
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
