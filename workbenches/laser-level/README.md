# Laser Level Webcam

Measure a line or a surface with a laser and a lensless camera sensor.
The nodes form the instrument panel. The **survey-data** tab shows the data below the canvas.

The [illustrated guide](https://github.com/bhowiebkr/Patina/blob/main/docs/workbenches/laser-level.md) shows each part of this workbench in the editor.

This workbench uses the owner's Laser Level Webcam measurement and camera code.
See the [source details](types/SOURCE.md).

## Start without hardware

1. Open this folder in Patina.
2. Select the **automated** graph.
3. Click **Run**.
4. Click **Start scan** in the **Survey** node.
5. Watch the station map during the scan.
6. Inspect the **Results** node when measurements appear.
7. Open **surface-view** from **Views** for a larger 3D view.
8. Open **survey-data** from **Views** to inspect all stations.

The synthetic camera produces sensor images with beam noise and warm-up drift.
The automated graph scans an authored coin relief on a 30 x 30 mm bed.
The coin has a raised rim, beads, engraved rings, and a compass rose.
This is a synthetic test object, not a recording of the coin in the reference video.
The default grid has 41 x 41 stations at 0.75 mm spacing.
The graph targets 150 updates per second. The synthetic camera advances by 1/30 second per update, for a 5x simulation rate.
A complete scan takes about two minutes when the computer sustains that rate.
Set the toolbar **FPS** to **150** if an already open workbench still uses **30**.
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
The station maps use equal X and Y scales. Square plans appear square; rectangular plans keep their proportions.

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
| Scan Controller | Local simulation, test object, relief scale, travel feed, lift, settling time, and zero visit interval |
| Survey | Station selection, sample progress, keep or discard, scan control, and session saving |
| Results | Rotatable 3D surface, measured height, reference removal, peak-to-valley, uncertainty, scrape, and shim |
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
Physical camera timestamps keep their normal measurement timing at any graph rate.
The sampler can wait until **Sample at most** when too few valid frames remain after rejection.
The automated preset allows 0.2 to 0.5 seconds of simulated sampling time.

## Results and the bottom table

**Measured** gives height relative to the zero station, with laser drift correction.
**Measured height** is the default 3D channel. It retains the shape and slope of the object.
**Leveled** removes a fitted line or plane from those heights.
**Scrape** gives height above the lowest corrected station.
**Shim** gives distance below the highest corrected station.

The 3D display joins adjacent measured grid cells with triangles. Missing measurements remain holes.
It shows measured points before enough stations exist for a fitted plane.
It never reads the simulated target to fill the result surface.
Line plots include uncertainty bars. The table includes uncertainty and measurement flags for every station.
The noise estimate helps distinguish surface variation from measurement noise.

Open **survey-data** below the canvas:

- **Stations** shows positions, measured and leveled heights, shim, scrape, uncertainty, visits, and checks.
- **Raw visits** shows the individual sample readings and their frame counts.
- **Session** shows the current file and controls for opening or starting a session.

The tables use pages of 500 rows. Change **Page** to see later stations or visits.
Selecting a station row also selects that station in **Survey**.
Table units affect displayed heights. Station coordinates remain in mm.

## Explore the 3D surface

Drag the 3D plot in **Results** to rotate it. Use **Zoom in**, **Zoom out**, **Top view**, and **Reset view**.
With plot focus, arrow keys rotate, plus and minus zoom, and Home resets the view.
The wheel over a graph node keeps its normal graph navigation behavior.

Open **surface-view** for the larger view beside the graph:

- Drag with the left button to rotate.
- Drag with the middle button or Shift to pan.
- Drag with the right button, or use the wheel, to zoom.
- Press Home to reset the view.

**Z display scale** exaggerates the displayed height only. It does not change measurements, table values, or exports.
The color legend shows the selected height channel in um. X and Y use mm.
**Leveled**, **Scrape**, and **Shim** remain available for flatness work.

In **Scan Controller**, select **Surface plate** or **Coin relief** with **Test object**.
**Coin relief scale** changes the simulated object's physical height. Start a new scan after changing the object.
Use 61 x 61 stations at 0.5 mm spacing for finer detail. The scan takes longer.

## Files

Each kept visit saves the session under `measurements/` in this workbench.
**main** uses `manual.json`. **automated** uses `coin.json`.
Earlier `automated.json` sessions remain available through **Open / new session**.
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
The native 3D dock needs Windows. The node plot uses Patina's canvas controls.
