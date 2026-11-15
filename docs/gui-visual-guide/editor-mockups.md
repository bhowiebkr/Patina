# GUI visual guide: Editor mockups

Part of the [GUI visual guide](../gui-visual-guide.md).

## 9. Smart material editor

![Primary reference: smart material editor with varied nodes and a docked shader preview](../mockups/mockup-9-smart-material-editor.png)

[Open the full-size reference](../mockups/mockup-9-smart-material-editor.png).

**Match:** the balance of graph and preview, readable controls, neutral bodies,
colored title strips, visible data, and strong differences in node size.
This is the main theme reference, with the remaining blue cast removed by the
proposed neutral palette.

The graph has sources on the left, mask editing in the middle, composition to
the right, and a small output. The preview is a docked workspace.

| Exact image detail | What an agent should implement |
|---|---|
| <img src="../mockups/details/nodes/mesh-maps.png" alt="Mesh data sources" width="201"> | **Mesh Maps: a source with two kinds of data**<br><br>Keep the asset selector at the top and identify each preview next to its output. Curvature and AO must look like distinct channels, not interchangeable decorative balls. The body needs enough width for those names, but no large property form. Reuse the asset picker and channel-image widget. |
| <img src="../mockups/details/nodes/fractal-noise.png" alt="Procedural noise source" width="208"> | **Fractal Noise: visual feedback over a short form**<br><br>The square texture is the main feedback. The three numeric rows belong below it, followed by projection mode. Keep field widths stable while values change. Scale, detail and seed must update the preview; changing preview zoom must not change the noise. Reuse a preview plus labeled number-field group. |
| <img src="../mockups/details/nodes/wear-curve.png" alt="Mask and editable curve" width="322"> | **Wear Curve: spatial mask beside a remapping function**<br><br>The mask thumbnail answers where the operation acts; the curve answers how strongly. Keep the histogram subordinate to the curve. Dragging a point and editing a threshold must affect the same mapping. Reserve width for the two visualizations rather than stacking them into a narrow column. |
| <img src="../mockups/details/nodes/patina-mask.png" alt="Mask authoring GUI" width="293"> | **Patina Mask: a large interactive mask editor**<br><br>Image first, remap ramp second, parameters last. The stops need obvious handles and a selected state. Keep coverage, contrast and cavity bias aligned. The light teal node outline indicates selection; the green dot indicates readiness. The ramp is grayscale data and should not inherit the header hue. |
| <img src="../mockups/details/nodes/copper-base.png" alt="Compact base material" width="248"> | **Copper Base: compact material properties**<br><br>A small rendered image and color swatch establish the material. Two labeled sliders complete the task. This node does not need the height of the layer stack. Clicking the swatch should use the shared color editor; metallic and roughness need exact values as well as dragging. |
| <img src="../mockups/details/nodes/material-layers.png" alt="Ordered custom material stack" width="322"> | **Material Layers: an ordered editor inside one node**<br><br>Rows combine a reorder handle, visibility, swatch, name and amount. Selection expands one layer below the list. The layer's ramp, roughness, metallic and height form a single coherent property group. The stack needs height; the controls need consistent columns. This is material content, not Patina's compilation Base/Slot panel. |
| <img src="../mockups/details/nodes/material-output.png" alt="Small material sink" width="152"> | **Material Output: let a sink stay small**<br><br>The input, PBR summary and channel disclosure are sufficient. Do not add a large duplicate preview just to match neighboring nodes. The preview dock can bind to this output. Keep its source binding explicit and distinguish a stale result from a current result. |
| <img src="../mockups/details/views/material-preview.png" alt="Docked shader preview and shared controls" width="396"> | **Material Preview: the tool's main viewing surface**<br><br>A neutral dock header contains source selection and Reference ball/Current asset tabs. Give the 3D object most of the available area. Keep orbit/reset controls on the viewport. Material, lighting and channel controls are distinct groups. The lower sliders edit shared graph values; lighting changes the presentation. Channel thumbnails should come from the actual material result, not independent sample images. |

**Connections to implement:** Curvature feeds Wear Curve; AO and Fractal Noise
feed Patina Mask; Copper Base, the patina mask and the wear mask feed Material
Layers; Material Layers feeds Material Output. The view has no graph cable.

**Correct these details:** the wear cable lands near the Verdigris row in the
image. It must land on **Wear mask**. Restore node kind/domain icons and runtime
labels where needed. Use the blue faceted logo. Rendered material/channel images
must agree with actual graph outputs; the mockup does not validate a shader.

## 1. Main editor with colored headers

![Main editor layout with graph, project navigation, code inspector and logs](../mockups/mockup-1-main-editor-colored-headers.png)

[Open the full-size reference](../mockups/mockup-1-main-editor-colored-headers.png).

**Match:** a stable shell with navigation at left, graph in the center, inspector
at right and logs below. The graph remains visible while code or parameters are
edited. The right panel gives the selected node context rather than opening an
unrelated window.

![Toolbar detail](../mockups/details/chrome/main-toolbar.png)

The toolbar has a hierarchy: project/graph path, run state, editing/navigation
controls, then execution and view actions. Keep related controls grouped and
separate them with space or thin dividers. Do not make every tool a colored button.

| Exact image detail | What an agent should implement |
|---|---|
| <img src="../mockups/details/chrome/code-inspector.png" alt="Code inspector and parameters" width="410"> | **Inspector: identity, work area, parameters and feedback**<br><br>Keep Parameters/Code/Logs as a stable tab group. Code needs line numbers, syntax highlighting and error locations. Parameters below it use the same fields as the node GUI. The selected-node name and path must remain visible. Use a Patina node-kind icon in place of the Rust logo. Build time and run time must have different labels. |

The five node headers demonstrate a category sequence: amber input, teal
processing, magenta color editing and purple output. Inline images are useful
previews, not a reason to enlarge every node.

**Correct these details:** replace the obsolete edit-layer list and toggles
with compilation **Base** and **Slot**. Remove orphan-edit concepts. The selected
magenta node should have a light pink outline. Apply neutral surfaces instead
of this image's blue cast. The logo remains a valid visual reference.

## 3. Error view

![Error workflow with expanded subgraph, code inspector and error log](../mockups/mockup-3-error-view.png)

[Open the full-size reference](../mockups/mockup-3-error-view.png).

**Match:** an error can be investigated without hiding the graph. An expanded
subgraph exposes its internal dependency chain. The inspector explains the
problem close to the code.

| Exact image detail | What an agent should implement |
|---|---|
| <img src="../mockups/details/chrome/compile-error.png" alt="Code location and compile error" width="380"> | **Error detail: identify the location and the mismatch**<br><br>Give the error an icon, a short heading and a readable message. Mark the actual source line with a gutter marker and underline. Keep code readable above the message. A red-tinted message well can be subtle; the important information is node, source location and expected/actual type. |

![Error log detail with source location](../mockups/details/chrome/error-log.png)

Log entries must link back to the node and source location. Match the compact
timestamp/message structure. Keep prose and code locations readable, and avoid
giving the entire log the same error color.

**Correct these details:** the failing node needs an error border/icon, not a
green ready dot. The subgraph border stays neutral and needs a collapse control.
Remove **Compare**, **Layer Edit** and the orphan edit warning. Do not copy the
claim that the graph aborted: a failed rebuild retains its last successful
build. The status bar, node, code and log must describe the same error.
