# Mockup color and size refresh

- **Status:** done.
- **Date:** 2026-11-14.
- **Request:** Update mockups 4 and 5 for colored headers and different node
  widths and heights. Remove the superseded uncolored main-editor image.
- **Files:** Existing paths replaced:
  [custom node canvas](../../../docs/mockups/mockup-4-custom-node-canvas.png)
  and [custom views](../../../docs/mockups/mockup-5-custom-views.png).
- **Method:** Built-in image generation. One edit call per image; two calls
  total. No extra variants or upscaling. Exact prompts below.
- **Changes:** Colored headers, header-derived selection/focus, neutral bodies,
  and content-driven node sizes. The canvas has a compact Batch Export and
  wide Sample Player. The sound editor has a tiny Mixer, narrow Audio Out,
  larger Filter, and two differently sized players. Spectrum and Timeline
  remain independent docks.
- **Additional corrections:** Removed the incorrect ADSR-to-Sample-Player
  cable in mockup 4. PNG export now shows Compression instead of Quality.
- **Reference cleanup:** Removed the uncolored main-editor image as requested.
  Updated README, AGENTS, spec overview, mockup plan, handoff guide and three
  draft briefs to use the existing colored version. Draft briefs were not
  submitted or otherwise changed.
- **Spec:** Recorded the owner's content-driven node-sizing rule in 4.3.
- **Checks:** Both generated outputs inspected. Mockup 4 is 1672 x 941;
  mockup 5 is 1586 x 992. Copies checked against generated originals by SHA-256.
  Checked obsolete-path references and deletion.
- **Limits:** These are visual references, not working controls. Plot points
  are illustrative. The ADSR plot is not a numerical validation of its fields.
  The new Mixer omits its old runtime footer, and tiny player trim buttons
  use ambiguous glyphs. Exact controls still need implementation design.
- **Git:** No .git directory exists in this workspace. No commit or push.

## Exact prompt: mockup 4

```text
Edit the attached Patina custom-node canvas mockup. Output ONE improved replacement image. Keep its six node identities, meaningful custom controls, fine dark grid, graphite bodies, sharp legible typography and general visual family. Update header colors AND completely remove the uniform 2x3 equal-card layout. Native landscape resolution at least as large as the reference if available; no perspective or explanatory captions.

MOST IMPORTANT: each node has a content-driven different WIDTH AND HEIGHT. No uniform boxes. No equal rows. No padding a small utility node to match a big editor. Arrange nodes freely with ample visible canvas gutters. Retain the actual UI inside each node, not a miniature generic symbol.
Approximate arrangement on a1672x941 canvas:
Transform x45 y65 w450 h410, largest photo-focused body.
Gradient Map x555 y100 w480 h285, deliberately wide and shallower.
Curve Editor x1110 y45 w510 h450, largest plot-focused body.
ADSR Envelope x85 y560 w390 h310, medium.
Sample Player x550 y520 w550 h285, wide waveform, not tall square.
Batch Export x1220 y600 w300 h230, smallest compact utility.
These are guides, not visible labels; give wires and pins space. All six fully on-screen.

Color only the narrow HEADER strips, charcoal bodies:
Transform teal.
Gradient Map berry/magenta.
Curve Editor same berry/magenta.
ADSR Envelope berry/magenta (editable modulation curve).
Sample Player amber (media source).
Batch Export purple (output).
Preserve separate small green checked Ready indicators, small kind and domain icons.
Selected Curve Editor outline and selected handle are LIGHT PINK derived from magenta header, NOT cyan. Focused controls within a colored node use a lighter shade of that node's header. Normal controls neutral gray.
Plots, gradients and photo previews may use data colors.

Transform: large mountain-lake photo with 2D transform handles, move/rotation gizmo. Below compact X120 Y-40 Rot12° Scale1.10 and pivot/aspectlock controls. Input Image, output Image. Header frame-domain icon.
Gradient Map: long editable navy-to-lilac-to-orange-to-cream gradient with stop handles, compact chosen-stop color field #E8A33D, Pos0.62, Linear; a WIDE SHALLOW graded mountain-lake preview beneath. Image input/output. Frame icon. Wire Transform Image -> Gradient Map Image across empty gutter.
Curve Editor: big square-ish curve plot over neutral histogram, RGB R G B tabs, a smooth rising curve with draggable points and tooltip "In0.42, Out0.55", compact In/Outfields Reset. Output Curve. Frameicon. Do not invent a wire into another node.
ADSR Envelope: simple wide ADSR graph and A12ms D180ms S0.65 R420ms fields, Loop and Audition. Audio wave domain icon. Input Gate/output Envelope. No cable connecting Envelope to Sample Player Trigger: those are different types; remove the old erroneous dashed cable entirely. No off-screen dangling cable.
Sample Player: impact_03.wav label, large WIDE waveform with start/end handles and playhead, play button, Pitch+2.0st, start0.00s/end2.40s fields. Audio wave icon. Trigger input and Audio output. No incoming wire from ADSR.
Batch Export: tightly packed Path "out/frame_####.png" and folderbutton, format PNG, Compression6, Overwrite toggle, small Export nowbutton and Last run#1042 line. Event bell icon. No oversized blank areas and no gigantic button spanning half the body.
Run-duration footers stay compact on all nodes, not padded to align neighboring bottoms.

These are examples of different custom node GUIs, so disconnected examples are fine. Keep only the valid Transform->Gradient wire. Preserve high contrast, clean text and all category colors in headers, varied asymmetric geometry and actual controls. This is an update of the existing mockup, not a new app concept.
```

## Exact prompt: mockup 5

```text
Edit the attached Patina "Impact SFX" editor screenshot into ONE improved replacement image. Preserve the complete Windows app, project sidebar, same functional audio graph, Spectrum dock on the right, Timeline dock below and bottom status. Same dark graphite Patina visual family and readable typography. Highest native resolution available, landscape16:10, preserve reference detail. Only intentional changes are better content-driven node dimensions, colored node headers, matching selection/focus colors and matching sidebar swatches. Do not add nodes, panels, or code.

CRITICAL: nodes must NOT all be same size. Deliberately assign distinct silhouettes according to their GUI. The Mixer is a TINY junction; Audio Out is a NARROW compact meter; Filter is a LARGE tall editable graph; Impact Player is a WIDE medium waveform node; Tail Player is a SHORTER, narrower waveform node. Do not render equal cards, equal widths, or equal heights. Show clear gutters and enough canvas to read each. All graph nodes fit above Timeline and left of Spectrum.

Within graph panel, plan roughly:
Impact Player top-left wide 250x180, including filename impact_03.wav, waveform and compact play/trimcontrols.
Tail Player lower-left 210x125, filename tail_air.wav and smaller waveform.
Mixer center about110x100 ONLY pins A/B, one tiny mix control and output Audio. No giant empty space.
Filter center-right about220x255, tallest, a substantial editable frequency-response plot, Cutoff2.4kHz and Q0.8.
Audio Out far-right about130x135, narrow, two horizontal L/R meters and numeric level, no big unused body.
Scale positions to fit. Stagger them vertically, Filter highest/central, Mixer on wire junction, players separated. Different top AND bottom edges. Wires should remain clear and outside bodies.

Header palette consistent across examples:
Impact Player and Tail Player AMBER (media sources).
Mixer and Filter TEAL (audio processing).
Audio Out PURPLE (output).
Use rich restrained narrow colored title strips with light text. Neutral charcoal bodies, gray unselected controls, separate small green ready checks. Filter selected outline and focused control LIGHT TEAL derived from its header, not generic cyan. Small audio-wave domain icons. Runtime footers unchanged, compact.
In Project tree under Nodes, tiny swatches match the node's header. Views folder icons remain neutral.
Keep graph dataflow exactly:
Impact Player Audio -> Mixer A
Tail Player Audio -> Mixer B
Mixer Audio -> Filter Audio
Filter Audio -> Audio Out Audio
All these are same audio domain, solid lightgray wires. NO wires to Spectrum or Timeline. Actual connected edge sockets, no wire crossing any body.

Spectrum stays an independent neutral dock with source "Main Graph / Audio Out", dB-vs-frequency plot20Hz–20kHz, L/R graymeters, FFT4096, Peak hold and Reset. Preserve readable axislabels and paneltabs/floatcloseicons. It has NO pins and NO colored node-like title strip.

Timeline stays an independent neutral dock: transport, time00:00:01.240, Loop and Snap0.1s, ruler0–3s, loop0–2.4s, separate Impact and Tail waveformtracks, editable Filter/Cutoff automation lane500Hz–5kHz. Muted clip tints acceptable; no rainbow decorative background. Preserve playhead, automation handles and correct source filenames. No graph wires here.

Top bar P PATINA, Impact SFX/Main Graph, LIVE, undo/redo,100%,fit/grid,Run,Pause,Bake,Views,Windows buttons. Left Project and Layers tabs, folder groups Graphs Nodes Views Widgets Types. Bottom Logs Output Debug, Ready,0errors,Fused,48kHz. Same professional crisp practical density. Keep all text and panels readable and prioritize the conspicuous node size variations.
```

