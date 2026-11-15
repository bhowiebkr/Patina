# Patina Specification: Layers and Compilation

Part of the [Patina Specification](README.md).

## 7. Layers

Layers give fast execution and fast iteration at the same time. The graph runs as one optimized build. A change to one node builds fast. The target is a few seconds.

### 7.1 Model

Patina compiles a graph in two layers:

| Layer | Contents | Build mode (8.3) | Purpose |
|---|---|---|---|
| Base | The whole graph, except the node in the slot. Small nodes inline into each other | Bake | Fast execution |
| Slot | The node that the user edits, compiled alone | Edit | Fast iteration |

- The base is the normal state. With no node in the slot, the whole graph is one optimized build.
- Patina never runs the graph as separately compiled nodes. Only the node in the slot runs as its own library.
- The base calls the slot through a function pointer. This costs one indirect call each time the node runs, and only while the node is in the slot.
- If the graph uses the node folder more than once (6.2), each use calls the slot. For a generic node, the slot holds a copy for each set of types that the graph uses (4.5).
- A change to a shared type crate (5.2) rebuilds the base. The slot does not help there.

### 7.2 Edit cycle

1. The user opens the code of node N. The engine starts a background build of the base with an empty slot for N. Only the clusters that hold a use of N rebuild (8.2). The cache keeps the other clusters (8.6).
2. Until that build is ready, the old base keeps running with the old code of N.
3. At each code change, the engine compiles N alone in edit mode, with a target of a few seconds. It swaps the new build into the slot between two runs of the graph.
4. If N does not compile, the slot keeps the last build that compiled, and the graph keeps running. The node shows the error (15.5), and its outputs keep their normal look. If N never compiled, it has no build: it shows the error, and its downstream nodes are Skipped (15.3).
5. The engine fuses N back into the base when one of these occurs:
   - The user opens the code of another node.
   - The user makes no change to N for a set time (see 18).
   - The user clicks **Bake**.
6. To fuse, the engine rebuilds the cluster of N in bake mode, with the new code of N inline. It swaps in the new cluster between two runs. If the user opened node M, the same build makes the slot for M.

Rules:

- A swap keeps the state of each stateful node whose code did not change. Only the edited node resets (4.2).
- The slot takes a change only when the pins of N stay the same. If a change adds, removes, or changes a pin, the engine rebuilds each cluster that holds a use of N, as for a wiring change.
- Each build has the revision of the code that it built. The engine never swaps in a build older than the active one, and it shows only the errors of the newest revision.
- When a swap touches more than one cluster, the engine loads and checks all the new builds before the first switch. If one fails, all the active builds stay. Each clock domain switches at its next run boundary.
- A change to a widget folder rebuilds each cluster and each view that uses it, as a wiring change does.
- A wiring change, for example a new node or a new connection, rebuilds the affected cluster in edit mode. That cluster fuses back after the same set time with no change, or when the user clicks **Bake**.

### 7.3 Visual display

- The Layers panel (15.2) shows the base and the slot. Each row shows its build state and its build time, for example "Built in 1.8 s".
- The node in the slot shows a slot icon. The slot uses no new color (15.3).
- The status bar shows the layer state, for example "Fused" or "Slot: Exposure".

### 7.4 Undo and redo

- The UI sends each user action to the engine as one edit:

```rust
enum Edit {
    AddNode(NodeId, NodeDef),
    RemoveNode(NodeId),
    Connect(Port, Port),
    Disconnect(Port, Port),
    SetCode(PathBuf, String), // a file in a node, view, widget, or connection folder (6.2)
    SetParam(NodeId, String, Value),
    Order(NodeId, NodeId),   // order wire, see 9.2
}
```

- Undo removes the last edit. Redo applies it again.
- Undo and redo of a code change go through the slot, the same as other code changes.

## 8. Compilation

### 8.1 Toolchain and dependencies

- Each project locks one Rust version.
- Each project is one Cargo workspace with one `Cargo.lock`. The lock file gives one resolved set of crate versions for all nodes.
- Cargo can still put two incompatible versions of one crate in the build, for example `image` 0.24 and 0.25. Their types are different types. The type key check rejects a wire between them (5.1).
- Each node, view, widget, connection, and type folder is one crate of the workspace (6.2).
- Nodes can use any crate from crates.io, as source.
- One shared crate cache serves all projects.

### 8.2 Units

| Unit | Rust form | Output |
|---|---|---|
| Node | One library crate, from its node folder (6.2) | `.rlib` |
| Widget | One library crate, from its widget folder (6.2) | `.rlib`, linked into the nodes and views that use it |
| Cluster | Generated crate that depends on its node crates | One `.dll` / `.so` / `.dylib` |
| Shared types | `std` + type crates of the project | One shared Rust library |
| Slot | One node, compiled alone (7) | One `.dll` / `.so` / `.dylib` |
| View | One view, compiled alone (15.8) | One `.dll` / `.so` / `.dylib` |
| Connection kind | One connection kind, compiled alone (6.3) | One `.dll` / `.so` / `.dylib` |

- A node crate is a unit of the build cache. A node never runs as its own library, except in the slot (7).
- A cluster is a part of the graph that compiles as one unit.
- Put cluster boundaries at large nodes. Keep small nodes inside one cluster, so the compiler can inline and fuse them.
- A cluster never crosses a clock domain boundary.

### 8.3 Build modes

| Mode | Optimization | Use |
|---|---|---|
| Edit | Low, no LTO | The slot (7): fast compile of the node that the user edits |
| Bake | Full, with LTO | The base (7) and packages: fast execution |

### 8.4 Base and slot

Section 7 describes the base and the slot. The base builds in bake mode. The slot builds in edit mode.

### 8.5 Hot reload

- Each build gets a new file name: clusters, the slot, views, connection kinds, and the shared types library. Windows locks loaded `.dll` files. Windows also gives an already loaded library to each later request with the same file name. Thus a new build with an old name would run the old code.
- The engine loads each library by its full path. It finds the libraries that a library needs only in the `build/` folder of the project. On Windows, `LoadLibraryExW` with the `LOAD_LIBRARY_SEARCH` flags limits the search path. The `build/` folder holds each library that a cluster needs, including the `std` library of the toolchain.
- Values, node states, futures, and callbacks keep a link to the build that made them. Old builds stay loaded, so these drop with their own code.
- A failed build never deletes or replaces a loaded file.
- When a project opens, Patina deletes the old builds in `build/`, except the loaded builds and the builds in the cache.
- The engine does not unload old builds. Rust libraries do not unload safely, and macOS often does not unload them. Memory and disk use grow with each reload until a restart. Milestone 0 measures the growth (19).
- With process isolation (see 16), the engine can restart the node process and free all old builds.

### 8.6 Cache

- The engine stores each compiled cluster with a hash of its contents as the key.

| Change | Compile needed? |
|---|---|
| Change a parameter | No |
| Undo to a graph that is in the cache | No |
| Change node code | Only that node, in the slot (7). Later, a fuse of its cluster |
| Change wiring | Only the affected cluster |
| Change shader source in a shader node (see 10.3) | No Rust compile |
