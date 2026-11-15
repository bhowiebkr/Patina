# Prototype: Rust types across clusters

This is the first milestone 0 prototype (spec section 19). It tests spec 5.1 and 5.2: Rust
values go between two compiled clusters through one shared types library. The result is in
[STATUS.md](../../STATUS.md).

## What it does

The test builds the user project in `project/` two times, for the `dev` and the `release`
profile:

1. The harness copies `project/` to a work folder in `target/tmp/`.
2. The harness writes the generated files of `patina-types`: `Cargo.toml`, `abi.rs` and
   `keys.rs`. The name of the library is a hash of its content.
3. Cargo builds the two clusters with `-C prefer-dynamic`.
4. The harness copies the libraries and `std` into one `build/` folder. Each cluster gets a new
   file name for each build (spec 8.5).
5. The harness loads the clusters with `LoadLibraryExW`, compares the type keys and passes
   erased values from cluster A to cluster B.
6. The test changes the two fields of `Point` from `u32` to `f32`, and builds again. The engine
   refuses the old values.

## Files

| Path | Content |
| --- | --- |
| `src/abi.rs` | The C ABI between the engine and a cluster. The types library copies this file. |
| `src/lib.rs` | The harness: generate, build, install, load, and the type key checks. |
| `tests/types_across_clusters.rs` | The scenario and its checks. |
| `project/types/` | The type crates: `geometry`, and two versions of `units`. |
| `project/nodes/` | The `source` and `sink` nodes. |
| `project/clusters/` | The glue of the two clusters, as Patina will generate it. |
| `project/patina-types/` | The shared types library. The harness writes its other files. |

## Run it

Run the test from the repository root, on Windows:

```bash
cargo test -p types-across-clusters -- --nocapture --test-threads=1
```

The output shows the files that cargo made, the imports of each library, the type keys and the
build times. On Linux and macOS, the test file is empty, because the loader supports only
Windows.
