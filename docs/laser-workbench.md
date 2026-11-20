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

The workbench has eleven nodes, two graphs, and a bottom data tab.
All 98 core, camera, and workflow tests pass on Windows.
Tests cover simulated surveys, linear drift removal, file reload, rejected setting changes,
and retained samples after failed saves.

The generated graphs pass in Edit, Bake, and slot builds. The test runs a complete
survey, saves its visits, selects a station through the bottom table, and exports CSV.
The manual graph also runs through the CLI.

Hidden Windows GUI checks passed for empty and populated sessions.
They checked both graphs, all node controls, the bottom tab, contrast, focus, and wire clearance.
The populated check showed nine stations from eleven saved visits.
Workspace Clippy and workspace tests passed. Editor frontend checks and both prototype frontend builds passed.

A 640 x 480 synthetic scan measured 18.72 ms per frame in Edit and 6.85 ms in Bake.
The survey slot build measured 41.84 ms per frame. Each run used 120 frames.
The hidden editor measured 18.45 ms per graph frame and 10.45 ms per node GUI update.
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
- The surface node shows measured points and line profiles. It has no 3D surface viewport.
