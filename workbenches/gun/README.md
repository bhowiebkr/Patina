# Gun

The workbench for seeded guns for a game. A gun has parts from four original makers and a
rarity. A seed changes the full gun. The workbench uses the model workbench as a library. The
plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task F.

## Run the workbench

Run this command from the root of the repository:

```
cargo run -p patina-cli -- run workbenches/gun --frames 1
```

The two `stats` nodes write the maker, the rarity, the stats and the parts of two guns, for
example `Vesper common`, `damage 35` and `parts: receiver vesper, barrel vesper, ...`. The two
`report` nodes write the counts and the hash of each `look` scene. The first build compiles the
C++ kernel of the model workbench in about 5 to 10 s.

## The two ways to make a gun

- The `gun` node makes a full gun from one seed. A **random** maker or rarity comes from the
  seed.
- The lower row of the `main` graph makes a gun from parts. Each part node has a seed and a
  maker. The maker **none** removes the part. `assemble` puts each part on its socket of the
  receiver and applies the rarity.

The same seed and maker as a part of the `gun` node give the same part, so you can start from a
`gun` node and then change one part.

## The parts

- `receiver`: the body of the gun, with a socket for each other part. It sets the base stats.
- `barrel`: the barrel, with the `muzzle` socket at its end. A long barrel gives accuracy.
- `grip`: the grip and the trigger guard, with the `hand` socket.
- `stock`: the stock. It gives accuracy and a slower reload.
- `magazine`: the magazine. A large magazine reloads slower.
- `sight`: the sight. It gives accuracy.
- `accessory`: the accessory under the barrel. Each maker changes other stats.

## The makers

- **Blockwerk**: blocky boxes and cut corners, high damage and a low fire rate. Olive body.
- **Vesper**: sleek round shapes and tapers, high accuracy. Off-white body.
- **Scrapjaw**: plates at random offsets, pipes and bolts, a high fire rate and large
  magazines. Rust body.
- **Halo**: rings and coils, a fast reload. Teal body.

The `gun` node uses the maker of the gun for most parts. Some parts come from another maker.

## The rarity

The rarities are **common**, **uncommon**, **rare**, **epic** and **legendary**. A random
rarity has these chances: 50, 25, 14, 8 and 3 percent. A higher rarity does these things:

- It gives more damage, a higher fire rate, more accuracy, a larger magazine and a faster
  reload.
- It adds a stock, a sight and an accessory more frequently.
- It changes the accent color: gray, green, blue, purple or orange.

## See the guns

Do these steps:

1. Open the project in the editor and click **Run**.
2. Open **Views** and select **model-view**. Click a `look` node to see its gun in 3D. The view
   shows an output only when it has a wire, so each `look` node has a `report` node.
3. Click **Toon** in the view for three light bands and dark outlines.
4. Open **Views** and select **gun-wall**. The view shows the guns of eight seeds, with the
   maker and the rarity of the `gun` node.
5. Click a gun on the wall to give its seed to the `gun` node. Click **Previous** or **Next**
   for other seeds.

## Export a gun

The `export-gun` node writes the gun to `<name>.glb` with the `muzzle` and `grip` sockets in the
glTF extras. It also writes the stats to `<name>.txt`. Do these steps:

1. Click **Folder** in the node and choose a folder. A relative path starts at the project
   folder.
2. Click **Export**.

The node writes only after a click on **Export**, or at each fire of a trigger with an order
wire to the node. A run after a change does not write.

## The crates

- `types/geometry` is an import of the model types. `types/guns` has the parts, the makers, the
  rarity, the stats and the side view, with unit tests.
- `nodes/seed` is an import from the model workbench, and `nodes/report` is an import from the
  city workbench. The other nodes are thin nodes on `types/guns`.
- `views/model-view` is an import of the model view. `views/gun-wall` is the gun wall.

## Known limits

- The parts are boxes, cylinders and simple booleans. There are no curved or sculpted shapes.
- The gun wall draws flat side views with paths, not 3D images.
- The toon outline can have small gaps at sharp corners.
