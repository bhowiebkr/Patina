# Mockup plan

The [GUI visual guide](gui-visual-guide.md) describes the current images in
[mockups/](mockups/) and their implementation rules. This plan lists further mockups to
request from Codex through the
[image handoff](../tools/image-handoff/README.md). A mockup is a design reference. It does not
prove that the program works.

Since 2026-11-15, Patina uses the current mockups and requests no new images. This plan stays
as a record. A row that ends with **Covered** has a current mockup that shows the same work.

## Rules for every mockup

- Follow the [spec](spec/README.md). The guide and spec section 0 list where the current
  mockups are wrong.
- Follow the palette and density in the [GUI visual guide](gui-visual-guide.md).
  [Mockup 9](mockups/mockup-9-smart-material-editor.png) is the main theme reference.
  Use neutral charcoal surfaces. Fit node widths and heights to their contents.
- Draw a Windows 11 window, with the caption buttons at the top right, as in mockup 1. Windows
  is the main platform (spec 13.5).
- The panels dock as tabs (spec 15.2). The left panel has the tabs **Layers** and **Project**.
  The top bar has a **Views** menu.
- The Layers panel has only the **Base** and **Slot** rows. No toggles, no colored layer list,
  no **+** button.
- The inspector header shows the node-kind icon, the node name, the node kind, the path and the
  layer. No Rust logo.
- Build time and run time have separate labels, for example "Built in 1.8 s" and "Run 0.42 ms".
- Node headers have the color that the node code sets, and the node bodies stay neutral (spec
  15.3), as in [the colored headers mockup](mockups/mockup-1-main-editor-colored-headers.png).
  Use the header colors of the guide. A selected node has an outline in a lighter shade of its
  header color.
- Green, yellow and red in the state dot, the error border and the logs mean node state. A
  header color never means a state. Each state also has an icon.
- Each node shows its kind icon, its clock domain icon, its state dot and its run time.
- Wires between two clock domains are dashed.
- The bottom panel has the **Logs**, **Output** and **Debug** tabs. No **Compare** tab.
- Colors that are data are allowed, for example image pixels, color stops and the color picker.

Proposed icons. The node states sheet (0014) can refine them.

| Meaning | Icon |
|---|---|
| Function node | An italic *f* |
| Stateful node | A small square with a dot inside |
| Generic node | `<T>` |
| Async node | An hourglass |
| Trigger node | A lightning bolt |
| Subgraph | Two nested squares |
| Batch domain | Play once: a triangle with a bar |
| Frame domain | A film frame |
| Audio domain | A sine wave |
| Event domain | A bell |
| Slot | A socket, in a neutral color |

## Node GUIs

A node can draw its own GUI with the `Ui` API (spec 4.3). The editor draws the widgets with the
Patina theme, so a node GUI looks like the rest of the editor.

- A node GUI has a compact size in the node body and a large size in the inspector. A node can
  also have a full size that fills the graph area (spec 4.3). Its header then shows an expand
  button.
- Spec 15.4: the inspector shows the large GUI at the top, under the header. The
  **Parameters**, **Code** and **Logs** tabs are below it.
- Widget fills are neutral light gray. The widget kit (0001) also shows a version filled with
  the header color of the node, so that the owner can choose. A focused control has an outline
  in a lighter shade of the header color (spec 15.3).
- Ask for high resolution: 3200 x 2000 when the image tool can make it. When the tool makes
  smaller images, get the detail from the framing instead: a higher graph zoom, fewer nodes,
  close-ups. Do not upscale a small image.

## Batch 1: node GUIs

The briefs are in `tools/image-handoff/requests/` as `.draft` files. To submit a brief, rename
it to `.md`.

| ID | Image | What it shows | Spec |
|---|---|---|---|
| 0001 | Node GUI widget kit | Each widget of 4.3 in the Patina theme, compact and large, with the fill color comparison | 4.3, 15.3 |
| 0002 | Node GUI gallery | Six nodes with compact GUIs at 200% zoom: transform gizmo, gradient map, tone curve, envelope, sample waveform, export panel. **Covered** by mockup 4 | 4.3, 9.1, 9.4 |
| 0003 | Image tool | The full editor: an image graph with node GUIs, the large curve GUI in the inspector, and the Layers panel when no node is in the slot | 4.3, 7.3, 15.2 to 15.4 |
| 0004 | Sound tool | The full editor: a sound effect graph in the audio and frame domains, with the large envelope GUI in the inspector | 4.3, 9.1, 9.3 |
| 0005 | Full-size node GUI | The Curve Editor GUI in the whole graph area, with **Back** and the path "Main Graph / Curve Editor" in the top bar. No brief yet | 4.3 |

## Batch 2: views

A view is a custom panel of the editor (spec 15.8). These images show what a project can build
with views. No briefs yet.

| ID | Image | What it shows | Spec |
|---|---|---|---|
| 0006 | Texture painter | A 3D object view with painted textures, docked in the center, and the texture graph. **Covered** in part by mockup 9 | 15.2, 15.8 |
| 0007 | Terrain generator | A 3D viewport of a procedural terrain, with the generator graph and its parameters | 15.8, 10.2 |
| 0008 | Sound designer | A timeline view and a spectrum view in a sound tool. The spectrum view floats in its own window. **Covered** by mockup 5 | 15.2, 15.8, 9.3 |
| 0009 | Scientific view | A plot view and a field view of simulation data, next to the graph. **Covered** by mockups 7 and 8 | 15.8 |
| 0010 | Project panel | The **Project** tab with the folders of the project, **Import** of a node folder, and **Update** of an imported folder | 6.2, 15.2 |

## Batch 3: editor states

| ID | Image | What it shows | Spec |
|---|---|---|---|
| 0011 | Editing a node | The code is open, the node is in the slot, the status bar shows "Slot: Exposure" | 7.2, 7.3, 15.4 |
| 0012 | Compile error | Mockup 3 corrected: the error in four places, and the graph keeps running | 7.2, 15.5 |
| 0013 | Layers states | The Layers panel and status bar in five states: fused, base rebuilding for a slot, slot ready, slot compile error, fusing | 7.2, 7.3 |
| 0014 | Node states sheet | The node kinds, the seven node states, the domain icons, the slot icon, selection and wires | 15.3 |
| 0015 | Debug timeline | The **Debug** tab: the threads of each domain, the nodes that ran in parallel, the frame budget and the audio deadline | 9.2, 15.7 |

## Batch 4: the rest

| ID | Image | What it shows | Spec |
|---|---|---|---|
| 0016 | Value viewers | Viewers on wires: image, table, plot, waveform, `Debug` text, type name and size | 5.3 |
| 0017 | Automation pipeline | The event domain: trigger nodes, async nodes, run IDs in the logs. **Covered** by mockup 6 | 9.4 |
| 0018 | First start | **Install toolchain** on Windows: the components, the folder and the progress | 13.2, 13.3 |
| 0019 | Trust prompt | The trust question and a read-only project | 11 |
| 0020 | Package export | Executable, C library and Python module | 14 |

## Screens that the spec now defines

These screens had no definition when this plan was written. The spec now defines them, and the
editor has them:

- The **Add node** palette and the **+** button that makes a node folder (15.2).
- **Add crate** in the **Code** tab, and the place of the large node GUI (15.4).
- The start screen and the settings screen (15.10). The fuse time goes into the settings screen
  when section 18, question 7 has an answer.

App mode (section 18, question 6) is not in version 1 (agent decision of 2026-11-16). A mockup
can help the owner change this decision, but only when the owner asks for one.
