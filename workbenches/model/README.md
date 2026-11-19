# Model

The workbench for the procedural generation of 3D models. Nodes make solids, and a seed changes
the output. The city and the gun workbenches use it as a library. The plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task D.

This workbench is at task D3. It has the types and the base nodes. The 3D view and the export
come in tasks D4 to D8.

## Run the workbench

Run this command from the root of the repository:

```
cargo run -p patina-cli -- run workbenches/model --frames 1
```

The sample graph makes a small tower: an octagon with a random height, four merlons on top and a
door. The `report` node writes
`144 triangles, bounds -1.2 -1.2 0 to 1.2 1.2 2.933, mesh hash 8049817992ebb0d7`.
The first build compiles the C++ kernel in about 5 to 10 s. With the Patina toolchain, the first
C++ build on a computer also makes the C++ runtime. This adds 10 to 20 s one time.

## The parts

- `types/geometry` holds the types of the model wires:
  - `Solid` is a closed solid of the [Manifold](https://github.com/elalish/manifold) kernel.
    It has the primitives, the booleans and the moves. Z is up.
  - `Mesh` has the triangles of a solid: positions, normals, UV and a part index.
  - `Sockets` has named frames, for example `muzzle` and `grip`.
  - `Sketch` is a flat shape on the XY plane, with holes. Extrude and revolve make solids from it.
  - `Seed` gives random values. `seed.child("barrel")` gives a sub-seed for one part, so a
    change to one part does not change the other parts. The same seed gives the same mesh at
    each run.
- The **Inspector** shows a solid, a sketch or a mesh as a picture, with the triangle count and
  the bounds.
- The nodes:
  - Solids: `cuboid`, `cylinder`, `cone`, `sphere` and `torus`.
  - Sketches: `rectangle`, `circle` and `polygon`. A polygon is a text of points, for example
    `0 0, 2 0, 1 1.5`.
  - Sketch to solid: `extrude` (with twist and taper) and `revolve` (with a gap in degrees).
  - Moves: `translate`, `rotate`, `scale`, `mirror`, `linear-array` and `polar-array`.
  - Booleans: `union`, `cut`, `intersect` and `hull`.
  - Sockets: `attach` moves a solid to a socket. A sockets input is a text, for example
    `muzzle 0 0 2; grip 1 2 3 0 0 90`.
  - Random: `seed` makes a sub-seed, `random` gives a number in a range and `pick` chooses one
    of two solids by weights.
  - `report` writes the solid as text.
- `types/geometry/vendor` has the C++ sources of Manifold and Clipper2, with their licenses.
  The build needs no download of C++ sources and no CMake. See
  [the vendor notes](types/geometry/vendor/README.md).

## Known limits

- A model builds again at each frame. Task D4 makes it build only after a change.
- The C++ kernel stops the program when it has no memory. It does not give an error. A shape
  has 1024 segments at most, and an array has 1000 copies at most.
- The kernel has no loft, no fillet and no bevel.
- The same graph gives the same mesh on MSVC and on the Patina toolchain for boxes, polygons,
  moves and turns by multiples of 90 degrees. Round shapes and other turns use the sine of the
  C++ runtime, so the last bit can be different on another platform. This is not checked.
- A number input with no wire starts at 0. Thus `taper` 0 keeps the top size, `gap` 0 makes a
  full turn and `segments` 0 lets the kernel choose.
