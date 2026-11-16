# Mockup gaps

This file compares the mockups of the [GUI visual guide](gui-visual-guide.md) with the editor
on 2026-11-16. It lists each control and behavior that the editor does not have yet. The
[spec](spec/README.md) stays the source of truth. A gap here is not a decision to build it.

The mockups show many example nodes, for example **Transform**, **Camera** or **Score Lead**.
These nodes are user code, so this file does not list them. It lists the editor features and
the shared widgets of the SDK `Ui` that such nodes need.

## What the editor has

The editor has these mockup features. They are not gaps.

- The dock shell with the **Project**, **Inspector**, **Logs** and **Output** panels, the
  **Views** menu, splitters, float windows and the status bar (mockups 1, 5 and 8).
- The node header with the name, the kind icon, the domain icon, the state icon, the slot icon
  and the header color. The selected node gets a lighter shade of its header color (mockups 1,
  4 and 9).
- Fixed node sizes, the frosted node body, pins at the body edge and a run time footer.
- The **Inspector** with **Parameters**, **Code** and **Logs** tabs, the kind icon in its
  header, Monaco with line numbers, syntax colors and error marks (mockups 1 and 3).
- Log lines and the compile error count of the status bar open the source location
  (mockup 3).
- A failed build keeps the last build that compiled. The node, the code, the log and the status
  bar show the same error (mockup 3).
- Compact node GUIs in the node body, and **Full** GUIs in the graph area with **Back** and Esc.
- Custom views in dock panels, with no pins (mockups 5, 8, 9 and 10).
- The automatic route around the nodes, route points, orthogonal segment bars, **Reset route**,
  the conflict mark, dashed cross-domain wires, bus ribbons, lane names and lane tracing
  (connections guide and mockups 12 to 16).
- Subgraphs with public pins as graph objects, **Collapse** of a selection and the expansion of
  uses into the graph (mockups 10 and 11).
- An **Open subgraph** button on each use, and a path bar with each use of a nested path
  (mockups 10 and 11).
- **Show inner nodes** on each use: the nodes of the subgraph in place, in a neutral frame,
  read-only and with live values. **Hide inner nodes** collapses it (mockups 3 and 11).
- A drag on the end of a wire moves it to another socket (connections guide).
- During a wire drag, the compatible sockets get a ring and the other sockets dim (connections
  guide).
- A junction dot where the wires of one output part (connections guide).
- A gap in the narrower wire where two wires of different outputs cross (mockup 16). A raised
  bridge stays an optional refinement.
- The **Subgraph interface** in the **Inspector**: the public pins in pin order, with their
  names, types and a preview of the use (mockup 11).
- The SDK `Ui` widgets: label, button, toggle, slider, number, text field, dropdown, color,
  plot, waveform, table, curve, image, 2D canvas and GPU viewport.

## Editor gaps

| Gap | Mockup | What the editor does now |
|---|---|---|
| A collapsed navigation rail that gives the graph more room | 6 | A panel closes or moves to another area. |
| **Lock bends** (optional in the guide) | Connections guide | The route points are fixed, and the router moves the free segments. |

## Shared widget gaps

The guide asks for one shared widget for each recurring control. The SDK `Ui` does not have
these widgets yet. A node can draw some of them on a 2D canvas, but then each project makes
its own version.

| Widget | Mockups | Note |
|---|---|---|
| Color ramp with stops, and the color, position and interpolation of the selected stop | 4, 9 | Gradient Map, Patina Mask and Material Layers. |
| Layer stack: ordered rows, drag handles, visibility, swatch, amount and the properties of the selected layer | 9 | Material Layers. |
| Tabs and collapsible sections in a node GUI | 4, 6 | The channel tabs of Curve Editor, the Payload of Webhook. |
| Plot axes with units and labels, a histogram behind a curve, and a scatter plot | 4, 7, 8, 9 | The plot shows the high and low values only. |
| Trim handles on a waveform | 4, 5 | The waveform has a playhead only. |
| A number field with a unit and step buttons | 4, 7, 9 | The number field has a label and a value. |
| A file path with a **Browse** button | 4 | Batch Export. A dialog of the platform needs a person, so a test cannot click it. |
| A progress bar | 8 | Sample Window. |
| A 2D transform gizmo with pivot, rotation and axis handles | 4 | A canvas with pointer events can draw it. |
| An asset picker | 9 | Mesh Maps. |
| Icon buttons with accessible names | 7 | Orientation. The button has text only. |
| A timeline with a ruler, clip lanes and automation lanes | 5 | A custom view. |

## Next steps

Each gap needs a spec decision before the build. The editor gaps change the editor only. The
widget gaps change the SDK `Ui`, the frontend and the widget tree of spec 4.3. Do the editor
gaps first, because each graph uses them.
