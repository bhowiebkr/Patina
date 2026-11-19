# Smart material workbench

Status: native PBR material prototype verified on Windows, 2026-11-19.

The owner asked to bring the smart material side of Millwright Surface into Patina.
The owner selected the existing GPU example and a second top-level graph during this study.
This document records the source findings and the proposed workbench.
The first implementation adds native core nodes and a separate material graph.
The acceptance records below distinguish implemented code from verified behavior.

## First implementation

- Eleven node types expose the ten core kernels and a material output.
- The copper example has 28 native nodes, 42 wires, and six groups.
- Each operation uses Patina's scheduler and explicit GPU inputs.
- Outputs keep three float32 planes for materials, or float32 masks.
- Node thumbnails use asynchronous readback. Unchanged inputs reuse the previous output.
- The PBR view uses Surface's renderer, mesh, studio light, and baked maps.
- Kernel body files can change without a Rust build. The adapter checks their declarations.
- The imported source revision is `2f47ff8160c2bd8362dee42b6bd4f11e460e06c8`.

The initial Windows test exposed FXC compiler errors on an RTX 4090.
Some dynamic array writes failed inside the DirectX shader compiler.
The implementation enables statically linked DXC on Windows x64.
This avoids a runtime DLL installation. See the [wgpu compiler documentation](https://docs.rs/wgpu/30.0.2/wgpu/enum.Dx12Compiler.html).
The vendored GPU helper also captures internal compiler errors, not only validation errors.

The GPU integration check passed on Windows MSVC with an RTX 4090 and DX12.
All three material planes matched Surface's evaluator exactly at 512 x 512.
The maximum absolute float difference was 0. All 640 x 640 shaded preview pixels also matched Surface's render path.
Initial evaluation took 0.533 seconds, including first shader compiles and readback.
Context creation took 1.620 seconds. Cached evaluation averaged 0.138 milliseconds across 100 runs.
These debug measurements exclude allocation counts and do not establish general performance.
Metallic and roughness tests changed shading while keeping base color identical.
Shader edits changed GPU output. Invalid declarations failed validation.
Hidden editor checks passed narrow previews, channel selection, parameter edits, undo, save, and top-graph switching.
Windows workspace Clippy, workspace tests, frontend builds, and frontend tests passed in an isolated snapshot.
Two regression tests cover shared GPU hash state across libraries and typed view reads after cluster replacement.

Static DXC needs ATL when linked with MSVC. The build finds an existing ATL installation.
The portable Windows GNU material path remains unverified; `wgpu` currently gates static DXC to MSVC.

Remaining work includes general recipe import and a full material library.
The current example preserves the original copper container and its pinned shaders.
It does not expose the Paint workspace.

## Owner corrections, 2026-11-19

- Keep the renderer, shaders, mesh baker, and texture export in the example.
- Material node previews must show the full PBR response, not only lit base color.
- Reuse Millwright's render path and compare its rendered result.
- Add an Object node for loading the mesh.
- Add object mesh-map baking and finished PBR texture passes.
- Use Millwright's original material sphere, UV atlas, and saved 2048 x 2048 mesh maps.

The first node implementation used Surface's cheap thumbnail shader for material previews.
That shader ignores roughness and metallic. This was a gap in the first implementation.
Material previews now use Surface's complete PBR renderer. Mask previews still show scalar coverage.
GPU tests confirm that roughness and metallic change shading without changing base color.
Object loading, explicit baking, and PBR export now have an implementation inside the example.
The previous demonstration used an invented bracket. This did not meet the owner's request.
The default graph also recomputed sphere maps at 512 instead of loading the original 2048 bake.
The bracket is removed. Object now loads the original sphere, and Bake loads its original saved maps.
The sphere mesh, asset metadata, and compressed maps match the current Millwright files byte for byte.
The current source has one extra cache-header query method; its rendering and bake algorithms are unchanged.
GPU checks passed for Object and Bake against the independent reference session.
Loaded position, normal, curvature, occlusion, thickness, part, and contact maps matched exactly at 2048 x 2048.
All three material planes matched at 512 x 512. All 640 x 640 shaded preview pixels matched.
An explicit 2048 bake also matched Millwright's direct baker byte for byte.
The sphere has 82,228 triangles. Its bake, comparison, render, and export check took 13.551 seconds on the RTX 4090.
Hidden editor checks passed the original-map load, explicit rebake, saved-map restore, and eight-pass export.
The editor also passed narrow docks, channel selection, parameter edits, undo, save, and graph switching.
The graph passed CLI edit and fused builds. Workspace Clippy, workspace tests, and frontend checks passed on Windows.
Checks used an isolated snapshot of committed sources plus the material changes, excluding concurrent edits.
Unit tests confirm that saved export requests do not arm a write and existing files remain unchanged.

The graph now uses explicit Object and Bake nodes before its original 28 material operations.
Each output carries its bake context. Objects do not share a mutable selected-mesh global.
The loader calls Surface's `MeshMaps::from_bytes`, which validates the mesh and resolution key.
An explicit bake calls Surface's `MeshMaps::bake` with tracing enabled at the object's atlas resolution.
Material output resolution does not change mesh-map resolution.
The export uses Surface's PBR channel order, +Y normal convention, and 16-bit height conversion.
The ZIP adds opacity and a channel description. Existing files are never replaced.
The export button arms one write; opening or running a saved graph does not arm another write.

OBJ import requires triangles and a prepared UV atlas. It does not unwrap or detect overlapping charts.
Millwright asset import validates mesh indices, parts, finite attributes, UV range, normals, and the fitted camera.
Saved Millwright maps load from `maps.bin` or `maps.bin.zlib` beside the asset metadata.
Saving newly traced bake files, other object formats, and export above 2048 remain open.

## Scope

Extend `examples/gpu` into the smart material workbench.
Keep the graph as its main work surface.
Use native Patina nodes, type crates, widgets and dock views.
Reuse Surface's material algorithms, data contracts and renderer.

Keep the existing gradient, bars, mix, histogram and waveform demonstrations.
They provide small checks for GPU transport, shader reload and image display.
Add `graphs/materials.toml` with `top = true` for the material workbench.
Keep the current `graphs/main.toml` as the GPU demonstration.
Use Patina's existing top-graph selection to open and run either graph.

Include procedural materials, mesh-driven masks, physical coats, channel inspection,
editable recipe import, and texture export.
Mesh maps belong in this scope because smart materials depend on them.
Brush painting, paint layers, object painting and the Paint workspace are outside this scope.

The first acceptance material is **Oxidized copper**.
Its existing graph is the compatibility reference, not proof of final material quality.
After parity, improve its controls and test its response on a second mesh.

## Evidence and limits

The study inspected these Surface components:

- `docs/wiki/`, `docs/materials/ARCHITECTURE.md` and `docs/materials/FORMAT.md`.
- `forge-core`, `forge-gpu`, `forge-nodes`, `forge-render`, `forge-png` and `forge-runtime`.
- The copper graph writer, the editable PNG, and the Materials workspace screenshot.
- Runtime round-trip tests and the kernel evaluation, inspection and export paths.

The Surface checkout was at `2f47ff8160c2bd8362dee42b6bd4f11e460e06c8`.
It had unrelated edits in the host, document, paint and mesh-map code.
This study changed no Surface files and ran no Surface application code.
The integration must select a recorded source revision before copying code.

Patina's GPU, graph, packaging, view and GUI contracts were inspected.
Patina was at `d72fd03950535dda155bbd0c7786c69a7baa61b8` during that inspection.
Another agent continues editor work, so these references identify the inspected state.

Labels used below:

- **Observed:** a data inspection ran during this study.
- **Source finding:** source was read, but the behavior was not run here.
- **Proposal:** an implementation or acceptance check still to do.
- **NOT_ENOUGH_INFO:** evidence does not establish the result.

## What Surface already supplies

| Component | Reusable work | Adaptation boundary |
| --- | --- | --- |
| `forge-core` | Mesh, bounds, asset and part data | Keep the existing mesh format initially. |
| `forge-gpu` | Texture targets, validation, uploads and readback | Use Patina's device and queue. |
| `forge-nodes` | WESL declarations, linker, parameter packing, kernels and texture pool | Separate kernel execution from Surface graph scheduling. |
| `forge-render` | PBR shading, reference mesh rendering, lighting and mesh-map baking | Render offscreen, then present through a Patina viewport. |
| `forge-sources` | Image decoding and source interpretation | Retain the bitmap path needed by materials. |
| `forge-png` | Editable PNG, bounded decoding, hashes and atomic storage | Convert graphs while preserving pinned sources and resources. |
| `forge-runtime` | Material validation, subgraphs, requirements and channel output | Reuse selected functions; use `Session` as the initial comparison host. |
| `forge-ui` | Existing authoring behavior and export adapters | Study and extract necessary non-UI helpers. Do not embed its egui workspace. |

The seven lower-level crates contain about 15,464 Rust source lines.
This count excludes tests, examples, shaders, assets and `forge-ui`.
None of their inspected Rust source files exceeds Patina's 1000-line limit.
This is substantial reusable code, but it is not a drop-in Patina node library.

The current kernel folder contains 20 definitions.
Ten are core operations, nine are material operations or interface helpers, and one is **Paint Mask**.
Exclude **Paint Mask** from this workbench's authoring palette.
Reject an imported recipe that needs unsupported painting state before changing the project.

### Existing data flow

1. Read the PNG and validate its manifest, hashes and resources.
2. Restore pinned shader sources and linked WGSL.
3. Validate and flatten declared subgraphs.
4. Bind the target mesh maps, bounds and part table.
5. Compile kernels and evaluate dirty graph nodes into GPU textures.
6. Bind the final material planes to the PBR renderer.
7. Render the selected mesh with its lighting and camera.
8. Read pixels only for thumbnails, inspections that need CPU data, or export.

`Session::new` supplies the reference ball, its baked maps and the studio environment.
It accepts an existing device and queue.
Its graph and document state belong to Surface, so the full session cannot become Patina's authoring model.

## Copper recipe inspection

**Observed:** a read-only Python inspection checked the original PNG without executing shaders.
It checked PNG chunk CRCs, bounded decompression, source hashes, shader hashes and resource hashes.
It also checked unique node IDs and link endpoint IDs.
These checks passed.
They do not replace `forge_png::load` or runtime graph validation.

| Item | Observed value |
| --- | --- |
| File size | 1,700,794 bytes |
| Container / material schema / graph version | 2 / 1 / 2 |
| Nodes / connections / groups | 28 / 42 / 6 |
| Kernel kinds | 8 |
| Pinned source files / linked shaders | 57 / 8 |
| Embedded resources | One HDR environment, 524,337 bytes |
| Exposed controls / presets / named subgraphs | 0 / 0 / 0 |

The file SHA-256 is
`456fe6c0827147745a315c4d1b8e113b2bf8156f33d8f7f3388998886c1c21ba`.

| Kernel | Instances | Material role |
| --- | --- | --- |
| Noise | 3 | Broad variation, surface grain and fine grain |
| Surface | 4 | Edge, upward exposure, shelter and crease drivers |
| Spread | 2 | Surface-aware softening |
| Combine | 7 | Shared wear and deposit coverage |
| Mask | 6 | Coverage thresholds and breakup |
| Layer | 4 | Copper, oxide, mineral deposit and worn conductor |
| Finish | 1 | Final height normal and baked occlusion |
| Output | 1 | Final material |

Pinned sources include support files absent from the current catalog.
Two files have substantive differences: `surface.wesl` and `prelude.wesl` gained the **Near** driver.
`noise.wesl` also differs as a complete string, but its split lines match.
Therefore, import must keep exact pinned source bytes and linked shader text.
Replacing them with current catalog files would change the recipe identity.

The embedded environment has empty source and license fields.
Preserve that metadata and confirm redistribution provenance before bundling the fixture and environment.
Do not invent attribution.

## Material contracts to preserve

### Channels and precision

Surface stores each material in three `Rgba32Float` texture layers.

| Layer | Components |
| --- | --- |
| 0 | sRGB base color RGB; opacity A |
| 1 | Roughness R; metallic G; height B; ambient occlusion A |
| 2 | Encoded tangent normal RGB; OpenGL positive Y convention |

Masks are linear scalar data.
Object-space mesh normals are different from tangent-space material normals.
Coverage in the position map is different from opacity.
Part IDs are categorical and must not pass through color interpolation.

The current exporter writes seven files: BaseColor, Roughness, Metallic, AO, Height, Normal and ORM.
ORM packs AO, roughness and metallic; it is not another material channel.
BaseColor exports RGB only, so this path drops opacity.
Add an explicit opacity export before claiming that every material channel survives export.
Keep the seven-file compatibility preset for existing consumers.

Patina's existing `GpuImage` uses `Rgba8Unorm`.
Use new material types instead of passing material values through that image type.
Keep float precision through evaluation and convert only for display or the selected export format.

**Layer** applies one mask to color, roughness, metallic and height.
It adds thickness and relief to existing height, which can exceed the usual 0..1 display range.
**Finish** derives the normal from final height and replaces the incoming normal.
It multiplies incoming AO by baked AO; it does not calculate AO from height.
The renderer does not displace geometry from the height channel.

### Mesh context

Smart inputs include position, normal, curvature, AO, thickness, part identity and distance to another part.
The baked-map cache uses `MWMAPS03`.
Untraced maps use neutral AO, thickness, curvature and distance values.
Show missing or neutral drivers explicitly; do not label them as valid traced measurements.

Current procedural lengths use the largest bounding-box side.
They are not meters or millimeters.
The **Up** direction is object-space Y unless the selected driver uses another axis.
Camera and lighting changes must not rebake these maps.

Texture-domain inputs can project onto a mesh through triplanar sampling.
An atlas-authored input uses its UV flag instead.
Keep the domain, resolution, projection and source interpretation with each texture value.

## Integration design

### One editable graph

Use Patina's graph as the only authoring state.
Each visible material operation becomes a real Rust node with typed pins.
Parameters, connections, groups and undo use Patina's existing edit path.
Views inspect outputs and edit those same parameters.

Do not embed the Surface canvas or keep a second editable graph behind it.
Do not place the entire copper graph inside one permanent **Material** node.
An opaque recipe loader is acceptable only as a comparison fixture in the first prototype.

Surface graphs remain useful at import, export and comparison boundaries.
A converted graph is data for those operations, not a second live document.

### Native execution

**Proposal:** extract a small kernel execution API from `forge-nodes`.
Its inputs are a pinned definition, packed parameters, texture bindings and mesh context.
Its result is a typed GPU output and diagnostics.
Reuse the existing code generator, prelude, multipass rules and shader implementations.

The current `Engine::record` is private and reads an entire Surface graph.
`Engine::evaluate` also owns scheduling, dirty propagation and the memory budget.
These are the main extraction seams.
Creating a complete Surface engine for every Patina node would duplicate caches and resources.

| Proposed type | Meaning |
| --- | --- |
| `MeshContext` | Mesh maps, bounds, part table, target identity and revision |
| `GrayField` | Scalar texture output, revision, resolution and sampling domain |
| `ColorField` | Color or declared numeric texture output with source interpretation |
| `MaterialSet` | Three material planes and their shared context and revision |

Use cheap cloned GPU handles for readers and view snapshots.
Keep shader pipelines and uniform buffers between runs.
Update a uniform for a numeric edit; do not compile a shader for each slider value.
Cache by source identity, parameter revision, input revisions, context and resolution.
Pass mesh context into source operations and retain its identity in their outputs.
Reject incompatible target contexts when two fields combine.
Do not let a global selected mesh silently change another open material.

Run material computation in the batch domain.
Redraw the camera through the view without evaluating materials again.
Patina currently calls every batch node after an edit, so unchanged nodes need a local cache hit.
Do not add general engine dirty propagation as a prerequisite.

GPU outputs must remain valid while a view, another node or an export owns them.
Return textures to a shared bounded pool only after their final owner releases them.
Do not overwrite a texture while an export owns its captured revision.
Use another pooled target when an edit must produce a new revision during export.
Give each concurrent node its own scratch lease; Surface's serial scratch reuse cannot be copied unchanged.
Serialize shader error-scope operations where necessary, without serializing all material evaluation.
Publish a new output revision only after its complete kernel sequence succeeds.
Do not present a partial result as the last valid material.

Surface permits Gray/Color conversion at a connection.
Patina requires explicit typed conversion nodes.
Reuse Surface's luminance and replication rules in those adapters.
Verify whether the existing sampling metadata can avoid an extra GPU pass.

### Why this route

| Route | Benefit | Decision |
| --- | --- | --- |
| Embed the existing egui workspace | Fastest way to display Surface | Reject: it bypasses Patina's graph, views and undo. |
| Build a Surface graph inside one Patina node | Reuses the evaluator with few changes | Use only for the initial reference test. It hides the authoring graph. |
| Native nodes with reused kernel execution | Gives real pins, diagnostics, source editing and shared widgets | Preferred implementation; prove the extraction with copper first. |

If the native execution prototype cannot preserve channels and resource lifetime, revise that boundary before expanding the node set.
Do not conceal a failed prototype behind an embedded editor.

### Project layout and source reuse

```text
examples/gpu/
  patina.toml
  graphs/             existing main, new materials top graph and learning subgraphs
  nodes/              existing GPU nodes and new material operations
  types/materials/    GPU value types and the Patina runtime adapter
  types/forge-*/      the required local source crates, with original notices
  widgets/material/   previews, ramps and material controls; keep widgets/scope
  views/              material docks alongside the existing scene view
```

Keep required assets within these packaged folders.
Patina's package source list does not copy an arbitrary top-level `materials/` or `assets/` folder.
Keep original source identities, notices and an upstream revision/hash inventory with copied code.
Use local source dependencies in the delivered example, not absolute paths to the Surface checkout.
Preserve the original MPL notices instead of inheriting Patina's license for copied files.

Keep Surface-specific code out of `patina-engine`.
Limit shared changes to reusable GPU capabilities, value inspection, precise controls and graph transactions.
Read the full Patina spec before implementation changes.

## Workbench and node design

Use [mockup 9](gui-visual-guide/editor-mockups.md#9-smart-material-editor) as the visual starting point.
Keep neutral bodies, family-colored headers, readable pin labels and visible keyboard focus.
Use useful node sizes instead of making every operation the same square.

| Node | Compact body | Larger inspector or full view |
| --- | --- | --- |
| **Mesh Maps** | Target, bake state and distinct channel thumbnails | Map validity, resolution, bounds, axes and bake controls |
| **Noise / Cells** | Actual output, scale, seed and the main shape controls | Stretch, direction, octaves and secondary controls |
| **Surface** | Driver name and an actual target-mesh mask | Driver explanation, axis and source-map state |
| **Mask** | Mask preview beside its threshold transfer curve | Exact From/To/Low/High values and breakup settings |
| **Combine** | A/B previews, operation and amount | Larger result and clear operation semantics |
| **Spread / Flow** | Before/after preview and distance or direction | Surface sampling diagnostics and exact values |
| **Layer** | Coat name, mask preview, color ramp and material response | Roughness endpoints, metallic, thickness and relief together |
| **Finish** | Normal preview, depth and occlusion | Height/normal comparison and normal convention |
| **Output** | Material name, resolution, channels and stale/error state | Source binding and result details |

The **Mask** curve represents its current cubic threshold operation.
It must not suggest an arbitrary editable curve that the shader does not implement.
Show **Layer** thickness and relief together because both affect the resulting normal.
Give **Output** a small body; the dock owns the detailed result preview.

Starting widths are 220-260 px for sources and 300-360 px for mask or coat editors.
An output can use 160-200 px.
These are prototype targets, not fixed limits.
Test them at normal editing zoom and Windows display scaling.

### Dock views

| Place | Tabs | Content and responsive behavior |
| --- | --- | --- |
| Left | **Materials**, **Learn** | Searchable recipe cards, pinned source information, and short material experiments. Use one column when narrow. |
| Right | **Preview**, **Controls**, **Mesh Maps** | Orbitable target, lighting, selected graph controls, and map validity. Stack fields vertically. |
| Bottom | **Channels**, **Export** | Seven channel tiles or a selected channel; output files and progress. Scroll horizontally when short. |

Keep the central graph visible by default.
Use a single interactive GPU viewport initially.
The bottom channel strip uses cached images and opens a selected channel in that viewport.
At narrow sizes, show a channel selector instead of shrinking seven plots into unreadable tiles.

Test side docks near 220, 320 and 420 px.
Test bottom docks near 160, 240 and 360 px high.
Allow a large dock or full view for pixel inspection.
Do not require a wide fixed performance card to use the tool.

The selected-node **Controls** view follows the current selection.
An optional pin keeps its source explicit.
Camera, channel selection and display exposure are view state.
Material parameters and mesh target selection are saved graph state.
Export settings must identify the captured material revision.

### Useful shared controls

- A channel preview with a label, source, range, zoom and stale state.
- A precise float color ramp that preserves imported stops and alpha.
- A threshold editor linked to exact numeric fields.
- A mesh-map status table with valid, neutral, missing and obsolete states.
- A selected-material parameter list bound through normal graph edits.
- An export job display with progress, cancellation and a named output location.

Reuse existing number fields, color controls, plots, asset pickers and sections.
Add a reusable widget only when the existing control cannot preserve the data or interaction.

## Compatibility gaps and required prototypes

| Gap | Source finding | Required action |
| --- | --- | --- |
| GPU limits | Surface binds 20 sampled textures; Patina requests the default limit of 16. | Negotiate supported limits before device creation. Report unsupported devices clearly. |
| Float filtering | Surface uses filterable `Rgba32Float`; Patina requests no optional device features. | Request supported float filtering and validate it before material startup. Do not silently use a second device. |
| Material precision | `GpuImage` is 8-bit; Surface outputs are 32-bit floats. | Add the dedicated material value types. |
| View output access | `Graph::value` needs a wired output and first requests retention for a later run. | Prove a batch result appears when a view opens, without rerunning export or other side effects. |
| Viewport composition | `ViewportSurface::draw` supplies a color pass without depth; Surface renders MSAA and depth offscreen. | Render to Surface's `View`, then blit through the native viewport with verified color conversion. |
| Multiple viewports | The current surface helper owns one window surface and clears it. | Start with one GPU preview. Do not promise independent simultaneous GPU channel docks. |
| Color ramps | Patina stores up to 32 RGB byte stops; Surface supports 256 float RGBA stops. | Preserve source precision and capacity. Never quantize or truncate an imported ramp silently. |
| Recipe metadata | Patina graph entries reject unknown fields. Views cannot read/write all layout or group metadata. | Store material metadata in supported project resources and add only the necessary transaction APIs. |
| Subgraphs | Surface has typed declared interfaces and pinned sources. | Map supported interfaces to Patina subgraphs; reject unsupported cases before changing the project. |
| Source editing | A uniform edit differs from a shader edit or a port-schema edit. | Keep uniforms cheap, compile shader candidates transactionally, and rebuild wrappers only for changed interfaces. |
| Export | Seven-file conversion includes code in `forge-ui`, and drops opacity. | Extract the non-UI conversion and add explicit opacity output. |

GPU capability changes need an update to spec section 10.2 in their implementation commit.
Output subscriptions and graph transactions need matching spec updates when their behavior changes.
The table records planned work, not changes to those contracts now.

### Recipe transactions

Import into a prepared candidate before changing the current graph.
Validate the PNG, hashes, resources, ports, defaults, cycles, subgraphs and supported definitions.
Retain node identities, group descriptions, source provenance, drafts and exposed controls where representable.
Show a specific compatibility error for unsupported content.

Commit an accepted import as one undoable graph operation.
Keep new immutable resources until no graph or undo entry references them.
Do not overwrite a resource that an older recipe or undo entry still uses.
The current `Graph` view API is not sufficient for that complete transaction.
Start with the existing copper file and known definitions.
Add arbitrary custom definition imports only after wrapper generation and source isolation have tests.
An unknown kernel must produce a compatibility report, not a placeholder material.

Save a Patina project through normal **Save**.
Export a compatible editable PNG through a separate **Export recipe** command.
Do not claim round-trip compatibility for arbitrary Patina nodes with no Surface equivalent.
Never mutate the original imported PNG during editing.

## Performance plan

A 2048 x 2048 three-plane material uses 192 MiB before mipmaps and other resources.
At 4096 x 4096, it uses 768 MiB.
These are storage calculations, not measured runtime memory.

Surface defaults to a 4 GiB held-output budget.
Its native evaluator waits for GPU completion after each 32 recorded nodes.
Those policies should not be copied into Patina's display loop.
Use a conservative explicit budget in the first prototype and measure peak live resources.
An output still owned by a graph slot or view cannot be evicted safely.
Therefore, a pool budget alone is not a hard total-memory limit.
Measure Patina's retained outputs before choosing an eviction strategy or accepting larger resolutions.
Its seven-file export uses bounded GPU read chunks but still collects full CPU planes and the archive.
Measure those allocations before claiming bounded total export memory.

Keep material evaluation out of idle camera redraws.
Reuse pipelines, uniforms, bind groups where valid, thumbnails and render targets.
Refresh thumbnails only when their source revision changes.
Read small images asynchronously through a bounded staging queue.
Do not perform one blocking readback per visible node each frame.

Separate texture-stage resolution from mesh-atlas resolution.
Surface runs rectangular texture declarations at their larger square side.
Changing the graph size alone does not change its mesh-map atlas resolution.
A draft-resolution control must define both behaviors and must not modify authored settings silently.

Measure these cases before claiming speed:

| Case | Record |
| --- | --- |
| Idle graph and camera orbit | Material dispatch count, CPU work, allocations and frame latency |
| One mask or coat parameter edit | Changed kernels, GPU time, preview latency and pipeline compilations |
| One source edit | Link time, compile time, retained last-good result and diagnostics |
| Copper at each supported resolution | Peak memory, texture reuse, dispatches and completion time |
| Export during editing | Captured revision, UI latency, memory, cancellation and file integrity |

Patina's node timer measures CPU submission work, not complete GPU execution.
Report GPU timing separately.
Do not promise shader fusion; spec section 10.4 places it after version 1.

## Implementation milestones

### 1. Prove the GPU and preview boundary

Select a clean Surface revision and record copied file hashes and notices.
Add the required source crates and assets to an isolated prototype.
Use Patina's shared device for Surface's reference session.
Render copper offscreen and compare raw channels with the same recipe in the reference host.
Present that offscreen result in one Patina viewport.

Also prove precise ramp storage and batch output visibility.
Report unsupported GPU features and failed shader diagnostics without losing the last valid image.
Run on Windows without taking desktop focus.

Acceptance: matching material channels, correct presentation, and no separate GPU device.
This milestone proves the bridge, not a completed node workbench.
Compare with identical mesh bytes, UVs, maps, shader hashes and parameters.
Compare viewport pixels only after matching lighting, camera, object pose and display conversion.

### 2. Make copper a native Patina graph

Extract and test the per-kernel execution interface.
Implement the eight kernel kinds used by copper, plus mesh context and required conversions.
Convert its 28 nodes, 42 links and six groups into a normal Patina graph.
Keep the same pinned kernels and numeric parameters for parity.

Add real node previews, useful compact controls and a bound **Preview** dock.
Verify one edit through the node, inspector, dock, undo and saved reopen.
Test **Edit** and **Bake** modes, type sharing, hot reload and headless execution.

Acceptance: copper works as editable native nodes in the **materials** top graph.
Its channels match the reference channels.
Verify the existing **main** graph through the CLI and editor.
Verify the material graph through top-graph selection and CLI `--graph materials`.

### 3. Complete smart material authoring

Add the remaining core operations: **Cells**, **Flow** and **Read**.
Add **Material**, **Split**, **Blend** and **Image** where the contracts fit.
Support reusable smart material subgraphs with explicit interfaces.
Keep interface proxy kernels as import machinery, not confusing entries in the node palette.

Build **Materials**, **Learn**, **Controls**, **Mesh Maps** and **Channels** views.
Add a simple learning graph and a second mesh test.
Expose a small set of meaningful copper controls only after their responses are verified.

Acceptance: the tool can author and inspect a new material, with clear missing-map behavior.

### 4. Complete files and export

Implement transactional recipe import and compatible editable PNG export.
Support the seven-file compatibility preset and explicit opacity export.
Document each file's color encoding and normal convention.
Preserve 16-bit height export and detect values outside the selected file range.
Show progress and cancellation for long work.
Check corrupt hashes, oversized payloads, path escapes and unsupported source interfaces before import.
Test an imported ramp with more than 32 stops and float colors between byte values.

An export must start from an explicit command, not from every material reevaluation.
Use a captured immutable result and write to temporary files before publishing completed output.
The headless route must use the same conversion and validation code.

Acceptance: export, reopen, compare channels, preserve existing files on failure, and move the packaged project to another folder.

### 5. Polish and validate material quality

Verify every channel independently and under changed lighting.
Sweep wear, deposit coverage, seed and scale through useful extremes.
Check UV borders, resolution changes and transfer to a second mesh.
Check node readability, focus, wire routes and the narrow dock layouts.
Measure the performance cases above and set budgets from those results.

Run Patina's required workspace and frontend gates for implementation milestones.
Run the material example's own tests because examples build in separate workspaces.
Commit each tested milestone before starting the next substantial feature.

## Known source limits and open verification

- Surface's wiki says no material has passed its complete channel and cold-test gate.
- Copper's reference match and transfer to a second mesh remain unverified in that source documentation.
- Arbitrary OBJ/glTF import is not implemented in Surface. Start with its supported mesh snapshots.
- The supplied ball mesh cannot be regenerated from Surface alone; its UV production step belongs to the Model tool.
- The normal-frame sign issue is documented in the wiki. The inspected shader still uses its derivative-frame formula.
  Reproduce it before changing shading. Keep parity results separate from any later renderer correction.
- Material emission and anisotropy are absent from the current material channels.
- The current export drops opacity. The proposed opacity output and transparent preview need independent checks.
- The existing test suite can skip GPU tests when no adapter exists. Acceptance must distinguish a skip from a pass.
- Source requirements inspection can identify needed maps, but it is not a substitute for valid target bakes.

**NOT_ENOUGH_INFO:** no Patina/Surface bridge, GPU parity run, normal-sign reproduction,
performance benchmark or Linux/macOS integration ran during this study.

## Source locations for implementation

Surface paths are relative to the `millwright-surface` checkout.

| Concern | Source |
| --- | --- |
| Declarations, ports and parameter packing | `crates/forge-nodes/src/def.rs` and `def/gradient.rs` |
| Graph, resolution and saved controls | `crates/forge-nodes/src/graph.rs` |
| Pinned registries | `crates/forge-nodes/src/registry.rs` |
| Kernel compilation | `crates/forge-nodes/src/engine/kernels.rs` |
| Dispatch, scratch and memory | `crates/forge-nodes/src/engine.rs`, `engine/run.rs` and `gpu.rs` |
| Material algorithms | `crates/forge-nodes/kernels/nodes/core/` and `nodes/material/` |
| Reference host | `crates/forge-runtime/src/lib.rs` |
| Subgraphs and map dependencies | `crates/forge-runtime/src/flatten.rs`, `subgraphs.rs` and `requirements.rs` |
| Texture export target | `crates/forge-runtime/src/map_export.rs` |
| Export conversion and archive | `crates/forge-ui/src/export.rs`, `material_export.rs` and `export_archive.rs` |
| Recipe container | `crates/forge-png/src/lib.rs` and `container.rs` |
| Shading and offscreen view | `crates/forge-render/src/scene.rs` and `shaders/scene.wgsl` |
| Copper authoring source | `tools/wiki/oxidized_metal_graph.py` |

Patina implementation boundaries:

- [GPU context](../crates/patina-gpu/src/lib.rs), [image type](../crates/patina-gpu/src/image.rs) and [viewport](../crates/patina-gpu/src/viewport.rs).
- [View graph access](../crates/patina-sdk/src/graph.rs) and [color ramp](../crates/patina-sdk/src/ui/ramp.rs).
- [Project schema](../crates/patina-engine/src/project.rs) and [package source list](../crates/patina-engine/src/package.rs).
- [GPU contract](spec/09-execution-and-gpu.md), [views contract](spec/15-editor.md) and [shared GUI controls](gui-visual-guide/controls.md).
