# GUI visual guide: Laser mockups

Part of the [GUI visual guide](../gui-visual-guide.md).

## 7. Laser acquisition

![Laser acquisition graph with differently sized camera, orientation, beam and calibration nodes](../mockups/mockup-7-laser-acquisition.png)

[Open the full-size reference](../mockups/mockup-7-laser-acquisition.png).

**Match:** the beam finder receives the most space because it combines an image
and profile. The camera is a tall form; Orientation is a small utility; the
calibrated reading is short and wide. The graph visually distinguishes setup,
analysis and monitoring.

| Exact image detail | What an agent should implement |
|---|---|
| <img src="../mockups/details/nodes/camera.png" alt="Camera controls and histogram" width="247"> | **Camera: a device form with immediate feedback**<br><br>Group device, mode and frame-rate selection before acquisition controls. Show manual exposure and mains frequency together. The histogram explains the exposure result. Lock for measuring is an explicit action with visible outcome. Unsupported Windows camera controls must be unavailable, not fictional working sliders. |
| <img src="../mockups/details/nodes/orientation.png" alt="Small orientation utility" width="171"> | **Orientation: a small transform utility**<br><br>Four icon actions, one orientation value, a sign check and axes are enough. This node should remain much smaller than the camera or beam finder. Give icons accessible names. Rotation, mirror state and the displayed coordinate system must agree. |
| <img src="../mockups/details/nodes/beam-finder.png" alt="Linked image ROI and profile" width="420"> | **Beam Finder: linked scientific editing surfaces**<br><br>The image and sideways intensity profile share a vertical coordinate. ROI handles, strip centers and rejected points explain the fit. Numeric ROI fields below are precise alternatives to dragging. Keep image overlays, fit bands, FWHM and threshold values synchronized. This is a 2D image/canvas GUI, not a GPU viewport embedded in the node. |
| <img src="../mockups/details/nodes/calibration.png" alt="Sensor calibration diagram" width="225"> | **Sensor Calibration: make the scale understandable**<br><br>The sensor diagram explains the dimensions beside pixel pitch and mode settings. Scaled and cropped modes affect effective scale differently. Keep full-sensor dimensions distinct from acquired-frame dimensions. Show units on the effective output rather than an unexplained scalar. |
| <img src="../mockups/details/nodes/calibrated-reading.png" alt="Large numeric measurement" width="339"> | **Calibrated Reading: a measurement-first node**<br><br>Use a large number for the main position and a smaller uncertainty directly below. Units are part of the value. Tilt and validity remain secondary. This is an absolute beam position; do not relabel it as corrected surface height. A failed reading must change validity, not silently retain a convincing old number. |
| <img src="../mockups/details/nodes/warm-up-monitor.png" alt="Scientific drift plot" width="363"> | **Warm-up Monitor: a plot plus a decision**<br><br>Time and displacement axes establish what drift means. Planned duration and allowed drift are inputs; expected drift and noise are results. Ready to measure is a named state. Restart estimate is an explicit action. Keep the plot wide enough to distinguish a trend from noise. |

**Correct these details:** remove the extra scale connection into Beam Finder.
Sensor Calibration feeds Calibrated Reading. The ROI geometry and strip counts
in the screenshot are schematic. The real overlay must be derived from the
current frame and fit, with timestamps and validity kept consistent.

## 8. Laser survey

![Survey workspace with graph, measurement console and separate 3D result view](../mockups/mockup-8-laser-survey.png)

[Open the full-size reference](../mockups/mockup-8-laser-survey.png).

**Match:** graph at upper left, an operator console below it, and results at
right. These are three complementary ways to work with the same session.
The console makes the graph usable as an instrument.

| Exact image detail | What an agent should implement |
|---|---|
| <img src="../mockups/details/nodes/station-plan.png" alt="Interactive station map" width="283"> | **Station Plan: an editable spatial control**<br><br>The grid, station labels and zero marker are data, not decoration. Selecting a station should update the console and result selection. Spacing and grid size affect the plan. Make the active station and the zero reference visually distinct. |
| <img src="../mockups/details/nodes/sample-window.png" alt="Sampling uncertainty and progress" width="277"> | **Sample Window: quality and progress together**<br><br>The scatter plot distinguishes accepted frames and outliers. Time bounds and uncertainty target are inputs. Current uncertainty, used-frame count and progress are results. Sampling actions must refer to one explicit request and station, even if the visible selection changes. |
| <img src="../mockups/details/nodes/fit-surface.png" alt="Compact fitting summary" width="284"> | **Fit Surface: a compact computed summary**<br><br>The fit option, contour thumbnail, flatness and noise floor make a useful short node. The node can show a projected 3D plot with orbit controls. A separate GPU view shows the same measured geometry. |

![Measurement Console custom view](../mockups/details/views/measurement-console.png)

The console combines station identity, readiness, sampling actions, limits,
visits and a zero-history plot. It is one integrated operator panel rather than
a row of unrelated dashboard tiles. Keep high-frequency actions easy to find.
During a sample, show progress and the appropriate cancel/keep actions.

![Surface Results custom view](../mockups/details/views/surface-results.png)

The result view combines a mode selector, 3D surface, color scale, summary
values and a numeric table. Axes and units stay visible. Selecting a row and
selecting a measured point should identify the same station. Distinguish
measured points from the surface interpolated between them. A line survey
uses a line plot and uncertainty bars instead.

**Correct these details:** the Plan cable must route directly to Session,
not behind Sample Window. The central zero marker and **A1 Zero** disagree.
Use one identity. The pictured surface and numbers are illustrative and do
not validate plane removal, uncertainty or drift correction.


## Surface probing update, 2026-11-20

The owner requested colored 3D results and a coin scan. Surface probing must retain the object shape by default.
Use **Measured height** for the raw geometry relative to zero. Keep plane removal under **Leveled**.
A node canvas can project the measured mesh and accept orbit input. The GPU viewport remains in a separate view.
Show a height legend, physical units, and the display exaggeration. Do not fill missing measurements from the simulation target.
