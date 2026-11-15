# Prototype: hot reload

This is the second milestone 0 prototype (spec section 19). It tests spec 8.5: new builds load
next to old builds, and values from old builds stay alive across a reload. It measures the time,
the memory and the disk use of each reload. The result is in [STATUS.md](../../STATUS.md).

## What it does

The test uses the user project and the loader of the prototype
[Rust types across clusters](../types-across-clusters/README.md). It runs the steps below in the
`dev` profile, then in the `release` profile:

1. **Cold build.** The harness copies the libraries into `build/`. Each cluster gets the name
   `<target>-<hash>.dll`, with a hash of the content of the file.
2. **No change.** A second build gives the same files, so nothing new loads.
3. **Node reloads.** The test changes the `points` function of the `source` node, builds, and
   loads the new cluster A. Values from the old build go to cluster B, and they drop with the
   code of their own build.
4. **Failed build.** The build fails, `build/` does not change, and the loaded build still
   runs. Then the test restores the source. The next build gives the same file as before.
5. **Types reloads.** The test changes a comment in the `geometry` type crate. The types
   library and the two clusters get new files. The engine refuses the old values of the changed
   types, and accepts the old `String`.
6. **Locks.** The test tries to delete and to rename a loaded library.

At the end, the test builds the project in `release` with `lto = "fat"`. It reports the
result, and does not fail when the build fails.

The test links each library with `/Brepro` on the MSVC target. Without this flag, the linker
writes a time stamp and a random PDB ID into each library, so the same source gives a new file.

## Files

| Path | Content |
| --- | --- |
| `src/lib.rs` | The install into `build/`, the set of loaded clusters, and the memory and folder sizes. |
| `tests/hot_reload.rs` | The scenario, its checks, the report and the LTO check. |

## Run it

Run the test from the repository root, on Windows:

```bash
cargo test -p hot-reload -- --nocapture
```

The output shows a table for each profile. Each row is one build: the build time, the new files
in `build/`, the size of `build/` and of the cargo target folder, the memory of the process, and
the number of loaded clusters. A summary for each step follows the table.

The default run is short, because `cargo test --workspace` runs it. It builds only the `dev`
profile, with 2 node reloads and 1 types reload. These environment variables change the run:

- `PATINA_FULL=1`: the measured run of the prototype. It adds the `release` profile and the LTO
  check.
- `PATINA_NODE_RELOADS`: the node reloads. The default of the measured run is 10 in `dev` and 4
  in `release`.
- `PATINA_TYPE_RELOADS`: the types reloads. The default of the measured run is 4 in `dev` and 2
  in `release`.

On Linux and macOS, the test file is empty, because the loader supports only Windows.
