# Prototype: toolchain

This is the toolchain prototype of milestone 0 (spec section 19). It tests spec 13.2: the
toolchain of the **Install toolchain** button builds a node with C code, with no Visual Studio,
no Windows SDK and no admin rights. The result is in [STATUS.md](../../STATUS.md).

## What it does

`toolchain run <work>` does these steps in `<work>/patina`, and writes `<work>/report.json`:

1. Downloads `rustup-init.exe`, the stable channel manifest and the packages of the minimal
   profile with `curl.exe`, and checks each SHA-256 hash.
2. Installs Rust for `x86_64-pc-windows-gnu` from this local mirror into `toolchain/rustup`
   and `toolchain/cargo`, with `--no-modify-path`.
3. Downloads the newest Zig release, checks its hash and unpacks it with `tar.exe` into
   `toolchain/zig`.
4. Copies its own executable to `zig-cc.exe` and `zig-ar.exe`. These wrappers start Zig.
5. Writes the [fixture](fixture) project to `projects/fixture`, and downloads its crates. The
   executable holds the files of the fixture.
6. Builds the node library offline, with a clean environment: `PATH` has only the Rust tools
   and System32. Zig compiles the C code, and `rust-lld` links.
7. Loads the library in a new process with the clean environment, and calls it.
8. Moves the folder to a name with spaces, an e with an acute accent and a CJK letter
   (`Patina T\u{e9}st \u{6587}`). Then it does steps 6 and 7 again.

The fixture has a crate with its own C file, and `libsqlite3-sys` with the bundled SQLite.

## Run it

The run downloads about 265 MB. Use an empty work folder:

```bash
cargo run -p toolchain -- run artifacts/toolchain-run
```

If a firewall allows only known programs, the new `cargo.exe` cannot download the crates. Then
set `PATINA_FETCH_CARGO` to a `cargo.exe` that the firewall allows. The build stays offline.

Delete the work folder after the run. It holds about 1.4 GB.

## Run it on a clean computer

The normal build needs `VCRUNTIME140.dll`, and a clean computer does not have it. Make a static
build on a computer with Rust:

```bash
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release -p toolchain --target-dir target/static
```

Copy `target/static/release/toolchain.exe` to the clean computer. Then run
`toolchain.exe run <empty folder>` under a standard account. The file needs no other files.
