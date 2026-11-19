# Workbench plan

This plan comes from the owner requests of 2026-11-19. It has the tasks for:

- The split of the center area of the editor.
- The new name "workbenches" for the example projects.
- A workbench for procedural 3D models.
- A city workbench and a gun workbench that use the 3D blocks.

## Rules for workbenches

These rules are owner decisions of 2026-11-19.

- A workbench is a Patina project for one domain. It holds the node folders, types, views,
  widgets, connections and graphs of that domain.
- Patina ships the included workbenches. A user makes their own workbench as a normal project.
- Domain logic goes into a workbench. The tool (engine, editor, CLI and SDK) gets only generic
  features that each domain can use.
- Use this test for a new feature: does a person who builds a digital logic computer need it?
  If the answer is no, the feature goes into a workbench.
- Later, the workbenches can move out of the Patina repository, to keep the tool small. Thus a
  workbench uses only the SDK (`build/sdk`) and its own folders. It never uses a path into
  `crates/`.

## Where each part goes

| Part | Place | Reason |
|---|---|---|
| Split of the center area | Tool, editor dock | Each domain can show a view next to the graph. |
| Name "workbench" for the examples | Tool, folders and docs | It is a name. |
| 3D view with a camera and depth | Model workbench, `views/` | Only 3D work needs it. |
| Geometry kernel, mesh type, seeds, export | Model workbench, `types/` and `nodes/` | Only 3D work needs it. |
| City nodes and graphs | City workbench | Only the city needs them. |
| Gun nodes and graphs | Gun workbench | Only the gun needs them. |
| Share of blocks between workbenches | Libraries (spec 6.2), which exist | Each domain can use it. |

## A. Split the center area (tool)

- **A1.** Write the owner decision into spec 15.2. The center area can split into two panes. The
  **Graph** tab stays in the first pane. The second pane takes view tabs. The panes are side by
  side, or one above the other.
- **A2.** Add **Split center** and **Stack the panes** to the **Views** menu, and **Move to the
  split pane** to the menu of a view tab. A tab drag to the edge of the center shows a drop zone
  for the second pane.
- **A3.** Add a splitter between the panes. It works as the splitters of the areas: a drag, the
  arrow keys for 16 px, and Shift with the arrow keys for 64 px. Each pane keeps at least 120 px,
  so either pane can be the small one (owner request of 2026-11-19).
- **A4.** The layout file keeps the split, the direction and the size. An old layout file opens
  with no split. **Reset layout** removes the split. The default layout still fits 1280 x 720.
- **A5.** A window has one viewport surface (spec 15.8). Check that a viewport in the second pane
  follows the splitter, and that the graph pane covers no part of it.
- **A6.** Checks: a unit test for the layout file, a selfcheck step (split, move a view, drag
  the splitter, keys, **Reset layout**) and a shot at 1280 x 720.
- Done when: the user splits the center, moves the **scene** view of the gpu workbench to the
  second pane, sees the graph and the view together, and the split stays after a restart.

## B. Examples become workbenches (tool)

- **B1.** Define the word in the spec. Spec 6.1 calls a top graph a "workbench". Change that text
  to "top graph", so that the word has one meaning.
- **B2.** Move `examples/` to `workbenches/`. 29 tracked files outside `examples/` name the
  folder: the package tool, the engine, the CLI tests, the editor tests, CI, the docs and the
  root `Cargo.toml`.
- **B3.** The package puts the included workbenches in `workbenches/`. Keep the update rule of
  spec 11: replace a workbench that the person did not change, and move a changed one to
  `<name>-old`.
- **B4.** The start screen lists the included workbenches, each with **Open**.
- **B5.** Add a check to `tools/repo-checks`. It fails when a workbench file names a path outside
  its own folder, except `build/sdk`. This prepares the move out of the tool.
- The synth agent edits `examples/synth` and `examples/gpu` now. Do B2 only after its work is
  committed. Write a note in `artifacts/agent-notes/to-synth-agent.md` before the move.
- Later, not in this plan: move the workbenches to their own repositories. The package or
  **Browse libraries** then gets them. The owner decides when.

## C. Audit of domain code in the tool

- **C1.** Find domain code in the tool, and keep or move each item. The audio clock domain
  (spec 9) is generic. These items can be domain code:
  - `Ui::beat_timeline` in the SDK, a music timeline (spec 4.3).
  - The WAV render command of `patina-cli` (spec 3.2).
  - The synth tests in `crates/patina-cli/tests/cli/synth.rs`.
- The synth agent owns these files now, so the owner decides the order.

## D. Model workbench (procedural 3D models)

The working name is the model workbench, in `workbenches/model`. The owner can give it a name.

- **D0. Prototype the geometry kernel.** Report the result before the other D tasks, as for
  milestone 0. CadQuery is Python on the OCCT kernel. Patina code is Rust, and a build needs no
  global tool (spec 13.4). Candidates:
  - Manifold, through the `manifold-csg` crate: fast and robust booleans of triangle meshes.
    Its C++ build needs CMake and a C++ compiler.
  - monstertruck, a fork of the truck kernel: pure Rust B-rep with booleans and fillets. Its
    maturity is not known.
  - OCCT, through Rust bindings such as `cadrum` or `opencascade-sys`: the CadQuery kernel,
    with fillets and STEP. Its C++ build is long and needs CMake.
- Measure on Windows, in a type crate of a workbench, with only the Patina toolchain:
  - the first build time and the rebuild time,
  - the time for a gun of 30 parts and for a block of 500 buildings,
  - closed and valid meshes, and the license.
- A C++ kernel needs a prebuilt library in the workbench, or an owner change of spec 13.4.
- Recommendation before the prototype: game assets need triangle meshes, not exact CAD. Thus a
  mesh kernel such as Manifold fits better than B-rep. The nodes look like CadQuery: workplane,
  sketch, extrude and cut. The kernel stays behind the type crate, so that it can change.
- Result of 2026-11-19, on Windows with the MSVC target: Manifold cut the gun in 2.7 ms and made
  a block of 500 in 81 ms. monstertruck failed the gun union. csgrs does not resolve. The
  details are in [prototypes/geometry](../prototypes/geometry/README.md).
- Agent decision of 2026-11-19, the owner can change it: use Manifold behind the type crate.
- **D0b. Build Manifold with no CMake.** Compile the Manifold and Clipper2 sources with the `cc`
  crate in the type crate. Prove it with the Patina toolchain (Zig and the gnu target).
- Result of 2026-11-19: the gnu build with Zig and `rust-lld` took 8 s cold, and a gun took
  3.1 ms. The toolchain needed a C++ setup by hand. The details are in
  [prototypes/geometry](../prototypes/geometry/README.md).
- **D0c. C++ crates in the toolchain (tool).** This is generic: each C++ crate needs it, not
  only the model workbench. Agent decision of 2026-11-19, the owner can change it.
  - Add a `zig-cxx` wrapper, and set `CXX` and `CXXSTDLIB=c++` for the target in the clean
    environment.
  - On Windows, make the C++ runtime one time: link a small C++ program with Zig, then put its
    libc++, libc++abi and the msvcrt shim of D0b into `libc++.a` in the Zig folder. Give rustc
    that folder as a link search path.
  - Check: a `patina-cli` test builds a crate with a C++ file and a C++ exception, and runs it.
- Result of 2026-11-19, on Windows with a toolchain run: a node crate compiled C++ with `cc`.
  The cluster threw and caught a C++ exception, and the graph ran. The first build took 12.7 s
  with the runtime, and a rebuild took 0.05 s. The CLI tests use the system toolchain, so the
  check is the toolchain run and unit tests. A fix came with it: build scripts did not start
  with the own toolchain, because they need libgcc_s. Details are in [STATUS.md](../STATUS.md).
- **D1 to D8** vendor the Manifold and Clipper2 sources into the type crate, with their license
  files. A build then needs no download of C++ sources.
- **D1. Types.** `Solid`, `Mesh` (positions, normals, UV and a part ID), `Sockets` (named frames,
  for example `muzzle` and `grip`) and `Seed`. The viewer gives a thumbnail and a text with the
  triangle count and the bounds.
- **D2. Seeds.** An integer random generator in the type crate, with no new dependency.
  `seed.child("barrel")` gives a sub-seed, so a change to one part does not change the other
  parts. The same seed and graph give the same mesh on each platform. A test compares the hash.
- **D3. Base nodes.**
  - Primitives: box, cylinder, sphere, cone and torus.
  - Sketch: rectangle, circle and polygon. Then extrude, revolve, and loft if the kernel has it.
  - Transform, mirror, linear array and polar array.
  - Union, cut, intersect, hull, and bevel or fillet if the kernel has it.
  - Attach at a socket.
  - A random number from a seed, and a pick of one of N inputs by seed and weights.
- Agent decisions of 2026-11-19 for D3, the owner can change them:
  - Manifold has no loft, no fillet and no bevel, so D3 has none of them.
  - `pick` has two solid inputs and a weights text, because a node has a fixed set of inputs.
    Chain picks for more choices.
  - Seeds and sockets have a text form, so an input with no wire can hold them. `attach` reads
    a sockets text, for example `muzzle 0 0 2; grip 1 2 3 0 0 90`.
  - The plan of spec 4.8 for `#[param]` defaults is not in the code. An input with no wire
    starts at 0, so the nodes use inputs where 0 is useful: `taper`, `gap` and `segments`.
  - A shape has 1024 segments at most, and an array has 1000 copies at most. A larger value
    from a wire could use all the memory, and the kernel stops the program then.
  - Each boolean runs at once. The kernel collapses a lazy boolean tree by reference counts at
    its evaluation, so a later evaluation could give other triangles.
  - The mesh sorts its triangles by their corners. The kernel orders them by IDs from a global
    counter, and parallel nodes take these IDs in any order. Before the sort, the hash of the
    sample tower changed between runs.
- **D4. Run model.** A model builds only when an input changes, not at each frame. Use the batch
  domain or the event domain (spec 9). A long build must not stop the frames. Measure it.
- Agent decisions of 2026-11-19 for D4, the owner can change them:
  - The model nodes use the batch domain. They declare no domain: a node with no wires in, and
    each node after it, is in the batch domain (spec 9.1). A user can still feed a frame value
    into a model for an animation.
  - The event domain does not fit. A wire out of it is a graph error, a view cannot read its
    outputs while it runs, and in the editor no trigger fires at a change.
  - In the editor, a batch run stops the frames, the node GUIs and the views until it ends.
    `measure_builds` gave 0.6 ms for the tower and 122 ms for 500 houses. Thus the model
    workbench keeps the batch domain as it is.
  - The cluster libraries are not `Send`, so the batch domain cannot move to another thread
    without new `unsafe` code. Task D4b does the generic part.
- **D4b. Views during a batch run (tool).** The views and the node GUIs go on during a long
  batch run, with the values of the last run. Do it before E8, or when a build of the model
  workbench takes more than 0.2 s.
- **D5. 3D view.** The `model-view` view of the workbench: orbit, pan and zoom, its own depth
  texture, a grid, a light, part colors and a wireframe. It shows the mesh of the selected node.
  It sets `dock = "split"`, so it opens next to the graph (task A). When it needs a generic GPU
  function that `patina-gpu` does not have, add only the generic part to the tool.
- Agent decisions of 2026-11-19 for D5, the owner can change them:
  - The view draws into its own texture with 4x MSAA and a depth texture. Then it copies the
    texture to the viewport. Thus `patina-gpu` needs no change.
  - The view reads the wired solid output of the selected node. For a node with no wired
    output, it reads the solid input. When the selection has no solid, it keeps the last solid.
  - Each `Solid` has a serial number, so the view uploads a mesh only when the value changes.
  - The first read of a batch output by a view starts a batch run (spec 15.8 rule 3). This is
    a generic tool change: a view that opens after **Run** sees a value at once.
  - The view panel sends only the newest viewport place with each ask. This is a generic tool
    fix: a slow view drew at an old place.
- **D6. Export.** A node writes `.glb`, `.obj` and `.stl` with a small writer in the workbench.
  The sockets go into the glTF extras. The node writes only after a button or a trigger.
- Agent decisions of 2026-11-19 for D6, the owner can change them:
  - The node writes after a click on **Export**, or when `patina_sdk::fired()` is true. An
    order wire from a `trigger` node puts the node in the event domain (spec 9.4).
  - `patina_sdk::fired()` is a generic tool addition of about 15 lines. It is true while a node
    runs for a fire of a trigger.
  - The `.glb` and the `.obj` files have Y up. The `.stl` file keeps Z up.
  - The `.glb` file has one primitive for each part, with no indices. The sockets go into the
    `extras.sockets` of the model node, with Y up.
  - The node replaces the file through a temporary file. The `.obj` and the `.stl` files have
    no sockets.
- **D7. Graphs and guide.** A `main` graph with a seeded sample model, learning graphs and a
  README.
- **D8. Checks.** The seed test, a mesh test, a headless CLI run, a selfcheck step (open the
  view in the second pane, change the seed, see a new mesh) and a shot.

## E. City workbench

The city workbench uses the model workbench as a library (spec 6.2). First, check that a pick
from a library also brings the type crates that the node needs. If it does not, fix the generic
library import in the tool.

- **E1.** Street network: a grid with noise, or a tensor field, from a seed and the bounds.
- **E2.** Blocks: the closed shapes between the streets.
- **E3.** Lots: a recursive split of each block, with a minimum width and area.
- **E4.** Buildings: the footprint, a height from a density map, floors, facade parts (windows
  and doors) and roof types. Split and repeat nodes, as in shape grammars.
- **E5.** Roads and props: the road mesh, sidewalks, and trees and lamps by scatter. Repeated
  parts use instances, not booleans.
- **E6.** The view draws a large area with instances.
- **E7.** Export: one glTF node for each building, so that a game can use them one by one.
- **E8.** Target: a district of 1 km x 1 km builds in less than 2 s in a bake build.

## F. Gun workbench

The gun workbench also uses the model workbench as a library. Use original part styles and
maker names. Do not copy the names, logos or art of Borderlands or of another game.

- **F1.** Parts: receiver, barrel, grip, stock, magazine, sight and accessory, each with sockets.
- **F2.** Maker styles: 3 or 4 original styles. A style is a set of shape rules and parameters,
  for example blocky, sleek or scrap.
- **F3.** Generation: a weighted pick for each part, with a sub-seed. The rarity changes the
  weights and adds parts.
- **F4.** Stats: damage, fire rate, accuracy and magazine size come from the parts. For example,
  a long barrel gives more accuracy. A node gives the stats as text next to the mesh.
- **F5.** Look: toon shading and outlines in the view, and colors for each maker and rarity.
- **F6.** Gun wall: a view that shows a grid of guns from a range of seeds. A click opens the
  seed of a gun.
- **F7.** Export: a `.glb` with the `muzzle` and `grip` sockets, and a stats file for each gun.

## Progress

- 2026-11-19: A and B1 to B5 are done. [STATUS.md](../STATUS.md) has the checks.
- 2026-11-19: D0 is done on Windows with the MSVC target. The Patina toolchain build is task
  D0b.
- 2026-11-19: D0b is done on Windows with a toolchain run and a C++ setup by hand. Next is D0c.
- 2026-11-19: D0c is done on Windows with a toolchain run. Next is D1.
- 2026-11-19: D1 and D2 are done on Windows. The MSVC build and the Patina toolchain gave the
  same mesh hash for a seeded model. Next is D3.
- 2026-11-19: D3 is done on Windows. The sample tower gave one mesh hash in 14 runs: 4 with
  MSVC and 10 with a toolchain run. Round shapes and turns that are not multiples of 90 degrees
  are not compared across platforms. Next is D4.
- 2026-11-19: D4 is done on Windows. The tower builds one time while 30 frames run. The views
  wait during a batch run, so D4b is open. Next is D5.
- 2026-11-19: D5 is done on Windows. The hidden shot check opens the view next to the graph,
  changes the seed and sees a new mesh. Next is D6.
- 2026-11-19: D6 is done on Windows. The CLI test runs the export with a trigger and reads the
  `.glb` file. A run with no fire writes no file. Next is D7.
- 2026-11-19: D7 and D8 are done on Windows. The `main` graph has a note and an `export` node.
  The guide graphs are **solids**, **sketches**, **seeds** and **sockets**. The CLI test runs
  each guide graph. The seed and mesh tests are the unit tests of D2 and D3. The hidden model
  shot is the selfcheck step of D8. Next is E.

## Order

1. A and B1 first. They are small and need no other task.
2. D0 next. It can run at the same time as A.
3. B2 to B5 after the synth agent commits its work in `examples/`.
4. D1 to D8.
5. E and F. Each needs D. They do not need each other.
6. C when the owner decides.

## Open decisions for the owner

- The name of the model workbench.
- The kernel. The agent decision after D0 is Manifold, built from source with Zig (task D0b).
- C++ support in the toolchain (task D0c), or a C++ setup in each workbench that needs it.
- City and gun as two workbenches that use the model workbench as a library (recommended), or
  as graphs of one workbench.
- When the workbenches move out of the Patina repository.
