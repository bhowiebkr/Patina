# Status

The current milestone is milestone 1, the engine and the CLI (spec section 19). Milestone 0
ended on 2026-11-15. Its results follow the milestone 1 section, with three owner checks open.

## Milestone 1: engine and CLI

Item 1 of section 19 works. `patina-cli run <project folder>` compiles the node folders of a
project, loads the cluster and runs the main graph one time. It ran on 2026-11-15 on Windows 11
Pro, with Rust 1.94.0 and 1.100.0. It did not run on Linux or macOS on this computer.

### What works

- `patina-engine` reads `patina.toml` and `graphs/main.toml` (6.4). A key that Patina does not
  know, or a newer format, stops the read with a message.
- It reads the pins of each node from the `#[node]` function with `syn` (4.1). Inputs by value,
  by `&`, by `&mut` and as `&str` work. A tuple output and a `Result` output work.
- It checks the graph and gives all the problems at one time: the IDs, the folders, the
  parameter keys, the wire ends, two wires to one input and the cycles.
- It sorts the nodes in run order, with the order wires (9.2). Ties keep the file order.
- It writes the project workspace, the SDK source and one cluster crate, and builds them with
  cargo. It writes a file only when the content changes, so a build with no change is fast.
- It loads the cluster, checks the ABI version, sets the parameters from their text form (4.8)
  and runs the graph. On Windows, only the folder of the library and the system folders give
  other libraries.
- A node that returns `Err` or panics gives a report with its ID and folder. The nodes after it
  do not run (4.7, 15.3). A bad parameter text gives a report, and the graph does not run.
- [examples/hello](examples/hello) adds two numbers. It uses one node folder two times.

### What was checked

- `cargo test --workspace --locked` passed 29 tests. Two CLI tests build and run projects in
  the temporary folder of cargo:
  - `hello_adds_two_numbers`: the exit code is 0, and the output is `6.5`.
  - `failed_nodes_are_reported_and_stop_the_nodes_after_them`: the exit code is 1. The error
    and the panic reports have the node IDs, and the output is empty. Then the parameter text
    changes to `abc`, and the bad parameter report shows.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` passed with Rust 1.94.0 and
  1.100.0.
- A manual run of `examples/hello` with the CLI wrote `6.5`.

### Measurements

The projects are very small, so these times do not predict real projects.

| Measurement | Time |
| --- | --- |
| Cold build of `examples/hello`, with the SDK | 0.8 s |
| Rebuild after a change to the SDK source | 0.6 s |
| Build with no change | less than 0.05 s |
| The 2 CLI tests, with the cold build of both projects | 1.2 s |

### Known limits

- One cluster, with a static `std`, in the `dev` profile. Clusters that share types, hot reload
  and **Bake** come at milestone 6.
- Only function nodes. Stateful nodes, node settings and generic nodes come at milestone 5.
  Subgraphs come at milestone 7, and async nodes at milestone 12.
- The toolchain comes from the `CARGO` variable or from `PATH`, and its version must be the
  `rust` version of `patina.toml`. The own toolchain comes at milestone 3.
- The engine loads the library from the cargo target folder, and never unloads it. Windows
  locks the file, so one process cannot build the same project again after the load.
- A node panic shows two messages: the default panic message of the cluster, and the report.
- A `Result` output is found by the type name `Result` only. A type alias with another name is
  not found, and then an `Err` is a value, not a failure.
- The CLI runs the graph one time. The clock domains (9.1) come later.
- These shortcuts are open in the code:
  - The toolchain comes from `CARGO` or `PATH` (`crates/patina-engine/src/cargo.rs`).
  - One cluster with a static `std` in the `dev` profile (`crates/patina-engine/src/cargo.rs`).
  - The library loads from the target folder (`crates/patina-engine/src/lib.rs`).
  - Only function nodes (`crates/patina-engine/src/node.rs`,
    `crates/patina-macros/src/lib.rs`).

## Milestone 0: risk prototypes

| Prototype | Result | Platforms that ran |
| --- | --- | --- |
| Rust types across clusters (5.1, 5.2) | Passed | Windows 11 and CI `windows-latest`, `x86_64-pc-windows-msvc` |
| Hot reload (8.5) | Passed | Windows 11 and CI `windows-latest`, `x86_64-pc-windows-msvc` |
| Toolchain (13.2) | Passed, with the clean computer run open | Windows 11, `x86_64-pc-windows-gnu` |
| Viewport (15.8) | Passed, with one owner check open | Windows 11 |
| Connections (15.9) | Passed, with one owner check open | Windows 11 |

## Rust types across clusters

The code is in [prototypes/types-across-clusters](prototypes/types-across-clusters/README.md).
It ran on 2026-11-15 with Rust 1.94.0, on Windows 11 Pro, in the `dev` and the `release`
profile. It did not run on Linux or macOS.

[CI](.github/workflows/ci.yml) runs the test for each push to `main`. It passed in both
profiles on 2026-11-15, on the GitHub Actions `windows-latest` runner, with Rust 1.100.0. On
the Linux and macOS runners, clippy checks the prototype, but the test has no code to run.

### What was checked

The test `cargo test -p types-across-clusters` ran and passed these checks in both profiles:

- A `String`, a `Vec<Point>` and a `Probe` with `Drop` went from cluster A to cluster B. Cluster
  B read the `String` and took the other two values.
- Cluster B dropped the `Probe` that cluster A made. After the drop, both clusters showed the
  same counts: 0 live, 1 dropped. Thus the two clusters use one copy of `geometry`.
- The engine dropped the values that it kept with the drop function of cluster A.
- `distance` has the type name `units::Meters` on both sides, from `units` 1.0.0 and 2.0.0. The
  type keys are different, and the connection is refused.
- Build 2 changed the fields of `Point` from `u32` to `f32`. The name, the crate version, the
  size and the alignment stayed the same. The types library got a new file name.
- The `points` and `probe` pins of build 1 do not connect to build 2, and the engine refused
  the old values. The `String` of build 1 still went to build 2, because both builds use one
  `std`.
- The engine dropped the old values with the code of build 1. The counts of build 2 did not
  change. Then build 2 ran from cluster A to cluster B.
- Each cluster imports `std-*.dll` and `patina_types_*.dll`, and does not import the other
  cluster. Each import is in `build/` or in Windows.

### Findings

- **One copy of each type works.** A generated `dylib` types crate, `cdylib` clusters and
  `-C prefer-dynamic` give one copy of `std` and of the type crates. The `release` profile
  also passed, with code inlined across crates.
- **One allocator.** The clusters and the types library import `__rust_alloc` from the `std`
  DLL.
- **`TypeId` misses a changed type.** The `TypeId` of `Point` was the same in build 1 and build
  2. The type key found the change.
- **Cargo does not give new names.** The file names of the type crates stayed the same after
  the change. The name of the types library is a hash of its content, so the harness got a new
  name. Spec 8.5 needs this.
- **Every library needs `VCRUNTIME140.dll`** with the MSVC target. A clean Windows does not have
  it. This is input for the toolchain prototype (13.2). The gnu target of that prototype does
  not need it.
- **Export counts are low.** The `std` DLL has 1,401 exports. The types library has 64 in
  `dev` and 18 in `release`. The PE limit is 65,535. A large type crate was not measured.
- **Files of each build.** Each `cdylib` and `dylib` gives a `.dll`, `.dll.lib`, `.dll.exp` and
  `.pdb`. The type and node crates give a `.rlib` and a `.rmeta`. The `std` DLL comes from
  `<sysroot>/bin`.
- **Build times.** A clean build took 0.7 to 0.8 s. The rebuild after the change took 0.5 to
  0.6 s. The project is very small, so these times do not predict real projects.

### Known limits

- The loader supports only Windows. On Linux and macOS, the test does not run.
  [Issue 1](https://github.com/bhowiebkr/Patina/issues/1) tracks this gap.
- The test did not run on a clean Windows computer.
- The only generic type is `Vec<T>`. The type crates have no dependencies, so the build ID
  does not include resolved dependencies yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.

## Hot reload

The code is in [prototypes/hot-reload](prototypes/hot-reload/README.md). It ran on 2026-11-15
with Rust 1.94.0, on Windows 11 Pro, in the `dev` and the `release` profile. It did not run on
Linux or macOS.

CI runs the test for each push to `main`. It passed in both profiles on 2026-11-15, on the
`windows-latest` runner, with Rust 1.100.0, in 12.1 s. On the Linux and macOS runners, clippy
checks the prototype, but the test has no code to run.

### What was checked

The test `cargo test -p hot-reload` ran and passed these checks in both profiles. A long run
did 100 node reloads and 20 types reloads in each profile, and passed the same checks.

- A build with no change added no file to `build/`. The loaded cluster A stayed in use.
- Each node change gave one new file: cluster A. The types library and cluster B kept their
  files.
- After each node reload, the new cluster A ran with the loaded cluster B. The values of the
  old build went to cluster B, and they dropped with the code of their own build.
- A `Probe` of build 0 stayed alive through all reloads. At the end, it dropped with the code
  of build 0, and the counts of the newest build did not change.
- A failed build did not change `build/`, and the loaded build still ran. After the fix, the
  same source gave the same file, so nothing new loaded.
- Each types change gave a new types library and two new cluster files. The engine refused the
  old `points` and `probe` values, and accepted the old `String`.
- The old values of each types reload dropped with the code of their build. The counts of the
  new build did not change.

### Measurements

The project has one node in each cluster, so these numbers do not predict real projects. The
numbers come from the long run. The memory is the growth of the test process.

| Measurement | `dev` | `release` |
| --- | --- | --- |
| Cold build | 0.72 s | 0.93 s |
| Build with no change | 0.03 s | 0.03 s |
| Rebuild after a node change, mean of 100 | 0.33 s (0.26 to 0.54) | 0.30 s (0.27 to 0.37) |
| Rebuild after a types change, mean of 20 | 0.51 s (0.46 to 0.57) | 0.57 s (0.55 to 0.63) |
| `build/` growth for each node reload | 50 KB | 15 KB |
| `build/` growth for each types reload | 181 KB | 111 KB |
| `build/` after 120 reloads | 9.4 MB, 164 files | 4.6 MB, 164 files |
| Private bytes after 120 reloads | +2.6 MB | +2.4 MB |
| Working set after 120 reloads | +9.4 MB | +4.2 MB |
| Cargo target folder at the end | 133 MB, from 12.6 MB | 23.5 MB, from 2.9 MB |

### Findings

- **Old builds stay loaded at a low cost.** 121 copies of cluster A, 21 of cluster B and 21 of
  the types library added less than 10 MB of working set. The memory growth is small next to the
  disk growth.
- **The builds were not reproducible.** Two builds of the same source differed in the PE time
  stamps and the PDB ID. The code was the same. With `-C link-arg=/Brepro`, the two files were
  identical. Spec 8.5 now requires reproducible builds.
- **Content-hash names work.** With `/Brepro`, a build of a restored source gave the name of
  the loaded file. Thus an undo needs no new copy. The cache key must still hash the inputs,
  because the engine needs the key before the compile (8.6).
- **The drop function is in the cluster.** The drop function of `Probe` was in the cluster A
  file, not in the types library, in both profiles. Thus a live value needs its cluster, not
  only the types library.
- **Windows locks a loaded library.** The delete failed with "Access is denied" (os error 5).
  A rename of the loaded library worked.
- **LTO fails with `-C prefer-dynamic`.** With `lto = "fat"` or `"thin"`, rustc refused: "linker
  plugin based LTO is not supported together with `-C prefer-dynamic`". With `-C lto` in
  `RUSTFLAGS`, rustc also refused. Spec 8.3 now has bake mode without LTO, as an agent decision.
- **The cargo target folder grows fast.** Each types reload added about 5.8 MB in `dev` and
  1 MB in `release`, mostly incremental data. The engine needs a cleanup of the target folder.

### Known limits

- The test ran only on Windows, because the loader supports only Windows.
- The cleanup of old builds when a project opens (8.5) is not tested.
- The memory of one row is noisy, because the test and the Windows loader also allocate. Use
  the totals of a long run.
- `/Brepro` is a flag of the MSVC linker. With `rust-lld` on `x86_64-pc-windows-gnu`, the flag
  is `--no-insert-timestamp` (see Toolchain). The hot reload test did not run on that target.
- The cleanup of the cargo target folder is not designed yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.

## Toolchain

The code is in [prototypes/toolchain](prototypes/toolchain/README.md). It ran on 2026-11-15 on
Windows 11 Pro, from an empty folder. It installed Rust 1.100.0 for `x86_64-pc-windows-gnu`
and Zig 0.17.1. It did not run on a clean computer, on Linux or on macOS.

### What was checked

The run passed all 12 steps:

- `curl.exe` downloaded `rustup-init.exe`, the channel manifest and the 4 packages of the
  minimal profile. The SHA-256 hash of each file was correct. Rustup installed from this local
  mirror, with `--no-modify-path`.
- `curl.exe` downloaded Zig, the hash was correct, and `tar.exe` unpacked it.
- The build ran offline, with a clean environment. `PATH` had only the Rust tools and
  System32. The other variables were removed, except 10 system variables.
- Zig compiled the C code of the local crate `cmix` and the bundled SQLite of
  `libsqlite3-sys`: 10 `zig cc` calls and 4 `zig ar` calls.
- `rust-lld` linked `node.dll`. It imports only Windows system DLLs and `msvcrt.dll`. It has
  no Rich header, so the MSVC linker did not make it.
- A new process with the clean environment loaded the library. The C function, the SQLite
  query and 4 Rust threads gave the expected results.
- The folder moved to a name with spaces, an e with an acute accent and a CJK letter. The
  build and the call passed again.
- After the run, the user `PATH` and the Rustup key in the registry had not changed.

Other checks on the same toolchain:

- Two clean `release` builds of `node.dll` gave different files. With
  `-C link-arg=--no-insert-timestamp`, the two files were identical.
- A build of the prototype with `-C target-feature=+crt-static` does not refer to
  `VCRUNTIME140.dll` or to the UCRT DLLs. This static build ran all 12 steps with the files
  that it holds.
- A Zig object with `snprintf` linked with the `msvcrt` of `rust-mingw`, and gave the correct
  text.

### Measurements

| Step | Time |
| --- | --- |
| Install Rust: downloads and rustup | 185.0 s, with a stalled download |
| Install Zig: download and unpack | 24.4 s |
| Download the crates | 1.9 s |
| Cold `dev` build of the node | 5.6 s, and 2.7 s with the Zig cache |
| Build after the move | 4.8 s |

- The downloads are about 265 MB: rustc 104 MB, Zig 100 MB, rust-std 28 MB, `rustup-init`
  15 MB, cargo 12 MB and rust-mingw 5 MB. The crates of the fixture are not included.
- After the run, the folder held 1.4 GB: `rustup` 845 MB, `zig` 416 MB with its cache, `cargo`
  45 MB, and the project with its target folder 82 MB.

### Findings

- **GNU `ld` cannot open a CJK path.** After the move, the GNU `ld` of `rust-mingw` failed:
  "cannot open output file ... Invalid argument". It reads its arguments in the ANSI code
  page, so the CJK letter became "?". The e with an acute accent worked, because code page 1252
  has it. `rust-lld` passed. Spec 13.2 now uses `rust-lld`, as an agent decision.
- **`rust-lld` needs no extra files.** rustc finds it in its own folder and uses the start
  files and import libraries of `rust-mingw`. Cargo also uses it for build scripts, because
  the target is the host.
- **An allowlist firewall blocks new programs.** This computer allows outgoing connections
  only for known programs. The new `rustup-init.exe` and `cargo.exe` failed with os error
  10051. `curl.exe` of System32 is allowed, so the prototype downloads all toolchain files with
  it. Spec 13.2 now says that Patina makes these downloads itself, as an agent decision.
- **A download can stall.** In the clean run, curl stopped at 372 KB of rust-mingw for more
  than 60 s. Then the step passed. Hypothesis: the speed limit stopped the transfer, and the
  retry of curl downloaded the file.
- **The C code links with `msvcrt.dll`, not the UCRT.** Windows has `msvcrt.dll` in System32.
- **Zig adds a sanitizer in `dev` builds.** A Zig debug object had a `__ubsan` symbol. With
  `-fno-sanitize=all`, it had none. The wrappers add this flag.
- **A folder rename can fail one time.** In one run of the static build, the move of the
  folder failed with "Access is denied" (os error 5). No process ran from the folder. The next
  run passed. Hypothesis: a virus scan or the search index held a file. The engine needs a
  retry when it renames a folder.
- **The normal prototype build needs `VCRUNTIME140.dll`.** A clean computer does not have it.
  The static build does not need it, and the Zig wrappers are copies of that file.

### Known limits

- The prototype did not run on a clean computer. This computer has Visual Studio and a Rust
  install. The clean environment hid them, and the build did not use MSVC.
- The prototype is for Windows only.
- C code that needs functions of the UCRT only was not tested.
- rust-analyzer and NVRTC (13.2, items 3 and 4) were not installed.
- The update of an installed toolchain was not tested.
- The fixture is a C ABI call, not a node in a graph.
- These shortcuts are open in the code:
  - `curl.exe` downloads all the Rust files, because the firewall can block a new
    `rustup-init.exe` (`src/install.rs`).
  - `PATINA_FETCH_CARGO` names a `cargo.exe` that the firewall allows, for the crate download
    (`src/main.rs`).

## Viewport

The code is in [prototypes/viewport](prototypes/viewport/README.md). It ran on 2026-11-15 on
Windows 11 Pro, with Rust 1.94.0, Tauri 2.12.5, `wgpu` 30.0.2 and WebView2 157, in the
`release` profile. It did not run on Linux or macOS. The computer is the computer of the
connections prototype.

### What was checked

The bench ran in an off-screen window that did not take the focus. It measured three frame
paths:

- **native**: the `wgpu` surface is on the window, under a transparent WebView2.
- **shared**: a GPU readback into WebView2 shared memory. The page draws it with WebGL2.
- **channel**: the same readback, sent to the page through a Tauri channel.

For each path, the bench dragged the camera of viewport 1 for 2 s in three scenarios: one
viewport, two viewports, and one viewport at 1920 x 1080. Then 11 checks captured the composed
window with `PrintWindow` and counted the scene pixels:

- The **Inspector** panel and an open menu cover the scene. The scene comes back after the
  menu closes.
- A splitter drag and a window resize change the viewport size. On the native path, the resize
  configured the surface again.
- Viewport 1 hides, restores, closes, reopens, floats into its own window and docks again.
- Keys go to a text field or to the focused viewport, never to both.
- The viewport follows a display scale of 200% and 100%, emulated with the zoom of the web view.
- Two viewports show the same GPU data.
- The bench destroyed the device while the surface textures were held. The render thread made
  a new device, and the scene came back.

The final runs passed 11 of 11 checks in each path: on the RTX 4090 and the Intel UHD 770 with
DX12, and on the RTX 4090 with Vulkan (native only). Each probe passed at its first capture.

### Measurements

The latency on the native path is the time from the input event of the page to the present
call. On the copy paths, it is the time to the end of the page draw. Neither includes the
compositor or the display. Work is the render-thread time of a frame. With DX12, it includes
the vsync wait. CPU is the CPU use of the process tree, in cores. Bytes is the mean copy for
each viewport frame.

RTX 4090, DX12:

| Path | Scenario | Viewport FPS | Page draw FPS | Latency p50 | Work p95 | Bytes | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native | one | 60 | - | 33.0 ms | 17.0 ms | 0 | 0.41 | 0.8% |
| native | two | 120 | - | 33.1 ms | 17.0 ms | 0 | 0.18 | 3.0% |
| native | full | 60 | - | 33.2 ms | 17.0 ms | 0 | 0.22 | 2.5% |
| shared | one | 60 | 60 | 49.5 ms | 1.2 ms | 6.3 MB | 0.34 | 23.9% |
| shared | two | 120 | 120 | 49.3 ms | 1.4 ms | 3.9 MB | 0.43 | 29.2% |
| shared | full | 60 | 59.5 | 49.6 ms | 1.3 ms | 8.3 MB | 0.27 | 29.1% |
| channel | one | 60 | 19.8 | 112.4 ms | 1.4 ms | 6.3 MB | 1.11 | 15.8% |
| channel | two | 120 | 39.7 | 109.5 ms | 1.8 ms | 3.9 MB | 1.28 | 19.3% |
| channel | full | 60 | 15.7 | 132.7 ms | 1.6 ms | 8.3 MB | 0.98 | 19.0% |

Intel UHD 770, DX12 (`--low-power-gpu`):

| Path | Scenario | Viewport FPS | Page draw FPS | Latency p50 | Work p95 | Bytes | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native | one | 60 | - | 33.0 ms | 16.9 ms | 0 | 0.35 | 20.9% |
| native | two | 120 | - | 32.8 ms | 17.1 ms | 0 | 0.18 | 32.5% |
| native | full | 60 | - | 33.0 ms | 17.0 ms | 0 | 0.13 | 31.7% |
| shared | one | 60 | 60 | 49.5 ms | 1.0 ms | 6.3 MB | 0.32 | 31.9% |
| shared | two | 120 | 120 | 49.3 ms | 1.3 ms | 3.9 MB | 0.32 | 44.1% |
| shared | full | 60 | 60 | 49.4 ms | 1.1 ms | 8.3 MB | 0.29 | 43.2% |
| channel | one | 60 | 20 | 99.2 ms | 1.4 ms | 6.3 MB | 0.93 | 16.3% |
| channel | two | 120 | 39.7 | 148.8 ms | 1.9 ms | 3.9 MB | 1.50 | 24.2% |
| channel | full | 60 | 15.2 | 132.7 ms | 1.9 ms | 8.3 MB | 1.04 | 24.2% |

Vulkan, native path:

| GPU | Scenario | Viewport FPS | Latency p50 | Work p95 | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- |
| RTX 4090 | one | 60 | 33.1 ms | 1.0 ms | 0.35 | 0.5% |
| RTX 4090 | two | 119 | 31.9 ms | 1.4 ms | 0.15 | 2.0% |
| RTX 4090 | full | 59.6 | 17.0 ms | 1.4 ms | 0.17 | 2.5% |
| Intel UHD 770 | one | 60 | 25.5 ms | 0.8 ms | 0.31 | 24.1% |
| Intel UHD 770 | two | 120 | 25.3 ms | 0.9 ms | 0.27 | 28.1% |
| Intel UHD 770 | full | 60 | 25.5 ms | 0.9 ms | 0.28 | 24.5% |

- The Intel UHD 770 rows with Vulkan are from a run before the DX12 default. That run passed
  10 of 11 checks.
- A device loss took 152 to 337 ms from the loss to the first new frame, on both GPUs and both
  backends.
- The upload of a copied frame into a WebGL texture took 0.6 to 0.8 ms at p95.
- An earlier Vulkan run on the RTX 4090, before the last fixes, gave a shared-path page draw of
  30 to 39 FPS with one viewport, and 27.5 ms native latency.

### Findings

- **The native path is the best path.** It held 60 FPS for each viewport on both GPUs, with no
  copied bytes and 0.13 to 0.41 CPU cores. Spec 15.8 now has the native path, as an agent
  decision.
- **Vulkan on the Intel UHD 770 did not show reliably under the web view.** In the last 4
  native runs with Vulkan, the viewport areas of many captures stayed black for 1 s or more.
  1 to 7 checks failed in 3 of the runs. The page areas of the same captures were correct. With
  DX12, 4 runs on the Intel UHD 770 passed 11 of 11, each probe at its first capture. Spec 15.8
  now uses DX12 on Windows, as an agent decision.
- **The copy paths add latency.** The shared path had 49 to 50 ms at p50 on both GPUs, against
  17 to 33 ms on the native path. It copied 378 to 498 MB/s. On DX12, its page drew 60 FPS.
- **The channel is too slow.** The page drew 15 to 40 FPS, with 99 to 149 ms of latency and
  about one CPU core. Each frame also allocates.
- **The latency to the present call does not compare between the backends.** With DX12, it was
  33 to 34 ms in each run, and the frame work included the vsync wait. With Vulkan, it was 17
  to 33 ms, and a timer paced the frames, so the image can wait after the present call.
  `NOT_ENOUGH_INFO` for the display latency: the bench does not measure the compositor.
- **`wgpu` 30.0.2 keeps a held surface texture after a device loss.** It discards the texture
  only on a valid device. Then the surface drop panicked on Vulkan, and a configure failed on
  DX12. The prototype releases the texture with a drop in a caught unwind, the method of
  gfx-rs/wgpu issue 8243.
- **DX12 needs new surfaces after a device loss.** An old surface did not configure on the new
  device. The render thread now makes a new surface for each window.
- **A surface texture must drop before its surface.** Else `wgpu` panicked on DX12 when a
  window closed during the vsync wait.

### Known limits

- The bench ran only on Windows 11, with one monitor at 60 Hz. It did not run on Linux or
  macOS. The shared path, the capture and the sampler use Windows APIs.
- The bench window is off-screen, and the checks read `PrintWindow` captures, not the screen.
- The zoom of the web view emulated the display scale. Two monitors with different scales were
  not checked.
- The owner check: run `cargo run --release -p viewport -- --mode native`. Drag, resize, float
  and dock viewport 1, and open the menu. Change the Windows display scale, and look at the
  viewport edges.
- A real device loss between the loss check and the present keeps the texture held. Then the
  Vulkan surface drop panics (`src/render.rs`).
- These shortcuts are open in the code:
  - The texture release after a device loss uses a caught unwind (`src/render.rs`).
  - The bench code is a copy of the connections bench (`src/bench.rs`).
  - The CPU and GPU sampler and the probes work only on Windows (`src/bench.rs`,
    `src/main.rs`).
  - The page and the reports use paths in the source tree, so the prototype runs only from a
    checkout (`src/main.rs`, `src/bench.rs`).
  - The scene file has no WESL features, so `wgpu` reads it as WGSL (`src/gpu.rs`).
  - The channel path allocates for each frame (`src/render.rs`).
  - A resize of the shared path makes a new buffer (`src/shared.rs`).

## Connections

The code is in [prototypes/connections](prototypes/connections/README.md). It ran on 2026-11-15
on Windows 11 Pro, with Rust 1.94.0, Tauri 2.12.5, WebView2 157, React 19.3.1 and React Flow
12.12.1. It did not run on Linux or macOS. The computer has one 1920 x 1080 monitor at 60 Hz, a
display scale of 125%, an RTX 4090 and an Intel UHD 770.

### What was checked

The bench ran these checks in an off-screen window that did not take the focus:

- The graph has 500 nodes and 1,000 connections. 200 connections are buses of 8 to 32 lanes.
  The graph has 4,376 lanes.
- A Rust thread sent the lane states at 60 FPS as packed bits in a Tauri channel (15.6). The page
  received all frames, with no gaps.
- The view panned, zoomed, selected a bus each 6 frames and dragged a route point, while the
  states changed.
- The SVG renderer and the WebGL renderer ran all scenarios on the RTX 4090 and on the Intel
  UHD 770.
- Other runs stopped the feed, sent JSON events, and emulated a DPR of 1 and 2.
- Screenshots of the two renderers showed the same wires, lanes, badges, selection and route
  handles. At a far zoom, a bus showed as one line.

### Measurements

The tables come from runs of 10 s with a warmup of 1 s. The defaults are now 3 s and 0.5 s, and
a default run gave the same result in 46 s. Work is the main-thread time of a frame. CPU is the
CPU use of the process tree, in cores. The rows use a DPR of 1.25 and the live feed, unless the
row gives a different value.

RTX 4090:

| Scenario | SVG FPS | SVG dropped | SVG work p95 | SVG CPU | WebGL FPS | WebGL dropped | WebGL work p95 | WebGL CPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| idle-far | 60 | 0 | 7.6 ms | 1.31 | 60 | 0 | 0.9 ms | 0.19 |
| idle-near | 60 | 0 | 3.1 ms | 1.31 | 60 | 0 | 0.6 ms | 0.12 |
| pan | 42.63 | 174 | 8.9 ms | 1.51 | 60 | 0 | 3.4 ms | 0.53 |
| zoom | 53.21 | 68 | 15.5 ms | 1.51 | 57.31 | 27 | 18.3 ms | 0.92 |
| select | 60 | 0 | 5 ms | 1.32 | 60 | 0 | 0.8 ms | 0.22 |
| drag | 60 | 0 | 5.8 ms | 1.55 | 60 | 0 | 1.4 ms | 0.24 |

Intel UHD 770 (`--low-power-gpu`):

| Scenario | SVG FPS | SVG dropped | SVG work p95 | SVG CPU | WebGL FPS | WebGL dropped | WebGL work p95 | WebGL CPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| idle-far | 60 | 0 | 8.3 ms | 1.43 | 59.9 | 1 | 1.7 ms | 0.39 |
| idle-near | 23.82 | 363 | 4.9 ms | 1.25 | 60 | 0 | 1.2 ms | 0.26 |
| pan | 11.49 | 490 | 22.3 ms | 1.08 | 60 | 0 | 6 ms | 0.94 |
| zoom | 19.2 | 408 | 32.9 ms | 1.34 | 53.89 | 61 | 24.2 ms | 1.23 |
| select | 17.87 | 422 | 8.6 ms | 1.24 | 60 | 0 | 1.1 ms | 0.22 |
| drag | 18.1 | 419 | 10.1 ms | 1.36 | 60 | 0 | 1.4 ms | 0.25 |

Other runs on the RTX 4090. The zoom of the web view emulates the DPR, so the window keeps the
same physical size:

| Run | Scenario | FPS | Dropped | Work p95 | CPU | GPU 3D |
| --- | --- | --- | --- | --- | --- | --- |
| SVG, feed off | idle-near | 60 | 0 | 0.1 ms | 0 | 0% |
| SVG, feed off | pan | 42.5 | 175 | 6.4 ms | 1.35 | 19% |
| WebGL, feed off | idle-near | 60 | 0 | 0.1 ms | 0.03 | 0% |
| WebGL, feed off | pan | 60 | 0 | 3.3 ms | 0.42 | 12% |
| WebGL, JSON events | idle-near | 60 | 0 | 0.7 ms | 0.16 | 6% |
| SVG, DPR 1 | idle-near | 60 | 0 | 4.1 ms | 1.44 | 21% |
| SVG, DPR 1 | pan | 60 | 0 | 10 ms | 1.48 | 26% |
| SVG, DPR 2 | idle-near | 60 | 0 | 2 ms | 1.02 | 22% |
| SVG, DPR 2 | pan | 49.32 | 107 | 5.8 ms | 1.44 | 18% |
| WebGL, DPR 2 | idle-near | 60 | 0 | 0.6 ms | 0.11 | 4% |
| WebGL, DPR 2 | pan | 60 | 0 | 3.5 ms | 0.4 | 11% |

- A frame of 4,376 lanes has 559 bytes. On the RTX 4090, the Rust send took 23 to 35 us at p50
  and 104 to 164 us at p99.
- On the RTX 4090, the latency from the Rust send to the page was 0.23 to 0.26 ms at p50 with
  WebGL. It was 0.5 to 6.3 ms with SVG.
- On the RTX 4090, the GPU 3D use was 3 to 18% with WebGL and 17 to 25% with SVG. On the Intel
  UHD 770, it was 29 to 58% with each renderer.

### Findings

- **The WebGL layer holds 60 FPS.** It held 60 FPS on both GPUs in all scenarios, except the
  zoom. It used 2 to 11 times less CPU than SVG. Spec 15.1 now has the WebGL layer and React,
  as agent decisions.
- **SVG drops frames when the view moves.** On the RTX 4090, the pan ran at 43 FPS with and
  without the feed. Thus the SVG edges cause the cost, not the feed. On the Intel UHD 770, SVG
  ran at 11 to 24 FPS in each scenario with a near view.
- **The DPR changes the SVG cost.** The SVG pan held 60 FPS at a DPR of 1. It dropped frames at
  1.25 and 2, with the same physical window size. `NOT_ENOUGH_INFO` for the cause: the bench
  does not split the paint and the raster time.
- **The nodes drop frames at a far zoom.** In the zoom, WebGL also dropped frames: 27 on the
  RTX 4090 and 61 on the Intel UHD 770. At a 20% zoom, React Flow shows all 500 nodes, and the
  page has 2,331 elements. The graph view needs a simpler node form at a far zoom (15.1).
- **Packed bits are small and fast.** The JSON events also held 60 FPS at this size. In the
  Tauri source, a channel sends a payload under 1 KB by `eval`, not by a fetch. Each send
  allocates one time, because the channel takes the frame by value.
- **A navigation from a Tauri command runs the command twice.** The navigation stops the IPC
  request, and the IPC script of Tauri then sends the command again. Thus the page must
  navigate itself after the command returns.
- **An off-screen window can get the focus.** Two full runs each had one focus event, with
  `focused(false)`. With `focusable(false)`, tao sets `WS_EX_NOACTIVATE`, and two runs had no
  focus event. Hypothesis: Windows activated the next window when another window closed.
- **SVG covers the badges.** In the SVG renderer, a later edge covers the badges of the edges
  before it. The WebGL renderer draws all badges on an overlay above the wires.

### Known limits

- The bench ran only on Windows 11, with one monitor at 60 Hz. It did not run on Linux or macOS.
- The zoom of the web view emulated the display scale. A live change of the Windows display
  scale is an owner check. Open the graph view, change the scale in Windows Settings, and look
  at the wires, the badges and the wire selection.
- The bench moves the view through the React Flow API. The mouse path of the pan, the zoom, the
  selection and the route drag was not measured.
- The nodes do not move, so a node drag was not measured. A bus socket does not show a contact
  for each lane.
- React Flow hides an SVG edge outside the box of its two nodes. A route point outside that box
  was not checked.
- Svelte Flow was not measured.
- These shortcuts are open in the code:
  - The CPU and GPU sampler and the screenshots work only on Windows (`src/bench.rs`,
    `src/main.rs`).
  - The page and the reports use paths in the source tree, so the prototype runs only from a
    checkout (`src/main.rs`, `src/bench.rs`).
  - Each send allocates. A send with no allocation needs shared memory (`src/feed.rs`, 15.6).
