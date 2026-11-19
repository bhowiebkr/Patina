# Model

The workbench for the procedural generation of 3D models. Nodes make solids, and a seed changes
the output. The city and the gun workbenches use it as a library. The plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task D.

This workbench is at task D5. It has the types, the base nodes, the run model and the 3D view.
The export, the guide graphs and the checks come in tasks D6 to D8.

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

A model builds at **Run** and after each change, not at each frame. The model nodes are in the
batch domain (spec 9.1): a node with no wires in, and each node after it, runs in that domain.
A wire from a node in the frame domain puts the model in the frame domain.

## The 3D view

The `model-view` view shows the solid of the selected node. It opens in the second center pane,
next to the graph. Do these steps:

1. Open the project in the editor and click **Run**.
2. Open **Views** and select **model-view**.
3. Click a model node, for example `union`.

The view shows the output of the node when the output has a wire. For a node with no wired
output, for example `report`, it shows the solid that goes into the node. When the selection
has no solid, the view keeps the last solid.

- Drag to orbit. Shift and drag to pan. Turn the wheel to zoom. Press Home or click
  **Frame model** to frame the model.
- **Wireframe** shows the triangle edges, **Grid** shows the ground grid and the X and Y axes,
  and **Part colors** gives each part its own color.
- Each new value of the node, for example after a seed change, shows at once. The camera stays.

## Measure the builds

To measure the build times, run this command from `workbenches/model`:

```
cargo test --release -p geometry measure -- --ignored --nocapture
```

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

- In the editor, a build stops the frames, the node GUIs and the views until it ends. A block
  of 500 houses takes about 0.13 s.
- The C++ kernel stops the program when it has no memory. It does not give an error. A shape
  has 1024 segments at most, and an array has 1000 copies at most.
- The kernel has no loft, no fillet and no bevel.
- The same graph gives the same mesh on MSVC and on the Patina toolchain for boxes, polygons,
  moves and turns by multiples of 90 degrees. Round shapes and other turns use the sine of the
  C++ runtime, so the last bit can be different on another platform. This is not checked.
- A number input with no wire starts at 0. Thus `taper` 0 keeps the top size, `gap` 0 makes a
  full turn and `segments` 0 lets the kernel choose.
