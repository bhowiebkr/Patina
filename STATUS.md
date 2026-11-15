# Status

The current milestone is milestone 0, the risk prototypes (spec section 19).

| Prototype | Result | Platforms that ran |
| --- | --- | --- |
| Rust types across clusters (5.1, 5.2) | Passed | Windows 11 and CI `windows-latest`, `x86_64-pc-windows-msvc` |
| Hot reload (8.5) | Passed | Windows 11, `x86_64-pc-windows-msvc` |
| Toolchain (13.2) | Not started | - |
| Viewport (15.8) | Not started | - |
| Connections (15.9) | Not started | - |

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
  it. This is input for the toolchain prototype (13.2).
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
- `/Brepro` is a flag of the MSVC linker. The toolchain prototype must find the flag for its
  target, for example `x86_64-pc-windows-gnu`.
- The cleanup of the cargo target folder is not designed yet.
- The harness is not the engine. It shows the method, and the engine will use the same method.
