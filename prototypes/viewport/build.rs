#[path = "../../crates/patina-gpu/build.rs"]
mod gpu_build;

fn main() {
    gpu_build::main();
    tauri_build::build();
}
