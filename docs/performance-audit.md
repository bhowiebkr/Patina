# Performance audit

Date: 2026-11-18. Review and CLI measurement baseline: `fcdda31`.
The later workspace checks use a clean archive of `277105e`.
That intervening commit changes a GPU badge label and its check, with no change to the audited execution paths.

The strongest first changes are structural caching in the canvas and less display reporting.
The engine already has the main gate-grouping optimizations.
Use a bake build before judging the maximum simulation rate.

## Scope and evidence

This audit checks graph execution, generated code, scheduling, and editor updates.
The computer example is the main workload. Other complex graphs need separate checks.

Each finding separates source evidence from measurements.
Proposed changes are not implemented optimizations.
The owner's changes in `examples/computer` are outside this audit.
Checks must use the committed example in a separate temporary folder.

## Existing work

[STATUS.md](../STATUS.md#speed-measurements) records earlier performance work.
These records are historical measurements, not new audit results.

- Light nodes already share tasks. Gate nets already have a fast path.
- Frame reports and changed-value lists already reuse buffers.
- The editor already sends changed values and draws visible node GUIs.
- Subgraph parsing and node metadata already have caches.

This audit examines the remaining costs around these improvements.

## Findings

### 1. Collect only the values that the editor needs

Priority: high. Evidence: source inspection. Speedup: `NOT_ENOUGH_INFO`; no comparison has run.

The gate generator reports every consumed output on each shown frame.
The editor compares these reports with kept values after the reports exist.
Thus, changed-value filtering saves transport work, but does not save the first formatting and collection work.
The visible-node list limits node GUIs, but does not limit these value reports.

Sources: [generate.rs](../crates/patina-engine/src/generate.rs), functions `net` and `probes`;
[frame.rs](../crates/patina-editor/src/engine/frame.rs), functions `frame` and `keep_all`.

Proposal: keep a subscription for visible wires, selected values, and view reads.
Resolve subscriptions to probe indices when the view changes.
Keep errors, logs, node execution, and typed view reads independent of display subscriptions.
Check nested subgraph aliases and the first value after a selection change.

### 2. Stop rebuilding unchanged node GUIs

Priority: high for GUI-heavy graphs. Evidence: source inspection.
Speedup: `NOT_ENOUGH_INFO`; the current GUI cost needs a separate measurement.

Each shown frame calls `draw` for every visible node GUI and each selected larger GUI.
The function creates new lists and sends each returned tree.
`Frames::call` copies the widget text into a new string.
Visibility filtering already exists, but a visible static GUI still repeats this work.

Sources: [frame.rs](../crates/patina-editor/src/engine/frame.rs), function `draw`;
[run.rs](../crates/patina-engine/src/run.rs), functions `Frames::ui` and `Frames::call`.

First proposal: retain GUI buffers and suppress unchanged tree messages.
A later change can use explicit GUI invalidation where the node contract permits it.
Do not infer unchanged GUI state from unchanged outputs. A node can change private state.
Check sliders, input events, parameter writes, and views that animate without output changes.

### 3. Measure the complete display tick

Priority: high for diagnosis. Evidence: source inspection.

The `run_ms` timer ends before value comparison, JSON transmission, and node GUI generation.
The frame loop also limits simulation work before these display costs occur.
Thus, the reported run time does not explain the complete tick cost.
The process writes JSON synchronously through a mutex and flushes the pipe.
A slow reader can delay the simulation thread.

Sources: [frame.rs](../crates/patina-editor/src/engine/frame.rs), function `frame`;
[node_process.rs](../crates/patina-editor/src/node_process.rs), function `Out::write`.

Proposal: measure simulation, value collection, value comparison, GUI generation, transport, and page rendering separately.
Reserve the measured display cost within the total tick budget.
If transport needs a bounded queue, preserve errors, logs, parameter writes, and user input order.
Merge value changes into a complete pending state before replacing a queued display update.

### 4. Cache graph structure outside live updates

Priority: high for large open graphs. Evidence: source inspection.
Speedup: `NOT_ENOUGH_INFO`; no current canvas comparison has run.

`buildNodes` maps and splits all wire destinations inside the loop over nodes.
This takes O(N x E) work for N nodes and E wires, before node rendering.
`GraphView` calls it when the run state or GUI map changes.
It preserves unchanged node objects, but first reconstructs their data and compares it.
`withValues` also scans all edges on each run update.

Sources: [graphData.ts](../crates/patina-editor/frontend/src/canvas/graphData.ts), function `buildNodes`;
[GraphView.tsx](../crates/patina-editor/frontend/src/canvas/GraphView.tsx), node and edge effects;
[liveValues.ts](../crates/patina-editor/frontend/src/canvas/liveValues.ts), function `withValues`.

First proposal: build input-pin and node metadata indices when graph structure changes.
Use each node's indexed wires during live updates.
Then update only nodes and edges reached by changed values or GUI trees.
Preserve drag positions, selection, stale values, lost pins, and expanded subgraphs.
Compare 500-node and 2000-node graphs, both paused and running.

### 5. Sample timing collection at the timing display rate

Priority: medium. Evidence: source inspection.
Speedup: `NOT_ENOUGH_INFO`; collection cost has not been isolated.

Every shown frame enables both values and times through the same `quiet` flag.
The editor transmits times on only one of six shown frames.
The other five frames still generate and parse these timing records.
Each net reports a timing record for each gate, although it measures one interval for the net.

Sources: [frame.rs](../crates/patina-editor/src/engine/frame.rs), `TIMES_EVERY` and `frame`;
[net.rs](../crates/patina-sdk/src/glue/net.rs), function `Net::run`;
[outcome.rs](../crates/patina-engine/src/outcome.rs), function `fill`.

Proposal: separate timing collection from value collection.
Collect times when the page needs a sample or the scheduler needs a new estimate.
Keep aggregate scheduler measurements current.
Check the **Debug** timeline, quiet runs, and changes between serial and parallel execution.

### 6. Preserve unchanged GUI state in the page

Priority: medium. Evidence: source inspection.

Each GUI event creates a new map, even when all widget texts match.
Each value update copies the complete value map when at least one value changes.
These updates then reach the canvas work in finding 4.

Sources: [RunStatus.tsx](../crates/patina-editor/frontend/src/run/RunStatus.tsx), function `useRun`;
[runEvents.ts](../crates/patina-editor/frontend/src/run/runEvents.ts), function `nextRun`.

First proposal: return the old GUI map when no tree changes.
Then measure whether indexed subscriptions justify changing the value store.
Keep React snapshots stable. Do not mutate a map that an earlier render can still read.

### 7. Update bus state without rebuilding all wire instances

Priority: medium for large bus graphs. Evidence: source inspection and an existing shortcut.

`WireStore::set` marks the full wire layout dirty when lane bits or hover state change.
Thus, a value change can rebuild instances for unchanged wires too.
The source comment records an earlier cost of 1 ms for 2000 wires.
That number was not measured again in this audit.

Source: [wires.ts](../crates/patina-editor/frontend/src/wires/wires.ts), function `WireStore::set`.

Proposal: separate wire geometry from lane colors and hover state.
Update only the changed state buffer or texture range.
Check bus width changes, selection, stale colors, crossings, and the fallback renderer.

### 8. End typed-value subscriptions when their views close

Priority: medium, especially for large values. Evidence: source inspection.

The first typed view read sets a flag that stays for the lifetime of `Keeps`.
Subsequent executions copy that value, including quiet simulation frames.
Closing a view drops its instance, but does not clear the cluster's flag.
After any read, gate nets also check the keep flags for their consumed outputs.

Sources: [keep.rs](../crates/patina-sdk/src/glue/keep.rs), functions `Keeps::read` and `Keeps::keep`;
[views.rs](../crates/patina-editor/src/views.rs), functions `close` and `drop_instance`;
[generate.rs](../crates/patina-engine/src/generate.rs), functions `net` and `probes`.

Proposal: track subscriptions by view instance and release unused subscriptions.
Keep shared subscriptions active until their last reader closes.
Evaluate collecting typed copies only before a consumer can read them.
Check two views of one output, close and reopen, rebuilds, and outputs consumed by value.
Speedup: `NOT_ENOUGH_INFO`; compare the same run before opening a view, while open, and after closing it.

### 9. Avoid synchronous GPU readback for unused previews

Priority: high for GPU graphs. Evidence: source inspection.
This finding does not describe the NAND execution cost.

The generic value probe calls the thumbnail and full viewer for every reported value with a viewer.
`GpuImage::thumbnail` shrinks on the GPU, then waits for a readback.
The readback creates a buffer and returns a new pixel vector.
The preview also creates a texture, a uniform buffer, and a bind group when shrinking is needed.
The shrink pipeline already has a cache.

Sources: [probe.rs](../crates/patina-sdk/src/glue/probe.rs), function `send_views`;
[image.rs](../crates/patina-gpu/src/image.rs), functions `preview`, `read`, and `thumbnail`.

First proposal: apply display subscriptions before starting thumbnail or full-view generation.
Then pool preview resources and read completed previews asynchronously.
Keep the last completed preview while a new readback is pending.
Retain synchronous reads where a CPU node needs the current pixels for correctness.
Speedup: `NOT_ENOUGH_INFO`; GPU waits and allocations need their own measurement.

### 10. Keep task grouping; measure before adding parallel gates

Priority: medium for heavy graphs, low for individual NAND gates.
Evidence: source inspection and the scheduler measurement below.

The planner already groups scalar nodes and supports `#[node(heavy)]`.
The scheduler already compares serial and pool execution.
Independent large branches can benefit from parallel execution.
A task for each NAND would restore overhead removed by the earlier gate work.

Sources: [task.rs](../crates/patina-engine/src/plan/task.rs), functions `is_light` and `split`;
[pool.rs](../crates/patina-engine/src/pool.rs), functions `run` and `step`;
[run.rs](../crates/patina-engine/src/run.rs), type `Pace`.

Proposal: retain the current small-node grouping.
Measure coarse branch groups before changing partitioning or adding workers.
Quiet-only runs keep the initial infinite node-work estimate, because only shown runs update it.
They still compare pool and serial wall times, but the 0.1 ms bypass does not activate.
Measure whether periodic pool trials cost too much for these small quiet runs.
Preserve dependencies, panic recovery, thread affinity, and the rules for non-`Sync` values.

### 11. Separate the requested simulation rate from engine capacity

Priority: high when the computer game runs slowly. Evidence: source inspection and CLI timings.

The committed computer example requests 240 frames per second.
Its CPU clock uses `every = 4`, so it requests 60 instructions per second.
More efficient gates cannot exceed that requested rate.
The editor already supports a higher **FPS** value and quiet intermediate frames.
The **Bake** action builds optimized clusters. Automatic fusion starts after 30 seconds without a change.

Sources: [patina.toml](../examples/computer/patina.toml);
[main.toml](../examples/computer/graphs/main.toml), the clock parameters;
[fuse.rs](../crates/patina-editor/src/engine/fuse.rs), `FUSE_IDLE`;
[clock.rs](../crates/patina-engine/src/clock.rs), function `frames_at`.

For a speed check, use **Bake**, then increase **FPS** within the workload's intended clock behavior.
Measure the achieved rate, because the editor discards simulation steps that exceed its tick budget.
Changing the clock divider changes the circuit's behavior. It is not a general engine optimization.

The example also prints changed readouts during quiet execution.
Quiet mode suppresses probe values and timing reports, but does not suppress node stdout.
High simulation rates can therefore produce substantial formatting, allocation, and output work.
The CLI measurement below includes that output.
A future example-specific output control must preserve the CLI checks and useful output behavior.
The 30000-frame check wrote 163608 lines, although the GUI was closed.

### 12. Treat unchanged-input skipping as a separate design change

Priority: later. Evidence: source inspection. Benefit: `NOT_ENOUGH_INFO`.

Skipping unchanged pure Boolean nets can help a mostly idle circuit.
The current gate rules establish scalar types and no node state, but do not establish purity.
A stateless Rust function can still log, read time, access a device, or change external state.
Skipping such calls can change results and side effects.

Source: [generate.rs](../crates/patina-engine/src/generate.rs), function `is_gate`.

Require a clear purity contract before caching arbitrary node results or skipping their calls.
Start with proven combinational circuits if measurements justify the change.
Compare the added dirty-check cost against direct execution of the existing optimized nets.
Keep delay state, input events, panic reporting, and side effects correct.

## Measurements from this audit

The checks ran on Windows with Rust 1.100.0 and a release engine.
The CPU is an Intel Core i9-14900K with 32 logical processors.
The shared heavy-job slot serialized this task's heavy checks.
Other desktop activity was not controlled.

The existing `measure_live_values` check processed 1000 numeric values per iteration.
It measured 78 ns per value, or about 78 us for the 1000-value collection.
This excludes GUI generation, aliases, JSON, IPC, page work, and GPU previews.
It does not measure the computer graph's complete reporting cost.

The existing `measure_split` check gave these median times, in microseconds.
Each row has the same total loop work in its three cases.

| Loop steps per task | One combined task | Eight chained tasks | Eight independent tasks |
| --- | ---: | ---: | ---: |
| 0 | 0.3 | 1.4 | 2.4 |
| 100 | 0.8 | 1.6 | 5.9 |
| 1000 | 3.5 | 5.7 | 9.4 |
| 10000 | 23.5 | 26.6 | 39.4 |
| 100000 | 177.5 | 178.7 | 127.6 |
| 1000000 | 1450.3 | 1448.3 | 541.4 |

These synthetic results support coarse tasks for substantial independent work.
They do not establish a universal split threshold or a computer-graph speedup.

Command:

```text
cargo +1.100.0 test -p patina-engine --lib --release --locked measure_ -- --ignored --nocapture --test-threads=1
```

### Computer CLI

The test used a `git archive` copy of the committed computer example.
Only its `frame_rate` changed, from 240 to 100000, in the temporary copy.
The CLI executable used a release build. The cluster mode varied between edit and bake.
Both modes ran once to warm their build caches.
The generated frame cluster contained 36 nets and 2,380,516 bytes of Rust source.
Three rounds each ran 1 frame and 30000 frames in each mode.
Native stdout redirection wrote to a file, without a terminal window or a PowerShell text pipeline.

| Cluster mode | 30000-frame process times, seconds | Incremental mean time per frame |
| --- | --- | --- |
| Bake | 0.814, 0.781, 0.806 | 17.7 to 18.6 us |
| Edit | 3.026, 3.006, 3.050 | 91.5 to 92.6 us |

The incremental estimate subtracts the paired 1-frame process time, then divides by 29999.
The median estimate is 17.9 us for bake and 91.9 us for edit, about a 5.1-fold difference.
This is a workload-level estimate, with output and frame pacing included.
It is not an isolated gate timing or an editor FPS measurement.

All six 30000-frame runs exited successfully and wrote identical output.
Each output had 1,805,594 bytes, with SHA-256:

```text
013E149BF86E83C72E08C6F0FD47486696AFA3BD7C74E5DCD3ED143BA5BE66B1
```

Commands, after setting the temporary copy's rate:

```text
cargo +1.100.0 build -p patina-cli --release --locked
target/release/patina-cli.exe run <temporary-computer> --frames 1
target/release/patina-cli.exe run <temporary-computer> --frames 30000
target/release/patina-cli.exe run <temporary-computer> --frames 1 --edit
target/release/patina-cli.exe run <temporary-computer> --frames 30000 --edit
```

The first timing attempt had a nested-shell quoting fault and used PowerShell output redirection.
Those results were discarded. The table uses the corrected native-redirection runs only.

## Suggested implementation order

1. Cache wire destinations and static node data in the canvas (finding 4).
2. Suppress unchanged GUI messages and preserve the page's GUI map (findings 2 and 6).
3. Measure all display-tick stages and collect timing samples less often (findings 3 and 5).
4. Add display subscriptions before formatting values and previews (findings 1 and 9).
5. Release typed-view subscriptions and avoid unnecessary copies (finding 8).
6. Update bus state separately from geometry (finding 7).
7. Measure coarse task partitions before changing the scheduler (finding 10).

Each change needs a before-and-after comparison on the same machine and workload.
Use the computer's main graph and CPU subgraph, plus 500-node and 2000-node graphs.
Measure simulation throughput, display-tick percentiles, allocation counts, and bytes sent per second.
Include a static graph, a graph with many changing values, and a graph with large GPU previews.
Check the editor and CLI, a fused build and a slot build, and all available platforms.

## Checks and limits

The source baseline and the existing performance records were inspected.
The two existing release measurement checks passed.
The Windows CLI comparisons passed with identical output in edit and bake modes.
The editor frontend build, type checks, and all 164 frontend tests passed.
Workspace Clippy passed with warnings denied.
The full workspace test command passed after the target-path correction below.
Both workspace checks ran in the clean source archive, so they did not use the owner's example edits.

```text
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100.0 test --workspace --locked
```

The first workspace test attempt found a path-format dependency in the event test.
The test compared a native Windows path with a mixed-separator path from `CARGO_TARGET_DIR`.
Changing that environment variable did not rebuild the test's embedded path.
The focused event test passed after recompilation with the native Windows target path.
No test source text changed.
Open test improvement: compare normalized paths in `events::triggers_start_runs_and_their_rules_hold`.
This issue has no measured effect on graph speed.

No application code changed. The proposals have no measured speedups yet.
The editor, slot mode, Linux, macOS, and allocation counts were not measured in this audit.
Risk: synthetic and CLI timings cannot establish the benefit of an editor optimization.
