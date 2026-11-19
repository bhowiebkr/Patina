# Model

The workbench for the procedural generation of 3D models. Nodes make solids, and a seed changes
the output. The city and the gun workbenches use it as a library. The plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task D.

This workbench is at task D2. It has the types and two nodes. The base nodes, the 3D view and
the export come in tasks D3 to D8.

## Run the workbench

Run this command from the root of the repository:

```
cargo run -p patina-cli -- run workbenches/model --frames 1
```

The `report` node writes `12 triangles, bounds -1 -2 -0.5 to 1 2 0.5, mesh hash 1463ee2a258be625`.
The first build compiles the C++ kernel in about 5 to 10 s. With the Patina toolchain, the first
C++ build on a computer also makes the C++ runtime. This adds 10 to 20 s one time.

## The parts

- `types/geometry` holds the types of the model wires:
  - `Solid` is a closed solid of the [Manifold](https://github.com/elalish/manifold) kernel.
    It has the primitives, the booleans and the moves. Z is up.
  - `Mesh` has the triangles of a solid: positions, normals, UV and a part index.
  - `Sockets` has named frames, for example `muzzle` and `grip`.
  - `Seed` gives random values. `seed.child("barrel")` gives a sub-seed for one part, so a
    change to one part does not change the other parts. The same seed gives the same mesh on
    each platform.
- The **Inspector** shows a solid or a mesh as a picture, with the triangle count and the bounds.
- `nodes/cuboid` makes a box. `nodes/report` writes the solid as text.
- `types/geometry/vendor` has the C++ sources of Manifold and Clipper2, with their licenses.
  The build needs no download of C++ sources and no CMake. See
  [the vendor notes](types/geometry/vendor/README.md).

## Known limits

- A model builds again at each frame. Task D4 makes it build only after a change.
- The C++ kernel stops the program when it has no memory. It does not give an error.
