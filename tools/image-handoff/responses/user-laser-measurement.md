# Laser measurement mockups

- **Status:** done.
- **Updated:** 2026-11-14.
- **Request:** Review the owner's laser-level-webcam README and images. Propose
  custom nodes and GUI elements. Size nodes to their contents. Multiple images
  were explicitly permitted.
- **Deliverables:** [Acquisition](../../../docs/mockups/mockup-7-laser-acquisition.png),
  [Survey](../../../docs/mockups/mockup-8-laser-survey.png),
  [node and view design](../../../docs/laser-measurement-design.md).
- **Source:** https://github.com/bhowiebkr/laser-level-webcam
- **Research:** Current README read using GitHub API; inspected four current
  workflow screenshots and two legacy screenshots. No external project code ran.
- **Generation:** Built-in image generation, exactly two calls. One acquisition
  view and one complementary survey view. No refinement variants.
- **References:** Acquisition used Patina's accepted colored-header image and
  LaserVision's setup screenshot. Survey used the generated acquisition image,
  LaserVision's measurement screenshot and its legacy surface-plot screenshot.
- **Verified:** Both deliverables are 1586 x 992. SHA-256 matches the original
  generated files. Visually inspected both tool outputs. Different node
  footprints and separate survey docks are present.
- **Limits:** Larger native dimensions requested but not returned. No upscale.
  Illustrative data; no camera, engine, survey or file-export integration.
  Known generated wiring, zero-station and plot inconsistencies are listed in
  the design document. That document defines the intended connections.
- **Git:** This workspace has no .git directory. No commit or push was possible.
- **Cleanup:** Downloaded research images were temporary; original generated
  files remain in the image tool's output directory.

## Exact acquisition prompt

```text
Create ONE polished, highly legible Patina Windows desktop editor screenshot mockup: "Laser Measurement / Acquisition". Use reference 1 for Patina visual styling and reference 2 for the laser camera scientific content. New layout, not a recreation of the reference geometry. Highest available native resolution, landscape 16:10, aim 3200x2000. Full-bleed app window, no perspective, no monitor.

CRITICAL USER REQUIREMENT: nodes have dramatically DIFFERENT widths AND heights fitted to their custom contents. NO equal-card grid, NO evenly sized dashboard cards, NO two rows of three. The Beam Finder is a dominant large scientific instrument; Orientation is a tiny horizontal utility; Camera is tall and narrow; Sensor Calibration is a compact settings card; Calibrated Reading is a small readout; Warm-up Monitor is a wide plot node. All six must be recognizably different silhouettes. Leave clear gutters for wires and varied whitespace, an organic practical left-to-right node canvas.

Style: graphite blue-black Patina app, fine square grid, Segoe UI-like crisp white/lightgray text, small corner radii, thin gray borders. Narrow category-colored node headers: Camera amber, Orientation and Beam Finder teal, Sensor Calibration and Calibrated Reading magenta, Warm-up Monitor purple. Header color separate from small green ready-dot. Beam Finder selected with a LIGHTER TEAL outline (not generic cyan). Node bodies neutral charcoal. Gray controls. Data plots may use semantic colors. Small node kind/domain icons. Named round left input and right output pins, solid lightgray curved wires, event-domain camera-frame pipeline throughout. No source code inspector.

Top narrow bar: P logo PATINA, "Laser Measurement / Acquisition", green LIVE dot, undo redo, 100%, Fit, Run, Pause, Views, Windows caption buttons. Below: tabs "Acquisition" active, "Survey". No project tree taking room; collapsed left icon rail only. Bottom slim status: "Illustrative data", "USB Camera · 1920 × 1080 · YUYV · 5 fps", "0 errors".

Layout guidelines on a 1600x1000 logical canvas, scale proportionally:
Camera x70 y175 width260 height390.
Orientation x385 y195 width175 height140.
Beam Finder x620 y140 width490 height570.
Sensor Calibration x365 y510 width210 height230.
Calibrated Reading x1190 y180 width280 height190.
Warm-up Monitor x1160 y470 width360 height300.
Keep all nodes visible, wires outside node bodies. Not a boxed infographic: real freely placed connected node graph. Header title sizes are consistent; node body dimensions are not.

CAMERA, amber, tall narrow:
Dropdown "USB Camera", tiny Rescan.
"Frames from" The camera / Windows, The camera selected.
"YUYV" dropdown; "1920 × 1080"; "5 fps".
Button "Lock for measuring".
Exposure slider "-9 · 1.95 ms", "Manual".
Mains 50 / 60 Hz, 60 selected.
Small grayscale histogram with amber peak marker and red clip line.
Footer "Peak 196 / 255", "No clipping". Output Frame right.
Do not claim truly raw Bayer; use YUYV.

ORIENTATION, teal, tiny:
Four icon buttons rotate left/right, flip H/V.
"90° clockwise", checked "Lift → line down".
Small simple sensor axes glyph. Input Frame left, output Frame right.
This node is under HALF Camera's height.

BEAM FINDER, teal, largest:
Input Frame, output Beam.
Top readings "Centre 960.18 px", "FWHM 11.6 px", "15 / 16 strips".
A large embedded two-dimensional scientific canvas split between grayscale sensor image left and intensity profile right. Sensor shows a thin bright slightly tilted HORIZONTAL laser line on almost black background, a cyan rectangular ROI with drag handles, 16 vertical strip divisions, tiny accepted centre markers, one red rejected strip marker. Profile on the right is ROTATED: same vertical sensor coordinate, intensity increases to the right, one narrow sideways peak precisely level with the beam. Orange fit-window band, blue background zones, white FWHM marker. Crisp measurement overlay, not scenery or an external camera photo.
Below image: labeled fields "Strips 16", "Fit ±2.5 σ", "Background 4 σ", "Least signal 10".
Small ROI fields "x 120   y 840   w 840   h 240".
"15 accepted · 1 rejected", button "Full frame".
Footnote "Drag to set search area", expand icon in header.
Give this embedded image real generous space. This node is an image/2D-canvas GUI, no 3D viewport.

SENSOR CALIBRATION, magenta, small:
"Sensor profile" dropdown "USB sensor".
"Pixel pitch 3.000 µm".
"Full sensor 1920 × 1080".
"Smaller modes" Scaled / Cropped, Scaled selected.
Tiny sensor rectangle dimension glyph and "Effective 3.000 µm/px".
No input. Output Scale.

CALIBRATED READING, magenta, short wide:
Two left inputs Beam and Scale.
Prominent "2880.54 µm", subordinate "within-frame ±0.08 µm".
Small "Tilt 0.6 mm/m", "Valid" with status icon.
Output Reading.
Don't label absolute beam position as surface height.

WARM-UP MONITOR, purple, wide medium:
Input Reading.
"Ready to measure" green small check.
Wide little drift-over-time line chart, line decays and flattens, labeled minutes on x and µm on y.
"Planned run 10 min", "Allowed drift 1.0 µm".
"Expected drift 0.4 µm", "Noise / 1 s 0.12 µm".
Button "Restart estimate".
Output Stability.
This is a plot-oriented wide node, not a duplicate of the camera settings card.

Connections EXACTLY:
Camera Frame → Orientation Frame → Beam Finder Frame → Calibrated Reading Beam.
Sensor Calibration Scale → Calibrated Reading Scale; route its long wire through empty bottom gutter around Beam Finder, never across its body.
Calibrated Reading Reading → Warm-up Monitor Reading.
Subtle arrowheads clarify directions; actual pin-to-pin endpoint connections. No wire through text or controls. No fictitious links to app panels.

This is a proposal for user-defined Patina nodes based on LaserVision's modern Rust workflow. No LinuxCNC controls in this screen. Prioritize varied node sizes, readable custom GUI controls, clear dataflow and accurate units. Do not introduce extra nodes, large explanatory annotations, marketing copy, or uniform equal boxes.
```

## Exact survey prompt

```text
Create ONE detailed Patina Windows desktop editor screenshot, "Laser Measurement / Survey". This is the SECOND complementary view of the same project as reference 1, not a revision of it. Reference 1 establishes EXACT Patina dark colors, small header style, typography, chrome, fine grid and custom node appearance. Reference 2 shows the source application's station measurement UI. Reference 3 shows an older surface visualization; use it only to inspire a modern scientific surface view, NOT LinuxCNC branding or remote machine controls. Highest native resolution available, ideally 3200x2000, landscape16:10. Flat screenshot, no perspective.

User's priority: arbitrary content-driven NODE SIZES, with SMALL utility nodes and LARGER rich interactive nodes mixed together. Not uniform cards, not a regular grid. Also show custom docked GUI VIEWS outside the nodes: these are real editor panes with tab bars, splitters, close/float icons, and NO graph pins. The result should look like a practical custom metrology application built inside Patina.

Window same as reference 1: P PATINA, "Laser Measurement / Survey", LIVE, undo/redo,100%, Fit,Run,Pause,Views, Windows caption. Tabs Acquisition / Survey (Survey active). Collapsed left icon rail. Status "Illustrative data · Plate A" and "0 errors". All measurements are invented examples.

COMPOSITION at1600x1000 logical scale:
Main graph occupies LEFT approximately 62% of width from below toolbar to y640.
Independent "Surface Results" dock occupies RIGHT approximately38% from toolbar to bottom status.
Independent "Measurement Console" dock occupies BOTTOM LEFT below y650.
Visible splitters, no overlap. Don't put a rainbow header on the dock panes; only graph nodes have colored headers. No code inspector or files tree.
Nodes on upper-left graph have irregular positions and dramatically different footprints:
Station Plan x75 y150 width230 height200;
Sample Window x370 y175 width255 height250;
Session x700 y190 width190 height135;
Drift Correction x115 y475 width210 height130;
Fit Surface x405 y475 width225 height150;
Export x745 y500 width160 height95.
Graph input tags at the top edge of canvas: "Reading" and "Stability" from Acquisition; simple small pill ports, NOT extra giant nodes.
Allow room for wires between nodes and through margins. All remain within graph boundary, never wires into docked views. Every node footer not necessary.

STATION PLAN — amber header, medium card:
"Across a surface" dropdown.
"Grid 5 × 5", "Spacing 100 mm".
A compact clickable 5x5 station dot-map, numbered corners, the origin marked "Zero".
"25 stations · 400 × 400 mm".
Output Plan on right.

SAMPLE WINDOW — teal header, tallest graph node:
Left input Reading, right output Sample.
A mini strip chart of recent samples, gray trace and a few red excluded dots.
"At least 2 s", "At most 10 s".
Checkbox "Stop when ±0.20 µm".
Large "±0.18 µm", small "48 / 50 frames used".
A slim sample-progress bar. "Last sample accepted".
Header small stateful/event icon. It holds sample logic while the docked console provides the main Take sample button.

SESSION — berry/magenta header, compact card:
Left inputs Plan and Sample.
"Plate A", "25 / 25 stations".
"28 visits", "Closed on zero" with check.
Output Visits. This card is considerably SHORTER than Station Plan.

DRIFT CORRECTION — teal header, short wide utility:
Input Visits, output Heights.
Dropdown "Interpolate zeros".
Tiny timeline three zero-visit markers Z1 Z2 Z3.
"Zero closure +0.6 µm".
Only these controls, no unnecessary filler.

FIT SURFACE — magenta header, mid-sized:
Input Heights, output Result.
"Remove best-fit plane" checked.
Small two-dimensional contour thumbnail.
"Flatness 7.8 µm P–V".
"Noise floor 0.7 µm".
Selected outline LIGHTER MAGENTA matching header, not blue.

EXPORT — purple header, smallest node:
Input Result.
"CSV / JSON" segmented buttons.
"Save…" button, "25 rows".
No preview, no big blank body. Visibly under half the area of Sample Window.

Exact solid lightgray data wires:
Reading input tag → Sample Window Reading.
Station Plan Plan → Session Plan.
Sample Window Sample → Session Sample.
Session Visits → Drift Correction Visits, long return routed through EMPTY horizontal gutter separating upper and lower nodes, outside all bodies.
Drift Correction Heights → Fit Surface Heights → Export Result.
Stability tag is a shared bound value for the Measurement Console; may be visible at top without wire, but do not draw a wire to the console.
All graph wires have subtle arrowheads and named edge sockets. Keep clean; no duplicated connections and no wire through another node.
These nodes run in the event domain; no dashed cross-domain wires needed.

RIGHT DOCKED VIEW: "Surface Results" with tabs "Surface" active and "Points". Neutral panel header, close and float icons, NO SOCKETS.
Toolbar "Shape | As measured | Scrape | Shim", Shape selected.
"Units µm", "Drift Interpolate".
Headline "Plate A · Flatness 7.8 µm".
Large sophisticated 3D scientific surface occupying most of upper panel: a gentle warped rectangular measured plate, 5x5 sample points visible, fine triangulation/grid, X and Y axes0–400mm, Z axisµm, vertical color scale −4 to+4µm with cool blue low areas and amber high areas. Color is measurement data, not decorative UI. Make it a shallow smooth warp, no huge mountains, no laser light or camera image. Thin axes on dark backdrop; small orbit/reset icons and view cube are acceptable. This is the ONLY 3D viewport, outside node bodies.
Below viewport, small summary strip "25 stations" / "Zero closure +0.6 µm" / "Noise floor 0.7 µm".
Then table with a few measured stations and labeled units:
Station    X mm    Y mm    Shape µm    ± µm
A1          0       0       -3.9       0.18
A2        100       0       -2.1       0.17
A3        200       0        0.3       0.19
A4        300       0        2.0       0.18
A5        400       0        3.9       0.20
Footer "Rotate: drag · Zoom: wheel" and small CSV / JSON / Save buttons.
Shows actual proposed operator-facing visualization, not a wire-connected viewer node.

BOTTOM LEFT DOCKED VIEW: "Measurement Console" neutral dock header, no pins.
A single integrated console, NOT a row of equal dashboard cards.
Left half: label "Plate A — run complete", current station "A1 · Zero".
"Ready to measure", small "Expected drift 0.4 µm / 10 min".
Buttons "Take sample", "Repeat station", "New run".
Small fields "At least 2 s   At most 10 s   Target ±0.20 µm".
Right half: "Visits" compact scrollable table showing three recent visits:
28  A1 Zero   09:42  accepted
27  E5        09:38  ±0.18 µm
26  E4        09:34  ±0.19 µm
At very bottom a thin time plot of zero visits with labeled Z1/Z2/Z3 markers.
This console reads the sample and session nodes and starts sampling; the screenshot never draws cables to it.

Use the same visual density and native controls as reference 1, clearly readable labels, real-looking docking framework. The screenshot should demonstrate BOTH irregular custom nodes AND custom application panels. No macOS chrome, no LinuxCNC machine-control promises, no source code, no explanation captions, no equal-size template boxes, no graphs crossing panel boundaries.
```

