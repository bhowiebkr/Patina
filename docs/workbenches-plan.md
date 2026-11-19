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
- **A2.** Add **Split center** to the **Views** menu, and **Move to the other pane** to the menu of
  a view tab. A tab drag to the edge of the center shows a drop zone for the second pane.
- **A3.** Add a splitter between the panes. It works as the splitters of the areas: a drag, the
  arrow keys for 16 px, and Shift with the arrow keys for 64 px.
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
- **D4. Run model.** A model builds only when an input changes, not at each frame. Use the batch
  domain or the event domain (spec 9). A long build must not stop the frames. Measure it.
- **D5. 3D view.** The `model-view` view of the workbench: orbit, pan and zoom, its own depth
  texture, a grid, a light, part colors and a wireframe. It shows the mesh of the selected node.
  It sets `dock = "center"`, so it opens next to the graph (task A). When it needs a generic GPU
  function that `patina-gpu` does not have, add only the generic part to the tool.
- **D6. Export.** A node writes `.glb`, `.obj` and `.stl` with a small writer in the workbench.
  The sockets go into the glTF extras. The node writes only after a button or a trigger.
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

## Order

1. A and B1 first. They are small and need no other task.
2. D0 next. It can run at the same time as A.
3. B2 to B5 after the synth agent commits its work in `examples/`.
4. D1 to D8.
5. E and F. Each needs D. They do not need each other.
6. C when the owner decides.

## Open decisions for the owner

- The name of the model workbench.
- The kernel, after D0. A C++ kernel needs CMake, or a prebuilt library (spec 13.4).
- City and gun as two workbenches that use the model workbench as a library (recommended), or
  as graphs of one workbench.
- When the workbenches move out of the Patina repository.
