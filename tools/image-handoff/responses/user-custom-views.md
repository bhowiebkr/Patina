# Custom editor views

> Superseded by the [color and size refresh](user-mockup-color-size-refresh.md).
> The image at the same path was replaced. The notes below describe the earlier render.

- **Status:** done; owner said the image is good enough.
- **Updated:** 2026-11-14 (America/Toronto).
- **Request:** Custom editor views consistent with mockups 1, 3, and 4.
- **Files:** `docs/mockups/mockup-5-custom-views.png`.
- **Generation:** Built-in image generation, followed by a meter/toggle color edit.
  The owner then instructed Codex to stop making extra images.
- **Checked:** Docked Timeline and Spectrum panels are separate from the node graph.
  Compared the palette, window chrome, node styling, and controls with references
  1, 3, and 4. Actual saved dimensions: 1585 x 992. Verified an exact SHA-256 copy
  of the generated output. No upscaling.
- **Limits:** The requested 3200 x 2000 was not returned. This is a visual mockup;
  graph values and plots are illustrative, not measured application output.
- **Git:** No Git repository is present. No commit or push was possible.

## Owner direction for future images

Generate one image per request. Do not generate additional revisions or variants
unless the owner asks.

The owner supplied another tool's screenshot and highlighted its use of node
color. It uses category-colored headers: purple for Debug, red for Events, gray
for Flow, amber for Inputs, teal for Lists, and magenta for Text. Node bodies stay
neutral. The sidebar repeats category colors. Status uses a separate small
indicator. Treat this as the owner's positive visual reference for future
mockups. An exact Patina category palette has not been selected. The current
spec 15.3 color restrictions will need reconciliation before a new category
palette is treated as settled.

## Original generation prompt

```text
Use case: ui-mockup.
Create the next mockup in this existing Patina series, visually CONSISTENT with all THREE reference images.
Reference 1: main editor. Preserve its Windows application frame, Patina P logo, header proportions, typography, panel dividers, charcoal palette, controls and layout density.
Reference 2: error view. Additional reference for the SAME theme and graph/node construction; do not reproduce its obsolete layers, error states or Compare tab.
Reference 3: custom node canvas. Preserve its detailed custom-control language, light-gray controls, original square-with-dot node-kind icons, audio sine-wave icons, green checked Ready indicators, neutral pins/wires and restrained cyan selected-node border.
These are three views of ONE application. Do not redesign the branding, invent a different color theme, use a futuristic style, or turn it into a web dashboard.

PRIMARY REQUEST: A polished Windows desktop editor screenshot illustrating PROJECT-DEFINED CUSTOM VIEWS integrated into the actual Patina editor, independently from the node graph. This is a sound-design project with a custom Timeline panel and a custom Spectrum panel. The two custom views are true docked editor panels: each has a tab header, straight panel edges and its own controls; neither has graph pins, wires, rounded node-card framing, a clock-domain badge or a node runtime footer. The node graph remains visible alongside them. The image must make their relationship immediately understandable.

OUTPUT: landscape 3200 x 2000 native resolution if supported, otherwise the largest native landscape size. Crisp legible text, fine waveform and spectrum detail. No upscaling. Flat orthographic application screenshot, no desktop wallpaper, monitor, perspective, presentation title, callouts, explanatory arrows or watermark. One complete Windows application window fills the image, with minimize/maximize/close caption buttons at top right.

LAYOUT:
A fixed full-width top toolbar, a narrow left Project sidebar (about 14% width), and three main docked areas:
- Upper middle (about 56% total width, 52% window height): the Main Graph tab and canvas.
- Upper right (about 30% total width, same height): the Spectrum custom view, docked alongside the graph.
- Lower workspace (the full width to the right of the Project sidebar, about 37% window height): the Timeline custom view, stretching beneath BOTH graph and spectrum.
A slim collapsed bottom panel has Logs, Output, Debug tabs. A thin status bar runs across the bottom. Clean aligned splitters. No floating window is necessary in this mockup; show small detach icons on custom-view tabs so they read as dockable panels.

TOP BAR: preserve the reference Patina logo and PATINA wordmark. Project path "Impact SFX / Main Graph". Green dot and "LIVE". Undo, redo, "100%", fit and grid icons. Buttons "Run", "Pause", "Bake", and "Views". Keep controls compact and consistent with reference 1. Bake subdued because this is a fused running graph and no code editor is open. No code inspector is visible.

LEFT SIDEBAR: tabs "Project" and "Layers", Project active. A crisp tree:
Impact SFX
  Graphs
    Main Graph
  Nodes
    Impact Player
    Tail Player
    Mixer
    Filter
    Audio Out
  Views
    Timeline
    Spectrum
  Widgets
  Types
Use neutral folder and item icons. Views expanded so the project-defined panels clearly come from the project. No colored layer rows, layer toggles, plus button or orphan-edit warnings.

UPPER MIDDLE NODE GRAPH:
Tab "Main Graph", inactive adjacent tab "Inspector". Dark subtle square grid, matching references. Five compact nodes with comfortable wire spacing, not gigantic controls.
Two source nodes stacked on the left:
"Impact Player" with small gray waveform and filename "impact_03.wav", Audio output.
"Tail Player" with smaller decaying gray waveform and filename "tail_air.wav", Audio output.
Both feed "Mixer" in the middle, inputs "A", "B", output "Audio".
Mixer feeds selected "Filter" on its right. Filter has a thin cyan-blue selection border, input "Audio", output "Audio", a small custom frequency-response curve and neutral controls "Cutoff 2.4 kHz", "Q 0.8".
Filter feeds "Audio Out" at far right with input "Audio" and small neutral L/R meters.
All wires are neutral gray solid lines. Use no dashed wires: these are all audio-domain nodes.
Each node header has the original appropriate node-kind icon, sine-wave audio domain icon and a checked green Ready dot. Each footer has a separately labeled runtime, e.g. "Run 0.04 ms". Node visual construction and small controls match reference 3 exactly. Fewer, clearer labels are preferable to microscopic detail.

UPPER RIGHT CUSTOM SPECTRUM VIEW:
A real dock tab "Spectrum", with subtle detach and close icons. Below it a compact binding row "Source" and dropdown "Main Graph / Audio Out".
Large detailed frequency spectrum plot on an inset charcoal plotting area. Logarithmic frequency labels "20", "100", "1k", "10k", "20k Hz"; vertical amplitude axis from "0" down to "-80 dB". A thin ivory live-spectrum line with a strong low-frequency peak and smaller jagged decaying high-frequency peaks. A faint gray filled area underneath and a subtle darker peak-hold trace. Fine muted gray grid.
At the bottom of this panel: a small "FFT 4096" dropdown, neutral "Peak hold" toggle, and a "Reset" button. Add two thin neutral L/R output meters along the right edge within this panel if space permits. This panel has no sockets and no node header.

LOWER CUSTOM TIMELINE VIEW:
A real dock tab "Timeline", small detach and close icons. This pane is clearly separate from the graph and spans both upper columns.
Compact toolbar: play triangle, pause, return-to-start icon, timecode "00:00:01.240", "Loop" toggle and "Snap" dropdown. Neutral gray control language from reference 3.
A horizontal timeline ruler labeled "0.0", "0.5", "1.0", "1.5", "2.0", "2.5", "3.0 s". A white vertical playhead at 1.24 s runs through every track. Thin light-gray loop brace spans 0 to 2.4 s.
Three clearly labeled tracks in a fixed narrow track-header column:
1. "Impact" with mute/solo controls; one waveform clip named "impact_03.wav" starting at zero and decaying toward 1.5 s.
2. "Tail" with mute/solo controls; a softer long waveform clip named "tail_air.wav" starting at 0.2 s and fading by 2.4 s.
3. "Filter / Cutoff" with a continuous editable automation curve, several diamond control points, and a small value label "2.4 kHz".
Clip backgrounds are muted slate, waveforms light gray. Different clips can use subtle desaturated data tints but avoid bright decorative colors. The automation line is ivory with neutral handles. The automation track visibly refers to the selected Filter node, without drawing a wire from the panel into the graph. Keep enough detail to show this is a custom editor, with proper trim handles, grid lines and clipped waveform content.

BOTTOM: collapsed Logs, Output, Debug tabs only, no Compare. Status bar shows green checked Ready indicator, "0 errors", "Project: Impact SFX", "Fused", and optional separate audio info "48 kHz". Do not mix compilation time with node execution time.

VISUAL RULES: near-black graphite canvas, slate panels, subtle thin gray borders, restrained small corner radii only on buttons/nodes, the exact white/light gray font hierarchy of references. Green for state indicators only; no decorative green wires. Cyan-blue only for selected node/wire state. Neutral controls, no Rust logos. Ordinary readable user-facing labels only: no Rust source, DLL labels, implementation annotations, or "custom view" explanatory badges. Prioritize the two substantial custom editor panels, with graph context above, and consistency across the existing Patina mockups.
```

## Color-edit prompt

```text
Edit this existing Patina screenshot with ONLY these small color corrections. Preserve the entire layout, all text, every position, all fonts, all icons, node and panel sizes, graph wires, waveform and spectrum data, timeline curves, image dimensions and sharpness exactly.

1. In the Audio Out node, change the TWO bright green horizontal L/R meter fill bars to neutral light gray (#c1c7ce). Preserve their lengths and gray tracks.
2. In the Spectrum panel at far right, change the TWO bright green vertical L/R meter fill bars to the SAME neutral light gray (#c1c7ce). Preserve their heights and gray tracks.
3. In the Timeline toolbar, change the cyan-blue background of the ON Loop toggle to neutral light gray, preserving its ON position and darker circular knob. Match the neutral toggle style of Patina's custom-node canvas.

Do not change green Ready/check status indicators or the green LIVE status dot. Do not change the cyan selection outline around Filter, the blue Run button or active-tab indicators. No other modifications. The intended output should be pixel-for-pixel visually identical apart from those four meter fills and one toggle fill. Keep the exact input resolution; do not resize or upscale.
```
