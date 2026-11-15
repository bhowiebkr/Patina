# GUI visual guide: Shared controls and implementation

Part of the [GUI visual guide](../gui-visual-guide.md).

The cutouts above are examples of one control vocabulary. Build shared widgets
for their recurring interactions; do not recreate an unrelated style for each
project. The same parameter shown in two places must represent one value.

## Typography and spacing

Use Segoe UI or the Windows UI font stack for controls. Use a monospace face for
code and logs, with tabular digits for measurements and times. These are starting
metrics at 100% UI scale, not sizes measured from the mockups:

| Element | Starting size |
|---|---|
| Normal label and value | 13 px |
| Secondary text | 12 px |
| Node title | 14 px, semibold |
| Panel title | 14 px, semibold |
| Control height | 26-30 px |
| Node header height | 30-34 px |
| Small icon drawing | 16 px, inside a larger hit area |
| Node body padding | 10-12 px |
| Gap within a control group | 6-8 px |
| Gap between groups | 12-16 px |
| Control corner radius | 3-4 px |
| Node corner radius | 6 px |
| Normal border / selected outline | 1 px / 2 px |

Size for Windows display scaling and readable text. Do not scale a whole PNG
into an interface or compress every control to make a graph fit. Keep labels
and values aligned. Put units beside values. Use short labels and expandable
details instead of repeated large blocks of explanation.

## Shell and docking

The window is a dock system. Keep a narrow top toolbar, a central graph, optional
side panels, optional lower panels, and a slim status bar. The graph should get
most of the space while it is the main task.

**Project** lists **Graphs**, **Nodes**, **Views**, **Widgets** and **Types**.
Its node entries can repeat their header color as small swatches. **Layers**
contains the compilation **Base** and **Slot**. It is not an effects stack.

A custom view has the same neutral tab, title, splitter, float and close controls
as a built-in dock. A selected tab uses the application accent. Docks can move
and float as defined in spec 15.2. Keep the Windows caption controls familiar.

The **Views** menu opens panels. Expanded node GUIs use the graph area and provide
**Back** or Esc. These are different navigation actions.

Keep the status bar concise: state, error count, project and compile-layer state.
Build time and execution time need separate labels. Do not use decorative badges
to imply a service is connected or a node has finished running.

## Header, body and pins

A header contains the node name, kind icon, clock-domain icon and state indicator.
Show the slot marker when relevant. Place expand/collapse actions consistently.
Do not replace kind icons with a Rust logo.

Put inputs on the left and outputs on the right, with readable names. Pins sit
at the body edge. Route wires to pins, not to a control row or a preview image.
The space for a pin label must not collide with a slider or button.

Use subtle gray wires by default, or named signal-family colors where useful.
Same-domain wires are solid. Cross-domain wires are dashed. Do not color all
wires green because the graph ran successfully. Use the application accent
for selection without erasing a cable's lane colors. Follow the full
[connection rules](connections.md) for routing and interaction.

## Layout and interaction

Arrange dependencies from left to right where practical. Branch only when the
data branches. Keep visible gutters for wires and avoid routing through nodes.
An output can be tiny; it should not resemble a large data editor.

Use neutral borders for subgraphs. Their label and collapse control explain
structure. A subgraph must not look like a warning because its enclosing box
happens to be yellow.

Graph gestures and GUI gestures must stay separate. Dragging a curve handle
edits the curve; dragging a node header moves the node. Interacting with a field,
plot or scrollable table must not accidentally drag the whole node. Keep focus
visible and expose meaningful names for icon-only actions.

## Reusable widgets

Use the same control vocabulary everywhere. A slider in a node and the same
slider in a view should share geometry, text, focus and keyboard behavior.

| Element | Visual and interaction guidance |
|---|---|
| Number field | Label, value and unit; stable alignment; optional compact step buttons |
| Slider | Thin neutral track, clear thumb and an exact numeric field when precision matters |
| Toggle / checkbox | Clear on/off shape; local focus color; no success meaning merely because checked |
| Dropdown | Single-line value, visible disclosure; avoid repeating a full form for a simple summary |
| Curve | Plot axes, meaningful range, visible handles, selected-point values, reset action |
| Color ramp | Stops with handles, selected color and position; do not rely on the ramp alone for precision |
| Image / mask | Stable preview area, optional overlays and handles; explicit crop/ROI coordinates |
| Waveform | Time axis, playhead and trim handles; transport separate from graph execution |
| Table | Aligned columns, units in headings, selected row and compact row actions |
| Layer stack | Ordered rows, drag handles, visibility, amount, selected layer and expandable properties |
| Plot | Labeled axes, units, legible traces, uncertainty where relevant; restrained grid |
| Viewport | Clear source selection, orbit/reset controls and relevant lighting or display controls |

Make reusable widgets for recurring patterns. Do not implement a second curve
editor or a different table style for each example project.

The SDK's Rust **Ui** function describes a widget tree; the editor draws it with
the shared theme. Nodes do not supply arbitrary independent web pages.
Reusable controls live in widget folders. See spec 4.3 and 6.2.

Node GUIs support **Compact** in the body, **Large** in the inspector and optional
**Full** in the graph area. More space should reveal useful controls, not merely
magnify a bitmap. Graph computation must work without the GUI.

Ordinary node UI runs between processing calls. Audio GUI changes use settings
and messages; they do not mutate the realtime processing state directly.
Preserve these rules when implementing responsive controls.

## Node GUI versus custom view

A node GUI edits or displays one node. It has a colored node header and graph
pins. A custom view can coordinate several nodes. It has a neutral dock header,
no pins and no clock domain.

A view reads graph outputs and sends parameter edits or commands through the
same paths as the editor. Do not duplicate the measurement session, timeline
or material state in the panel. Undoable edits use the graph's undo path.

Compact node bodies may show image previews, plots and 2D canvas controls.
Interactive GPU viewports belong in a view or a Large/Full node GUI. A rendered
ball thumbnail inside a node is an image, not a separate interactive 3D surface.
