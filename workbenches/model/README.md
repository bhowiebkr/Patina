# Model

The workbench for the procedural generation of 3D models. Nodes make solids, and a seed changes
the output. The city and the gun workbenches use it as a library. The plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task D.

The [illustrated guide](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches/model.md) shows each part of this workbench in the editor.

This workbench has the types, the base nodes, the run model, the 3D view, the export, the guide
graphs and the checks (tasks D1 to D8).

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

## The guide graphs

Open a graph in the **Project** panel. Each graph has a note that tells what it shows.

| Graph | What it shows |
|---|---|
| **main** | A seeded tower and the `export` node. Change **seed** to get a new tower. |
| **solids** | Primitives, `cut`, `translate` and `union`. |
| **sketches** | `rectangle`, `circle` and `polygon`, with `extrude` (twist and taper), `revolve` (gap) and `loft`. |
| **seeds** | A house from a seed: `seed`, `random` and `pick`. |
| **sockets** | `attach` at named sockets, and `export` with the sockets in the `.glb` file. |

To run a guide graph without the editor, use `--graph`, for example
`cargo run -p patina-cli -- run workbenches/model --graph seeds --frames 1`.

## The run model

A model builds at **Run** and after each change, not at each frame. The model nodes are in the
batch domain (spec 9.1): a node with no wires in, and each node after it, runs in that domain.
A wire from a node in the frame domain puts the model in the frame domain.

## The 3D view

The `model-view` view shows the solid of the selected node. It opens in the second center pane,
next to the graph. Do these steps:

1. Open the project in the editor and click **Run**.
2. Open **Views** and select **model-view**.
3. Click a model node, for example `union`.

The view shows the output of the node, also when the output has no wire. For a node with no
solid output, for example `report`, it shows the solid that goes into the node. When the selection
has no solid, the view keeps the last solid.

- The mouse works as in Maya. Drag with the left button to orbit, with the middle button to
  pan, and with the right button to zoom. A drag to the right or down zooms in. Alt and a button
  does the same. Shift and a left drag also pans. Turn the wheel to zoom. Press Home or click
  **Frame model** to frame the model.
- **Wireframe** shows the triangle edges, **Grid** shows the ground grid and the X and Y axes,
  and **Part colors** gives each part its own color. **Toon** gives three light bands and dark
  outlines.
- Each new value of the node, for example after a seed change, shows at once. The camera stays.

## Export a model

The `export` node writes its solid to a `.glb`, `.obj` or `.stl` file. The file extension
selects the format. A relative path starts at the project folder. Do these steps:

1. Connect a solid to the `solid` input of an `export` node.
2. Click **File** in the node and choose a file name.
3. Click **Export**.

The node writes only after a click on **Export**. A run after a change does not write. To write
at each fire of a trigger, connect an order wire from a `trigger` node to the `export` node.
`patina-cli run workbenches/model --events 1` fires each trigger one time.

- The `.glb` file has one primitive for each part. The `sockets` input goes into the glTF
  extras of the model node, as `extras.sockets`.
- The `.glb` and the `.obj` files have Y up, as glTF and most game tools expect. The `.stl`
  file keeps Z up.
- The node writes a temporary file first and then replaces the old file. A failed write keeps
  the old file.

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
  - `Sketch` is a flat shape on the XY plane, with holes. Extrude, revolve and loft make solids
    from it.
  - `Seed` gives random values. `seed.child("barrel")` gives a sub-seed for one part, so a
    change to one part does not change the other parts. The same seed gives the same mesh at
    each run.
- The **Inspector** shows a solid, a sketch or a mesh as a picture, with the triangle count and
  the bounds.
- The nodes:
  - Solids: `cuboid`, `cylinder`, `cone`, `sphere` and `torus`.
  - Sketches: `rectangle`, `circle` and `polygon`. A polygon is a text of points, for example
    `0 0, 2 0, 1 1.5`.
  - Sketch to solid: `extrude` (with twist, taper and a 45 degree **chamfer** of the top
    edges), `revolve` (with a gap in degrees) and
    `loft` (from the outlines of a bottom sketch to the outlines of a top sketch). Each outer
    outline joins the outer outline with the nearest center and size. Two joined outer outlines
    must have the same number of holes.
  - Moves: `translate`, `rotate`, `scale`, `mirror`, `linear-array` and `polar-array`.
  - Booleans: `union`, `cut`, `intersect` and `hull`.
  - Sockets: `attach` moves a solid to a socket. A sockets input is a text, for example
    `muzzle 0 0 2; grip 1 2 3 0 0 90`.
  - Random: `seed` makes a sub-seed, `random` gives a number in a range and `pick` chooses one
    of two solids by weights.
  - `report` writes the solid as text.
  - `export` writes the solid to a `.glb`, `.obj` or `.stl` file.
- `types/geometry/vendor` has the C++ sources of Manifold and Clipper2, with their licenses.
  The build needs no download of C++ sources and no CMake. See
  [the vendor notes](types/geometry/vendor/README.md).

## Known limits

- In the editor, a build stops the frames, the node GUIs and the views until it ends. A block
  of 500 houses takes about 0.13 s.
- When the C++ kernel has no memory, the node gets the error "the result is too large". A
  sketch with a very large coordinate gets the error "the construction is not valid". A shape
  has 1024 segments at most, and an array has 1000 copies at most.
- The kernel has no fillet. A Minkowski opening was too slow, and `SmoothOut` makes the faces
  bulge (see STATUS.md). The **chamfer** of `extrude` cuts only the top edges, with sharp
  corners. A chamfer that removes an edge of the sketch gives an error.
- A loft of very different outlines can give sides that cross, and a crossing at any height gives
  an error. A loft joins each outline to the outline with the nearest center and size, so very
  different holes or islands can also give sides that cross. A loft cannot join one outline to
  two outlines.
- The same graph gives the same mesh on MSVC and on the Patina toolchain for boxes, polygons,
  moves and turns by multiples of 90 degrees. Round shapes and other turns use the sine of the
  C++ runtime, so the last bit can be different on another platform. This is not checked.
- The `.obj` and the `.stl` files have no sockets. A solid has no colors, so the `export` node
  writes no materials. The `export-buildings` node of the city and the `export-gun` node of the
  gun write one material for each color.
- A number input with no wire, no text and no `#[param]` default starts at 0. Thus `taper` 0
  keeps the top size, `gap` 0 makes a full turn and `segments` 0 lets the kernel choose. The
  sizes, the scales and the counts have a default, for example 1 for the sides of `cuboid`.
