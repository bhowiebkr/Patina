# Laser measurement workbench

## Scope

The workbench ports Laser Level Webcam into Patina. The measurement code stays
inside the workbench. The editor, engine, and SDK do not get laser-specific code.

The source is the owner's `laser-level-webcam` repository, commit
`2355302eb86c280089c8fecb3149ffafc80a87d4`.

The owner selected two graphs on 2026-11-20:

- **Manual Survey**: set up the camera, find the beam, sample stations, and inspect results.
- **Automated Survey**: simulate machine motion and sample a surface without LinuxCNC.

The automated graph must identify its simulated motion. It does not connect to
LinuxCNC or move a real machine.

The original setup used a laptop for measurement and a separate Linux computer
for CNC control. LinuxCNC retains the real-time motion loop. Patina handles
measurement and survey coordination outside that loop. A future network adapter
must use bounded requests and completion messages. It must not stream servo
commands or place camera work on the controller's real-time thread.

The old TCP protocol is a reference, not a compatibility requirement. Keep the
survey sequence separate from the transport. The owner clarified this on 2026-11-20.

## Source findings

| Source | Reused responsibility |
| --- | --- |
| `laser-core/beam.rs` | Strip centroids, background removal, outlier rejection, tilt, and uncertainty |
| `laser-core/frame.rs` | Image orientation, ROI, and sensor coordinates |
| `laser-core/camera.rs` | Camera controls, measurement lock, and scaled or cropped sensor modes |
| `laser-core/monitor.rs` | Drift, disturbance detection, readiness, and Allan deviation |
| `laser-core/reading.rs` | Sample rejection, correlation-aware uncertainty, and completion rules |
| `laser-core/survey.rs` | Stations, visits, reference visits, session files, and drift correction |
| `laser-core/results.rs` | Line or plane removal, noise floor, scrape, shim, CSV, and JSON |
| `laser-camera` | Native capture, camera control readback, and the synthetic camera |

The original LinuxCNC driver runs on the machine computer. It waits for each
motion, then sends `ZERO` or `TAKE_SAMPLE` to the measurement app over TCP.
The reply to a sample contains the height in mm. The original driver converts
that value to um for its surface plot. This is not a remote G-code interface.

## UI direction

Use mockup 7 for acquisition and mockup 8 for the survey. Apply the current
neutral charcoal theme and amber focus rules. Keep the original mockup files.

The Beam Finder needs an oriented image, a profile, the ROI, and fit information.
The camera needs device and mode controls with actual control readback.
The calibrated reading must show validity and units beside the value.
The station map must show the selected station and zero station separately.

The **Survey** node combines station selection, sampling, and scan control.
The **Results** node shows measured points, reference removal, uncertainty, and numeric results.
The bottom **survey-data** tab shows station results, raw visits, and session controls.
The nodes remain the primary interface. The bottom table reads the same graph values.

## Integration checks

- Compare the adapted core against its original synthetic regression tests.
- Check orientation, sensor scaling, missing frames, and camera restarts.
- Reject mixed camera grids and changed settings during a sample.
- Check repeated visits, closing on zero, drift removal, and result export.
- Run the local automated graph through a complete simulated survey.
- Check saved parameters, session reload, dock controls, and undo.
- Run the required local Rust and frontend checks before the milestone commit.

## Current checkpoint

The workbench has eleven nodes, two graphs, a bottom data tab, and a 3D surface view.
All 111 core, camera, workflow, and display tests pass on Windows.
Tests cover simulated surveys, linear drift removal, file reload, rejected setting changes,
and retained samples after failed saves.

The generated graphs pass in Edit, Bake, and slot builds. The test runs a complete
survey, saves its visits, selects a station through the bottom table, and exports CSV.
The 3D dock reads the measured surface in all three build modes.
The manual graph also runs through the CLI.

Hidden Windows GUI checks passed for empty and populated sessions.
They checked both graphs, all node controls, the bottom tab, contrast, focus, and wire clearance.
The latest populated check showed 1,681 stations from 1,722 saved visits.
The node plot and native GPU view both displayed the reconstructed coin.
Workspace Clippy and workspace tests passed. Editor frontend checks and both prototype frontend builds passed.

A complete coin scan processed 18,177 synthetic 640 x 480 frames and measured all 1,681 stations.
The headless camera-to-session workflow averaged 2.59 ms per frame after the camera optimization, down from 6.70 ms.
Height error against the known test surface was 0.062 um RMS and 0.246 um maximum.
These errors describe the synthetic test, not physical camera accuracy.
Before the camera optimization, the generated integration check measured 42.55 ms in Edit, 6.20 ms in Bake, and 39.87 ms with the survey slot.
Each integration run used 120 frames.
The current populated hidden editor measured 18.25 ms per simulation tick and 13.95 ms per GUI update at the 150 FPS target.
Each tick can contain multiple graph frames. The page frame interval was 16.7 ms.
These are measurements on this Windows computer. Allocation counts remain unmeasured.

The core and camera manifests use local dependencies. ASCII labels replace Unicode labels.
One chunk iterator uses the current Rust spelling. The numerical algorithms are unchanged.

## Remaining limits

- Native camera backends compile on Windows. Physical camera operation remains unchecked.
- Linux and macOS execution remain unchecked.
- LinuxCNC networking and real machine motion are not implemented.
- Results show the current run. Earlier runs remain in the session file.
- Gauge-block calibration has no node UI. Its original core implementation remains available.
- Camera and analysis buffers still allocate. Session writes can delay a frame at sample completion.
- The GPU surface view needs Windows. The projected node plot uses the normal canvas path.
- The mesh connects regular grid cells. Irregular imported plans can show points without a connected surface.


## Coin probing and 3D results

The owner's screenshot defines the result direction: a colored relief surface with a height legend and an orbit camera.
The automated graph now uses a 41 x 41 grid over a 30 x 30 mm bed.
An authored coin height field drives the synthetic beam displacement. Its rim, beads, rings, and compass rose provide known detail.
This is a synthetic medallion, not measured data from the video.

The results come from the camera frames, beam finder, sampler, saved visits, and drift correction.
**Measured height** keeps the object shape relative to zero. **Leveled** removes the fitted plane for flatness work.
The mesh joins four adjacent measured stations. Missing cells remain open.
The measured channel works before enough stations exist for a plane fit.

The **Results** node has a projected 3D plot with drag and keyboard orbit controls.
The **surface-view** dock uses Patina's native GPU surface with depth testing and antialiasing.
Its camera follows the model workbench: left drag orbits, middle drag pans, and right drag or the wheel zooms.
The renderer and simulation code stay inside this workbench.
**Z display scale** changes displayed geometry only. The measured values and exports remain unchanged.

Both tables use pages of 500 rows. Every station and raw visit remains accessible beyond the editor's table row limit.
The station map builds its visited flags in one pass. Geometry rebuilds only when data or display settings change.

## Square maps and faster simulation

The station maps have square drawing areas and equal X and Y scales. Rectangular and line plans retain their actual proportions.
The project targets 150 graph frames per second, compared with the earlier 30.
Each synthetic frame still advances camera time by 1/30 second. This gives a 5x target simulation rate without reducing sample counts.
Actual playback speed depends on the computer and the visible panels. Physical cameras retain their own timestamps.

The aligned synthetic laser reuses its beam profile across rows. A regression test verifies identical pixels against the general renderer.
The full 1,681-station scan retained its earlier reconstruction errors and frame count.
The sampler now waits for enough accepted frames until the maximum sample time.
The automated preset uses a 0.2-second minimum and a 0.5-second maximum. A dropout test checks recovery and bounded failure.
