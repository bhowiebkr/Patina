//! Compiles the vendored Manifold and Clipper2 sources (vendor/README.md) into one static library.

use std::path::PathBuf;

const MANIFOLD: [&str; 19] = [
    "boolean3", "boolean_result", "constructors", "csg_tree", "edge_op", "execution_impl", "face_op", "impl", "manifold",
    "minkowski", "polygon", "properties", "quickhull", "sdf", "smoothing", "sort", "subdivision", "tree2d",
    "cross_section/cross_section",
];
const BINDINGS: [&str; 5] = ["box", "conv", "cross", "manifoldc", "rect"];
const CLIPPER: [&str; 4] = ["clipper.engine", "clipper.offset", "clipper.rectclip", "clipper.triangulation"];

fn main() {
    let vendor = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it")).join("vendor");
    let (manifold, clipper) = (vendor.join("manifold"), vendor.join("clipper2"));
    println!("cargo:rerun-if-changed=vendor");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .warnings(false)
        // The kernel is slow without optimization, and the library builds only one time.
        .opt_level(2)
        // -1 is the sequential backend: no oneTBB.
        .define("MANIFOLD_PAR", "-1")
        .define("MANIFOLD_CROSS_SECTION", None)
        .include(manifold.join("include"))
        .include(manifold.join("src"))
        .include(manifold.join("bindings/c/include"))
        .include(clipper.join("include"));
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        build.flag("/EHsc").flag("/bigobj");
    }
    build.files(MANIFOLD.iter().map(|name| manifold.join("src").join(format!("{name}.cpp"))));
    build.files(BINDINGS.iter().map(|name| manifold.join("bindings/c").join(format!("{name}.cpp"))));
    build.files(CLIPPER.iter().map(|name| clipper.join("src").join(format!("{name}.cpp"))));
    build.compile("manifold");
}
