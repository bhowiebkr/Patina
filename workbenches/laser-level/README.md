# Laser Level Webcam

Measure a line or a surface with a laser and a lensless camera sensor.
The nodes form the instrument panel. The **survey-data** tab shows the data below the canvas.

This workbench uses the owner's Laser Level Webcam measurement and camera code.
See the [source details](types/SOURCE.md).

## Start without hardware

1. Open this folder in Patina.
2. Select the **automated** graph.
3. Click **Run**.
4. Click **Start scan** in the **Survey** node.
5. Watch the station map during the scan.
6. Inspect the **Results** node when measurements appear.
7. Open **survey-data** from **Views** to inspect all stations.

The synthetic camera produces sensor images with beam noise and warm-up drift.
The simulated controller moves a block across the original tool's test surface.
The normal beam finder measures each image. The results are not a stored animation.

The controller lifts, travels, lowers, waits, and samples. It revisits zero and finishes at zero.
The graph has no network connection. It does not send machine commands.

## Measure by hand

1. Select the **main** graph.
2. Click **Run**.
3. Set the station layout in **Station Plan**.
4. Click **New run** in **Survey** to apply a changed layout.
5. Select the zero station.
6. Place the measurement block at that station.
7. Click **Take sample**.
8. Check the sample information.
9. Click **Keep sample** to save the visit.
10. Repeat these steps at the other stations.
11. Return to zero regularly and after the final station.

With the synthetic camera, selecting a station also places the simulated block.
With a real camera, move the block yourself. The station selector does not move hardware.
The active sample keeps its initial station, even if you select another station.

**Discard** removes an unfinished or pending sample. It keeps all saved visits.
A failed save retains the pending sample. Correct the file problem, then click **Keep sample** again.

## Nodes

| Node | Controls and information |
| --- | --- |
| Camera | Device, mode, exposure controls, measurement lock, histogram, and control readback |
| Orientation | Sensor rotation, mirror, measurement axis, and height sign |
| Sensor | Pixel pitch, full sensor dimensions, scaled or cropped modes, and scale uncertainty |
| Beam Finder | Sensor image, linked intensity profile, centroid, ROI, strip count, and signal quality |
| Reading | Absolute beam position, within-frame uncertainty, tilt, and validity |
| Warm-up | Drift history, settle estimate, noise, and readiness for the planned run |
| Station Plan | Line or grid, spacing, station count, and zero reference |
| Scan Controller | Local simulation, travel feed, lift, settling time, and zero visit interval |
| Survey | Station selection, sample progress, keep or discard, scan control, and session saving |
| Results | Measured height, reference removal, peak-to-valley, uncertainty, scrape, and shim |
| Export | CSV or JSON results in a new file |

Drag across the **Beam Finder** image to set the ROI.
Use the numeric ROI fields for exact sensor coordinates.
Click **Full sensor** to clear the ROI.

## Camera setup

The initial source is **Synthetic camera**. A real camera is optional.
Windows uses Media Foundation. The inherited source also provides Linux V4L2 and macOS AVFoundation backends.

1. Click **Rescan cameras**.
2. Select the camera and its mode.
3. Set the orientation to match the sensor mounting.
4. Enter the actual sensor pitch and full sensor dimensions.
5. Select **Scaled** or **Cropped** to match the camera's smaller modes.
6. Click **Lock for measuring**.
7. Inspect the reported camera controls.
8. Adjust exposure until the beam remains clear and unclipped.

Camera controls depend on the device. Readback reports settings that the driver does not hold.
Camera, ROI, orientation, scale, or sample-rule changes discard an active sample.
A changed camera grid requires a new run. A changed calibration requires a new session file.

## Results and the bottom table

**Measured** gives height relative to the zero station, with laser drift correction.
**Shape** removes a fitted line or plane from those heights.
**Scrape** gives height above the lowest corrected station.
**Shim** gives distance below the highest corrected station.

The surface display shows measured stations. It does not fill unmeasured regions with invented values.
Line plots include uncertainty bars. The table includes uncertainty and measurement flags for every station.
The noise estimate helps distinguish surface variation from measurement noise.

Open **survey-data** below the canvas:

- **Stations** shows positions, measured heights, shape, shim, scrape, uncertainty, visits, and checks.
- **Raw visits** shows the individual sample readings and their frame counts.
- **Session** shows the current file and controls for opening or starting a session.

Selecting a station row also selects that station in **Survey**.
Table units affect displayed heights. Station coordinates remain in mm.

## Files

Each kept visit saves the session under `measurements/` in this workbench.
**main** uses `manual.json`. **automated** uses `automated.json`.
The session includes frame readings, measurement settings, station layouts, and run history.
It does not include camera image recordings.

**Save** in Patina saves graph parameters and layout.
**Keep sample** and **Save session** save measurements separately.
Undo changes graph parameters. It does not remove saved measurements or repeat a sample command.

Use **Session** in **survey-data** to choose another session file.
**Open / new session** opens an existing file or starts an empty session.
Keep or discard the pending sample before changing the file.

Use **Export new file** to write the current surface results.
CSV uses the selected height unit. JSON always uses um for heights and mm for positions.
Export refuses to replace an existing file. Select another file name for a new export.

## LinuxCNC later

The historical setup used a measurement laptop and a separate LinuxCNC computer.
The Python branch used TCP requests after each completed move.
See the [original branch](https://github.com/bhowiebkr/laser-level-webcam/tree/python).

This workbench currently uses local simulated motion. LinuxCNC is not required.
A future adapter must keep motion and real-time scheduling on the CNC computer.
Patina will coordinate completed moves and measurement requests outside that real-time loop.
The old protocol does not constrain that future adapter.

## Current limits

Results show the current run. The session file retains earlier runs.
Gauge-block scale calibration remains in the inherited core. It has no workbench GUI yet.
The frame path still allocates camera and analysis buffers. It is not a real-time control loop.
Physical cameras and the Linux and macOS backends still need hardware checks.
