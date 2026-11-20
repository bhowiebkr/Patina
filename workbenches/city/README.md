# City

The workbench for seeded cities: streets, blocks, lots and buildings. It uses the model
workbench as a library. A seed changes the full city. The plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task E.

## Run the workbench

Run this command from the root of the repository:

```
cargo run -p patina-cli -- run workbenches/city --frames 1
```

The `main` graph makes an organic city of 600 m x 450 m on hills, with a river and a coast.
The `report` node writes
`574 items, 42651 places, 1329526 triangles, bounds -300 -225 -6.5 to 300 225 169.422, scene hash ...`.
The first build compiles the C++ kernel of the model workbench in about 5 to 10 s.

The `planned` graph puts the same buildings on a grid of streets on flat land. Run it with
`--graph planned`.

## The steps of a city

Each step is a node, so you can change or replace one step. The `main` graph has these steps:

1. `terrain` makes the hills from the seed, with a slope down to the coast. Then drops of rain
   run down the slopes: they cut the valleys and sharpen the ridges. Where the water collects,
   it cuts streams down to the sea. **erosion** sets the drops for each sample, from 0 to 8.
   `river` cuts a river along the routes of the water, from the edge opposite the lowest edge
   to the sea. Thus the streams join it. It also cuts a valley: **valley** sets the width of
   each side in meters.
   `ground` shows the grass, the sand, the rock and the water.
2. `density` puts the people on the land: a main center and sub-centers, less on steep slopes
   and none on the water.
3. `highways` joins gates on the edge to the center. A steep slope costs more, and the water
   costs a bridge. `grow` grows the arterial roads and the local streets from the highways.
   A street follows the ground, and it turns along a bank or a steep slope. `prune` removes
   the dead ends.
4. `blocks` gives the faces between the streets. `inset` moves the edges in for a sidewalk.
   `lots` splits each block into lots. Each lot keeps a street edge, so a lot in a large block
   is deeper. A thin part of a cut joins a lot across the cut.
5. `zoning` gives each lot a zone: residential, commercial, industrial or park. A thin lot
   becomes a park. `heights` gives each lot a number of floors from the density and the zone.
6. `buildings` plans a building on each lot: a house, an apartment, a shop, a tower or a
   warehouse. The plan has the footprint, the wings, the step-backs, the floors, the roof form
   and a style.
7. `walls` builds the bodies, the floor bands, the cornices and the parapets. `facades` puts
   the windows, the doors, the shop fronts, the loading doors and the balconies on the walls.
   `roofs` builds the gable, hip, flat and sawtooth roofs and the parts on them.
8. `roads` makes the asphalt, the sidewalks, the center lines and the bridges. `parks` gives
   the park lots, and `scatter` puts a solid along the edges of the plots on the ground, for
   example trees on the parks and lamps on the sidewalks.
9. `merge` puts two scenes into one. `report` writes the scene as text.

In the `planned` graph, `streets` makes the grid from the seed instead of steps 1 to 3. Each
`avenue`th line is a wide, straight avenue. `jitter` moves the crossings, and `drop` removes a
part of the inner streets.

## Scenes and instances

The city wires carry a `Scene`, not one solid. A scene has items, and each item is one mesh at
many places. All the windows are one mesh at many thousands of places, so the build makes no
boolean for them. Each building is a group, so the export can write each building alone.

The `model-view` view shows a scene with GPU instances. Do these steps:

1. Open the project in the editor and click **Run**.
2. Open **Views** and select **model-view**.
3. Click `report` to see the full city, or click a step, for example `buildings`.

## Export the buildings

The `export-buildings` node writes each building to its own file `building-<group>.glb` in a
folder. The center of the base of each building is at the origin. Each item of the building,
for example the walls or the windows, is one primitive. Each color of the items is one glTF
material with no metal. Do these steps:

1. Click **Folder** in the node and choose a folder. A relative path starts at the project
   folder.
2. Click **Export**.

The node writes only after a click on **Export**, or at each fire of a trigger with an order
wire to the node. A run after a change does not write.

## The parts

- `types/geometry` is an import of the model types, with `Scene`. `types/city` has the
  street graph, the plots and the steps above, with unit tests.
- `nodes/cuboid`, `nodes/cylinder`, `nodes/sphere`, `nodes/translate`, `nodes/union` and
  `nodes/seed` are imports from the model workbench. The other nodes are thin nodes on
  `types/city`.
- `views/model-view` is an import of the model view.

## Measurements

On 2026-11-19, on the Windows computer of the owner, with a bake build:

- The `main` graph runs in about 0.7 s in a CLI run after the build.
- A city of 1 km x 1 km has 95518 places and 2956283 triangles. It runs in 0.23 s after the
  build in the CLI test. The target of task E8 is 2 s.
- The erosion takes about 19 ms for `main` and 81 ms for 1 km x 1 km in a release build.

## Known limits

- A thin part of a cut stays a park when the lot that it joins is also thin, for example with
  a long arm of 2 m.
- A street at the edge of the land turns along the edge. The roads stop at the edge.
