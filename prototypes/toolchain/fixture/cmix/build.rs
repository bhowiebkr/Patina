fn main() {
    println!("cargo::rerun-if-changed=csrc/mix.c");
    cc::Build::new().file("csrc/mix.c").compile("mix");
}
