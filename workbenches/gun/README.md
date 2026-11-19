# Gun

The workbench for seeded guns for a game. A gun has a class, parts from four original makers, a
rarity and an element. A seed changes the full gun. The workbench uses the model workbench as a library. The
plan is in
[the workbenches plan](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches-plan.md),
task F.

## Run the workbench

Run this command from the root of the repository:

```
cargo run -p patina-cli -- run workbenches/gun --frames 1
```

The two `stats` nodes write the name, the maker, the rarity, the class, the element, the stats
and the parts of two guns, for example `Brutal Gale`, `Vesper common shotgun`, `damage 95` and
`parts: receiver vesper, barrel vesper, ...`. The two
`report` nodes write the counts and the hash of each `look` scene. The first build compiles the
C++ kernel of the model workbench in about 5 to 10 s.

## The two ways to make a gun

- The `gun` node makes a full gun from one seed. A **random** class, maker, rarity or element
  comes from the seed.
- The lower row of the `main` graph makes a gun from parts. Each part node has a seed, a class
  and a maker. The maker **none** removes the part. `assemble` puts each part on its socket of
  the receiver and applies the rarity and the element.

The same seed, class and maker as a part of the `gun` node give the same part, so you can start
from a `gun` node and then change one part.

## The classes

The class sets the size of each part, the base stats and the chance of each extra part. A random
class has these chances: pistol 24, SMG 20, rifle 22, shotgun 14, sniper rifle 12 and launcher
8 percent.

- **pistol**: a short body and barrel, a medium damage and a small magazine.
- **smg**: a compact body, a high fire rate and a large magazine.
- **rifle**: a long body, a stock and a medium fire rate.
- **shotgun**: a wide barrel or two barrels, high damage and a slow fire rate.
- **sniper**: a long barrel and a long scope, high damage and high accuracy. It always has a
  sight.
- **launcher**: a wide tube and a back blast pipe, very high damage and a magazine of two.

## The parts

- `receiver`: the body of the gun, with a socket for each other part. It sets the base stats.
- `barrel`: the barrel, with the `muzzle` socket at its end. A long barrel gives accuracy.
- `grip`: the grip and the trigger guard, with the `hand` socket.
- `stock`: the stock. It gives accuracy and a slower reload.
- `magazine`: the magazine. A large magazine reloads slower.
- `sight`: the sight. It gives accuracy.
- `accessory`: the accessory under the barrel. Each maker changes other stats.

## The makers

Each maker has its own shape language, stat bias, three paint schemes, a metal color and a
glow color.

- **Blockwerk**: heavy slabs with chamfers, vents, bolts and stripes. High damage and a
  low fire rate. Olive, tan or gray paint.
- **Vesper**: sleek turned shapes, tapers, fins and glow strips. High accuracy. White, black or
  red paint.
- **Scrapjaw**: plates at random offsets, pipes, bolts, tape and wire. A high fire rate and
  large magazines. Rust, teal or red paint.
- **Halo**: rings, coils, capsules and glow cells. A fast reload. Teal, purple or orange
  paint.

The `gun` node uses the maker of the gun for most parts. Some parts come from another maker.

Each part has six materials, each with its own color: paint, trim, metal, grip, rarity and
glow. The rarity accents use the color of the rarity. The glow parts use the color of the
element, or the glow color of the maker when the gun has no element.

## The rarity

The rarities are **common**, **uncommon**, **rare**, **epic** and **legendary**. A random
rarity has these chances: 50, 25, 14, 8 and 3 percent. A higher rarity does these things:

- It gives more damage, a higher fire rate, more accuracy, a larger magazine and a faster
  reload.
- It adds a stock, a sight and an accessory more frequently.
- It changes the accent color: gray, green, blue, purple or orange.
- It adds an element more frequently.

## The elements

The elements are **none**, **fire**, **shock**, **corrosive** and **cryo**. A gun with an
element does 15 percent less direct damage. Its glow parts have the color of the element:
orange, blue, green or pale blue.

## The names

Each maker has a model name for each class, for example **Kestrel** for a Vesper rifle. The
first word of the name comes from the element, for example **Searing** for fire. A gun with no
element gets the word of its largest part bonus: **Brutal** (damage), **Rapid** (fire rate),
**Steady** (accuracy), **Hoarding** (magazine) or **Nimble** (reload). A gun with no large
bonus has only the model name.

## See the guns

Do these steps:

1. Open the project in the editor and click **Run**.
2. Open **Views** and select **model-view**. Click a `look` node to see its gun in 3D. The view
   shows an output only when it has a wire, so each `look` node has a `report` node.
3. Click **Toon** in the view for three light bands, dark outlines and ink lines on the sharp
   edges.
4. Open **Views** and select **gun-wall**. The view shows the guns of eight seeds, with the
   class, the maker, the rarity and the element of the `gun` node.
5. Click a gun on the wall to give its seed to the `gun` node. Click **Previous** or **Next**
   for other seeds.

## Export a gun

The `export-gun` node writes the gun to `<name>.glb` with the `muzzle` and `grip` sockets in the
glTF extras. It also writes the stats to `<name>.txt`. The `.glb` file has the look of the 3D
view: one primitive for each material of each part, and one glTF material for each color. Do
these steps:

1. Click **Folder** in the node and choose a folder. A relative path starts at the project
   folder.
2. Click **Export**.

The node writes only after a click on **Export**, or at each fire of a trigger with an order
wire to the node. A run after a change does not write.

## The crates

- `types/geometry` is an import of the model types. `types/guns` has the parts, the classes,
  the makers, the materials, the elements, the rarity, the stats, the names and the side view,
  with unit tests.
- `nodes/seed` is an import from the model workbench, and `nodes/report` is an import from the
  city workbench. The other nodes are thin nodes on `types/guns`.
- `views/model-view` is an import of the model view. `views/gun-wall` is the gun wall.

## Known limits

- The parts are made from extrusions, turned profiles and booleans. There are no sculpted
  shapes.
- The gun wall draws flat side views with paths, not 3D images.
- The guns have no decals, logos or text on the parts.
