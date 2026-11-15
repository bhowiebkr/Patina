# Custom node canvas

> Superseded by the [color and size refresh](user-mockup-color-size-refresh.md).
> The image at the same path was replaced. The notes below describe the earlier render.

- **Status:** done; the owner accepted the first render.
- **Updated:** 2026-11-14 (America/Toronto).
- **Request:** Direct owner request: a canvas view of nodes with custom GUIs.
- **Files:** `docs/mockups/mockup-4-custom-node-canvas.png`.
- **Generation:** Built-in image generation, using the two original mockups as references.
- **References:** Original mockups and Claude's draft 0002 gallery brief. The widget
  kit does not exist yet; this image uses the original mockup style directly.
- **Checked:** Six complete nodes with transform, gradient, tone curve, ADSR,
  waveform, and export GUIs. Actual PNG dimensions: 1672 x 941. The workspace file
  is an exact SHA-256-verified copy of the generated image. No upscaling.
- **Limits:** Native 3840 x 2160 was requested but not returned. The dashed wire
  incorrectly connects ADSR Envelope to Sample Player Trigger. Treat the image as
  a GUI appearance reference, not an approved graph or wiring example.
- **Owner direction:** The first image is good enough. Keep it. A second generation
  had started; its waiting execution was terminated. Backend cancellation was not
  verified, and no second output was selected.
- **Git:** No Git repository in this folder. No commit or push was possible.
- **Next:** Claude can use this image as a node GUI reference. The existing draft
  requests remain unchanged; this does not claim completion of the widget kit.

## Exact prompt used for the accepted image

```text
Use case: ui-mockup.
Create a high-fidelity canvas-only visual variant of the supplied Patina editor references. The references establish the charcoal palette, node borders, compact headers, crisp typography, input/output pins and subtle grid. Replace their central graph with six large, richly detailed nodes containing custom GUIs. Remove all application chrome, side panels, code inspector, logs and status bar. The result is a close-up node canvas, not a screenshot of the entire application, not a poster, and not a contact sheet with separate frames.

OUTPUT: request a native 3840 x 2160 pixel PNG, 16:9 landscape, maximum sharp detail. Do not upscale a lower-resolution image. Prioritize readable node labels, values, handles and plotting axes. Flat orthographic UI render, absolutely no perspective or physical monitor. Fill almost the whole image with the canvas and node bodies; small margins. Six complete nodes in two rows of three, generous useful width and readable text, with tidy curved wires routed in the gutters and never across widgets. All six are fully visible. Nodes should feel like instruments embedded in the actual graph, with shared header/pin/footer construction, not generic dashboard cards. Dark subtle square grid, very dark graphite canvas, slightly lighter slate node bodies, understated thin gray borders, tiny radius, restrained soft shadows, white and light gray text, neutral gray controls. Match the reference's density and professionalism but make nodes substantially larger to expose detail.

COMMON NODE FRAME: each node has a header with a small original node-kind symbol (square with a dot for stateful; lightning for trigger), exact title, a small clock-domain symbol (film frame for the top row, sine wave for ADSR and Sample Player, bell for Batch Export), then green Ready dot with a small check. Do not use Rust logos. Named input pins appear along the left edge in a distinct row just below the header, outputs on the right. Pin labels must stay clear of controls. Pins and wires are neutral light gray. Footer contains a separate clearly labeled execution time, e.g. "Run 0.08 ms"; no fake build/runtime combined labels. State green appears only in readiness indicators. No red or yellow widget chrome. Color is permitted inside image and gradient data. The sole selection accent is a restrained cyan-blue border around Curve Editor. Other controls are grayscale, including sliders, play buttons and handles.

TOP LEFT: exact title "Transform". Input "Image", output "Image". Large landscape thumbnail of an alpine mountain lake, similar to reference imagery, with a visible transform gizmo overlaid: two perpendicular move arrows, a thin rotate ring and a tilted rectangular bounding box with corner scale handles. Use neutral white gizmo strokes for readability. Below the image, four clean numeric fields: "X 120", "Y -40", "Rot 12°", "Scale 1.10". Include compact pivot-point control and an "Aspect lock" checkbox if there is room. Footer "Run 0.08 ms".

TOP CENTER: exact title "Gradient Map". Input "Image", output "Image". A large custom gradient editor, not merely a slider: dark blue through muted violet into warm amber and ivory, with four clearly visible draggable stop handles. Below, a warm amber color swatch, hex entry "#E8A33D", field "Pos 0.62", and a small "Linear" interpolation dropdown. Then a landscape preview of the same mountain lake remapped using the gradient. The gradient strip and image are the colored content. Footer "Run 0.12 ms".

TOP RIGHT: exact title "Curve Editor". Output "Curve". Selected node with a subtle cyan-blue outline. Only this header also has a small expand icon (two outward corners). A large custom tone-curve plot fills most of the body: faint histogram, thin plotting grid, understated numeric axes from 0 to 1, a diagonal reference line and a smooth rising S-curve with five round editable control points. Above the plot, segmented tabs "RGB", "R", "G", "B" in neutral chrome. One handle has a compact readable tooltip "In 0.42, Out 0.55". Under the plot, two number fields "In 0.42" and "Out 0.55", plus a small "Reset" button. Footer "Run 0.02 ms".

BOTTOM LEFT: exact title "ADSR Envelope". Input "Gate", output "Envelope". A large envelope editing canvas with a sharp attack, a curved decay, sustained plateau and smooth release to zero. Four clear draggable handles, vertical amplitude axis 0 to 1, horizontal time axis marked 0, 200, 400, 600, 800 ms. Light gray line and muted translucent gray area below it, no green/yellow/red plotting colors. Below, four aligned number fields "A 12 ms", "D 180 ms", "S 0.65", "R 420 ms". Then an off "Loop" toggle and a small "Audition" button. Footer "Run 0.01 ms".

BOTTOM CENTER: exact title "Sample Player". Input "Trigger", output "Audio". Label "impact_03.wav". A wide, detailed light-gray audio waveform on a darker inset background, with a strong initial transient tapering into textured decay, selection region, trim handles at both ends and a visible white vertical playhead. Tick-marked timeline below. Under that, a small play triangle button, "Pitch +2.0 st" slider with its exact value, and compact "Start 0.00 s" / "End 2.40 s" fields. Footer "Run 0.04 ms".

BOTTOM RIGHT: exact title "Batch Export". Trigger-kind icon and event-domain bell, no input pins, output "Run". A bespoke export form inside the node: labeled path field with exact text "out/frame_####.png", small folder-picker icon, "PNG" dropdown, "Quality 92" numeric field, "Overwrite" off toggle, a wide neutral light-gray "Export now" button. Small label "Last run #1042, 2.4 s" and a thin neutral progress track. Footer "Run 2.4 s". This should clearly be an embedded node GUI, not a separate floating panel.

WIRES: one clean solid curved wire from Transform Image output to Gradient Map Image input. Other output pins have short neatly routed solid wire stubs traveling into nearby canvas gutters. Exactly one dashed wire enters Sample Player Trigger input from the left canvas edge, routed through the gutter between rows before curving to its input; the dashed line denotes a cross-domain connection. Avoid wires through text or GUI content. Do not invent extra node cards or impossible connections between image/audio types.

No window title, no sidebar, no toolbar, no explanatory captions outside the nodes, no layer toggles, no Rust logos, no watermark, no blueprint labels. Clear realistic application typography, fine strokes, beautiful detailed custom controls, native high-resolution output.
```
