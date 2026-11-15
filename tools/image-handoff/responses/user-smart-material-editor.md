# Smart material shader editor

- **Status:** done.
- **Date:** 2026-11-14.
- **Request:** Make a smart material shader node editor inspired by the owner's
  attached Millwrite oxidized-copper graph.
- **File:** [mockup-9-smart-material-editor.png](../../../docs/mockups/mockup-9-smart-material-editor.png).
- **Generation:** Built-in image generation, one call with the attached screenshot
  as reference. No extra variants.
- **Checked:** Seven differently sized nodes, colored headers, selected mask
  controls, material layer GUI and independent preview dock are visible.
  Saved PNG is 1586 x 992. SHA-256 matches the generated original.
- **Scope:** Visual proposal. No shader, camera interaction, baking, map export
  or material application was implemented or tested.
- **Git:** No .git directory exists. No commit or push.

## Proposed custom nodes

| Node | Custom GUI |
|---|---|
| Mesh Maps | Asset selector and curvature/AO image thumbnails |
| Fractal Noise | Flat noise preview, scale, detail, seed and projection |
| Wear Curve | Editable remap curve, histogram and wear-mask thumbnail |
| Patina Mask | Mask image, remap handles, coverage, contrast and cavity bias |
| Copper Base | Material swatch, image preview, metallic and roughness |
| Material Layers | Ordered material stack, visibility, amounts and expanded layer controls |
| Material Output | Compact PBR sink and channel disclosure |

The layer stack describes the material. It is not Patina's base/slot compilation
system. Layer parameters must affect generated material data, not just a panel.

The **Material Preview** is a separate custom view. Its 3D viewport shows the
selected material on a reference ball or current asset. Shared controls bind
to graph parameters. Channel thumbnails, lighting, map export and material
application are proposed GUI actions. They are not working integrations.

Node-body previews are image widgets. The interactive GPU viewport belongs in
the dock, consistent with spec 4.3 and 15.8. These project nodes could generate
shader data/source for a runtime compiler, as permitted by spec 10; this image
does not prescribe the compiler architecture or a finalized material type.

## Intended connections

- Mesh Maps Curvature -> Wear Curve Curvature.
- Mesh Maps AO -> Patina Mask AO.
- Fractal Noise Noise -> Patina Mask Noise.
- Copper Base Material -> Material Layers Base.
- Wear Curve Wear -> Material Layers Wear mask.
- Patina Mask Mask -> Material Layers Patina mask.
- Material Layers Material -> Material Output Material.

The preview binds to Material Output without a graph cable.

## Limits in the generated image

- The Wear Curve cable lands near the Verdigris row instead of the **Wear mask**
  socket. The intended connection is listed above.
- Header kind/domain icons and runtime labels are omitted or simplified.
- The generator substituted a simple amber P mark for the established Patina
  logo. The existing logo remains the implementation reference.
- Material appearance, masks and channel thumbnails are illustrative. They were
  not rendered from the depicted graph and are not a shader validation.
- Larger native output was requested but not returned. No upscaling.

## Exact generation prompt

```text
Create ONE polished Patina Windows desktop smart-material shader node editor mockup inspired by the user's attached Millwrite oxidized-copper material graph. This is a new Patina mockup, NOT a copy of Millwrite's tiny text or layout. Preserve the material idea: warm copper, dark oxidation, green-blue mineral patina in recesses, exposed polished copper edges, and a grooved shader-ball reference object. Make the node graph understandable and the custom GUIs readable.

Output a full app screenshot, landscape approximately16:10 at the largest native resolution available, ideally3200x2000. No perspective, monitor, captions or explanatory poster. Dark blue-black graphite Patina window, fine square graph grid, crisp SegoeUI-like text, small corner radii, gray bodies, narrow colored node title strips. P logo PATINA upper left, Windows caption buttons upper right. Match the established Patina editor family. Top bar "Oxidized Copper / Material Graph", LIVE, undo/redo,100%,Fit,Run,Pause,Views. Collapsed40px left toolrail. Bottom slim Logs/status "Ready · 0 errors · Illustrative material". No code inspector or large project tree.

USER'S STRONG REQUIREMENT: never make all nodes the same size. Every node's width AND height must fit its content. Mix a TINY output, SMALL mesh input, MEDIUM noise preview and copper controls, a WIDE curve editor, a LARGER mask composer and a TALL material-layer editor. Do not arrange equal-size cards in rows. Do not wrap every node in identical group rectangles. No giant disconnected full-window diagram. Prioritize visible useful controls over dozens of miniature uniform thumbnails.

Layout: graph occupies LEFT76% of window; independent "Material Preview" dock RIGHT24%, full usable height. The dock has a neutral tab header and float/close buttons, NO graph sockets and NO cables crossing its boundary. All graph nodes fully visible with clear gutters and routed connections. About7 detailed nodes, not25 unreadable miniatures.
Use this approximate composition on a1600x1000 logical canvas, scale proportionally:
Mesh Maps x65 y165 w165 h185.
Fractal Noise x65 y505 w195 h240.
Wear Curve x295 y105 w270 h265.
Patina Mask x315 y475 w280 h310.
Copper Base x655 y125 w205 h230.
Material Layers x690 y420 w310 h420.
Material Output x1050 y510 w150 h150.
Right dock starts x1230.
Deliberately uneven tops and bottoms, asymmetrical sizes and open routing gutters. The Material Layers is substantially larger than Mesh Maps; Output is much smaller than Noise. Keep the names and all essential labels legible at the chosen resolution.

CATEGORY COLORS:
Mesh Maps amber (source).
Fractal Noise amber (source).
Wear Curve magenta (remap).
Patina Mask teal (processing).
Copper Base copper-orange (material).
Material Layers copper-orange (material).
Material Output purple (output).
Keep small separate green ready checkdots. Selected Patina Mask has a LIGHT TEAL border and focused handles derived from its header. Do not color neutral viewheaders. All graph connections solid lightgray, small direction arrows and labeled sockets; the same frame domain throughout. Pincolors neutral. Preview images are image widgets, NOT interactive3Dviewports inside node bodies; only the right dock contains the3Dviewport.

MESH MAPS small:
"Source Current asset" dropdown.
Two small grayscale image thumbnails labeled Curvature and AO, clearly actual mesh-driven mask previews on the grooved ball.
Right outputs Curvature and AO.
No giant empty body.

FRACTAL NOISE medium:
Large square black-white mottled fractal texture thumbnail.
"Scale 18.0", "Detail 5", "Seed 42" smallfields.
"Triplanar" dropdown.
Output Noise. Don't use the same sphericalpreview as everyothernode; this is a flat texture sample.

WEAR CURVE wide:
Input Curvature, output Wear.
An editable rising S-curve with histogram beneath it, controlpoints and a lightpink selectedhandle.
Small two-tone mask thumbnail at one side, with bright worn edges and dark faces.
"Low 0.35", "High 0.72", "Invert" checkbox.
Custom curve GUI must dominate its footprint.

PATINA MASK larger:
Inputs AO and Noise, output Mask.
Large grayscale texture/ball MASK preview with white mottled recess coverage and mostly black exposed edges.
Below preview, a black-to-white remap ramp with2handles, "Coverage 0.62" slider, "Contrast 0.74", "Cavity bias 0.80".
"Multiply cavities" selected.
A small expand icon in header.
This node's active controls should look like a tailored material mask authoringtool.

COPPER BASE medium compact:
A small polished-copper sample sphere beside materialcolor swatch #B76E42.
"Metallic 1.00", "Roughness 0.28".
A thin roughnessslider. Output Material.
Do not enlarge to match maskeditor.

MATERIAL LAYERS tall, rich customGUI:
Left inputs Base, Patina mask, Wear mask, clearly distinct and named.
An ordered layerstack inside one node, not separate fake nodes:
top row "Edge polish" with draghandle, eyeicon and amount0.35
middle row "Verdigris" with turquoise swatch, amount0.62
next row "Dark oxide" with darkbrown swatch, amount0.45
bottom row "Copper" with copper swatch, Base label
Small state note "Top layer overrides below".
Expanded Verdigris layer settings beneath:
a SHORT deep-teal-to-mint custom colorramp with editable stops
"Roughness 0.78", "Metallic 0.00", "Height 0.012".
Small materialthumbnail or swatch-strip showing copper, oxide,patina.
Right output Material.
This custom layer stack is material content, not obsolete Patina compilation layers. Keep allrows readable.
Do not use a GPUviewport in this nodebody.

MATERIAL OUTPUT tiny:
Input Material.
Small labels "PBR" and "Preview bound".
Compact "Channels ▸" disclosure.
No large redundant sphere, no bulky controls. Size like a small sinknode.

EXACT DATAFLOW:
Mesh Maps Curvature -> Wear Curve Curvature.
Mesh Maps AO -> Patina Mask AO, route downward through left gutter.
Fractal Noise Noise -> Patina Mask Noise.
Copper Base Material -> Material Layers Base.
Wear Curve Wear -> Material Layers Wear mask, route through empty central gutter.
Patina Mask Mask -> Material Layers Patina mask.
Material Layers Material -> Material Output Material.
Seven wires only, no stray wires, no crossing nodebodies. Use curves and empty gutters; don't add cables into the3Dpreview. Grouping is conveyed by spacing, not large boxes. The graph reads sources on left, mask generation in center, material composition on right.

RIGHT DOCK "Material Preview":
Source dropdown "Material Output".
Tabs "Reference ball" selected and "Current asset".
Large photoreal PBR shaderball with concentric grooves, angled threequarter view, occupying the top half of dock on a dark neutral studio background. Warm rich copper base with fine grain, irregular darkbrown oxidation, pale green/turquoise patina concentrated in grooves and crevices, clean gleaming copper edgewear along ridges. Fine realistic roughness and height details, not a smooth turquoise-painted stripe. Model inspired by attached Millwrite ball with stepped circular grooves. Warm rimlight and soft neutral keylight make materialreadable. Grounding shadow, no pedestal text.
Small orbit/reset/viewcube controls.
Below viewport, neutral tabbed controls "Material | Lighting | Channels", Material selected:
"Patina coverage 0.62" slider.
"Edge wear 0.35" slider.
"Surface scale 18.0" field.
"Studio environment" dropdown and exposure0.0 compactrow.
A row of small map thumbnails labeled Base color, Roughness, Metallic, Normal.
Buttons "Export maps…" and "Apply material".
These are project-defined view controls bound to graphsettings, not claims of implemented features.
No source code, no branded otherapps, no decorative rainbowpanels, no equal-size nodes, no unreadable miniature fullgraph. One high-quality screenshot with an attractive convincing material result AND an easy-to-follow graph.
```

