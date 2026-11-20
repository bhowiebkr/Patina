# Laser measurement nodes and views

Design proposal. No node library or camera integration was implemented.

The owner asked for nodes sized to their contents. Do not normalize these nodes
to one width or height.

## Reference

Reviewed the current [LaserVision README](https://github.com/bhowiebkr/laser-level-webcam#readme)
through the GitHub API on 2026-11-14. Inspected all four step screenshots and both
legacy Python screenshots linked from the README. The web cache returned an old
README for the raw URL; the API returned the current Rust version.

The current workflow covers camera setup, beam detection, thermal settling,
station visits, drift correction and results. The old LinuxCNC screenshot
inspired the surface viewport. It is not evidence of that integration in version 2
or in Patina. The source screenshots contain illustrative data.

## Proposed nodes

These are proposed project nodes, not Patina built-ins. Sizes describe relative
footprints at the same zoom. A compact GUI can still be wide when its content
needs width.

| Node | Footprint | Custom GUI | Main output |
|---|---|---|---|
| Camera | Tall, narrow | Device and mode selectors, manual exposure, mains setting, measurement lock, histogram, frame-gap counters | Timestamped frame |
| Orientation | Very small | Rotate and mirror buttons, sensor axes, sign check | Oriented frame |
| Beam Finder | Largest | Image with editable ROI, strip overlay, linked intensity profile, fit bands, accepted and rejected strips | Beam reading in pixels, uncertainty and quality |
| Sensor Calibration | Compact | Saved profile, sensor dimensions, pixel pitch, scaled/cropped mode diagram | Effective scale |
| Calibrated Reading | Short, wide | Large numeric position, units, uncertainty and tilt | Timestamped calibrated reading |
| Warm-up Monitor | Wide | Drift history, planned duration, allowed drift, estimate state | Stability estimate |
| Station Plan | Medium | Editable line or surface station map, spacing, zero selection | Station plan |
| Sample Window | Medium, tall | Frame scatter, excluded samples, time limits, uncertainty target and progress | Completed sample |
| Session | Small | Session name, station progress, visit count and zero closure | Visits with station IDs |
| Drift Correction | Short, wide | Zero-visit timeline and correction method | Corrected heights |
| Fit Surface | Medium | Fit mode, contour thumbnail, peak-to-valley result and noise estimate | Result data |
| Export | Smallest action node | Format and destination controls | Saved-file result |

For a line survey, **Fit Surface** should fit a line instead of a plane.
The large 3D display belongs in a view, not in the compact node.

## Connections and commands

Acquisition data:

```text
Camera -> Orientation -> Beam Finder -> Calibrated Reading -> Warm-up Monitor
Sensor Calibration ------------------> Calibrated Reading
```

Survey data:

```text
Calibrated Reading -> Sample Window -> Session -> Drift Correction -> Fit Surface -> Export
Station Plan -----------------------> Session
```

**Take sample** needs an
explicit request carrying a station ID and measurement settings. **Session**
must associate the completed sample with that request, even if selection
changes while sampling. The sample window consumes timestamped readings;
opening a view must not start or reset a measurement.

**Export** needs an explicit save request. A new upstream reading must not
silently write another file. Headless callers need the same operations.

Use one coherent event-driven design for the camera and sampling workflow.
Preserve capture timestamps, frame sequence numbers and rejected-frame counts.
Do not turn every editor display refresh into a new camera sample. Queue limits
and overload behavior need an implementation design; the pictures do not settle
those details.

## Custom views

**Measurement Console** is the main operator panel. It selects a run and station,
starts or cancels sampling, shows uncertainty and visits, and offers repeat and
zero-visit actions. It reads the nodes and sends graph commands. It has no pins.
The in-progress controls still need a design.

**Surface Results** combines a 3D viewport, a station table and result modes.
Keep the color scale, height units, coordinates and uncertainty visible.
Use a line plot with error bars for a line survey. Clearly distinguish measured
points from interpolated surface geometry. A smooth mesh is not extra measured
data.

**Beam Inspection** would offer an enlarged version of the ROI and profile
widgets for fine adjustment. It can be a full node GUI when editing one beam
finder, or a separate view when comparing sources.

**Drift History** would expand the warm-up node's plot. It should support
long-term drift and short-term scatter without making the graph node enormous.

Use shared widgets for the ROI editor, linked profile, station map, plots with
uncertainty, zero-visit timeline and sortable visit table. Node GUIs and views
should use the same values and commands. Do not duplicate measurement state
inside panels.

These placements follow Patina sections 4.3 and 15.8. Views can use a GPU
viewport. Compact node bodies use images, plots and 2D canvas widgets.

## Windows details

The **Camera** GUI must distinguish camera modes from Windows-converted modes.
Show unsupported controls as unavailable. Keep requested settings separate from
settings actually accepted by the device. The lock action needs a visible result.

Keep sensor pitch and effective frame scale distinct. Rotation, cropping,
resizing and ROI coordinates must agree. An absolute beam position is not a
drift-corrected surface height.

Save calibration and measurement settings with the session. Reopening the graph
must not depend on a particular dock arrangement.
