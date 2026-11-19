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

The sample graph makes a district of 600 m x 450 m. The `report` node writes
`559 items, 53244 places, 664546 triangles, bounds -308 -233 -0.1 to 308 233 54.2, scene hash ...`.
The first build compiles the C++ kernel of the model workbench in about 5 to 10 s.

## The steps of a city

Each step is a node, so you can change or replace one step:

1. `streets` makes a grid of streets from the seed. Each `avenue`th line is a wide, straight
   avenue. `jitter` moves the crossings, and `drop` removes a part of the inner streets.
2. `blocks` gives the faces between the streets. `inset` moves the edges in for a sidewalk.
3. `lots` splits each block into lots. A lot with no street edge becomes a park.
4. `heights` gives each lot a number of floors. The buildings are tall at the center.
5. `masses` makes the body and the roof of each building: flat, gable or hip.
6. `facades` puts a window on each floor and bay, and a door on the street. The window and the
   door are solid inputs, for example `cuboid` nodes.
7. `roads` makes the asphalt and the sidewalks. `scatter` puts a solid along the edges of the
   plots, for example trees on the parks and lamps on the sidewalks.
8. `merge` puts two scenes into one. `report` writes the scene as text.

## Scenes and instances

The city wires carry a `Scene`, not one solid. A scene has items, and each item is one mesh at
many places. All the windows are one mesh at many thousands of places, so the build makes no
boolean for them. Each building is a group, so the export can write each building alone.

The `model-view` view shows a scene with GPU instances. Do these steps:

1. Open the project in the editor and click **Run**.
2. Open **Views** and select **model-view**.
3. Click `report` to see the full city, or click a step, for example `masses`.

## Export the buildings

The `export-buildings` node writes each building to its own file `building-<group>.glb` in a
folder. The center of the base of each building is at the origin. Do these steps:

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

- The sample district runs in about 0.3 s after the build.
- A district of 1 km x 1 km has 105482 places and 1352998 triangles. It runs in less than 0.5 s
  after the build. The target of task E8 is 2 s.

## Known limits

- The streets are a grid with jitter. The workbench has no curved streets and no terrain.
- A block is one face of the street graph, so a very concave block can give thin lots.
- Round shapes and turned roofs use the sine of the C++ runtime, so the scene hash can be
  different on another platform. This is not checked.
- The export writes no materials. The colors are on the scene items only.
