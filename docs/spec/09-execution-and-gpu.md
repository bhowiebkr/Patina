# Patina Specification: Execution and GPU

Part of the [Patina Specification](README.md).

## 9. Execution

### 9.1 Clock domains

Each node runs in one clock domain. The domain decides when the node runs.

| Domain | Clock | Example uses | Target |
|---|---|---|---|
| Batch | One run when the user clicks Run or an input changes | Data processing, scientific work, export | As fast as possible |
| Frame | Display or simulation tick | Interactive tools, simulation, previews | 60 FPS (16.7 ms) |
| Audio | Audio device callback, one block of samples | Sound effects, synthesis | Block deadline. Example: 256 samples at 48 kHz = 5.3 ms |
| Event | A trigger: schedule, webhook, file change, GUI event | Automation pipelines | Fast start. A run can last minutes |

**Rules**

- A node can declare its domain: `#[node(domain = audio)]`.
- If a node does not declare a domain, it takes the domain of its inputs. If its inputs come from different domains, the editor shows a warning and the user selects the domain.
- A wire between two domains goes through a buffer that does not lock. A slow domain never stops a fast domain.
- Two kinds of cross-domain wire:
  - **Latest value:** for control values, for example a frame-domain slider that sets the gain of an audio node.
  - **Queue:** for streams, for example sample blocks or events.
- Each queue has a fixed size. A full queue never makes the frame or the audio domain wait. For a stream, the oldest item drops, and the wire shows how many items dropped.
- Trigger events and commands use queues, not latest values, so a short press of a button is never lost. If such a queue is full, the node shows an error.
- Threads: batch and frame use the worker pool. Audio uses its own high-priority thread. Event uses an async runtime (`tokio`).

**Domain decisions of milestone 4.** These are agent decisions of 2026-11-15. The owner can change them.

- A node declares its domain with `#[node(domain = batch)]` or `#[node(domain = frame)]`. Milestone 11 adds `#[node(domain = audio)]` (9.3). Event gives an error until milestone 12.
- The `domain` key of a node in the graph file (6.4) is the domain that the user selects. A node that declares a domain cannot have another domain in the graph file.
- A node with no declared and no selected domain takes the domain that all its data wires and order wires can reach. A batch and a frame input thus give the frame domain. A frame and an audio input give the audio domain. A node with no wires in is in the batch domain.
- A node with inputs that reach no common domain and no selected domain is a graph error: "select its domain". The editor shows it as an error, not a warning, until it can show warnings.
- A task (9.2) holds steps of one domain. A value never goes from one domain to another inside a task.
- The batch domain runs one time, when the user clicks **Run**. Then the frame domain runs at each tick of a 60 FPS clock until the user clicks **Stop**. A frame that is late starts at once, and the clock skips the ticks that it missed. Frames never queue.
- A run of the frame domain stops at the first frame in which a node fails.
- `patina-cli run <folder> --frames <count>` runs the given number of frames. Without `--frames`, it runs frames until the user stops the process. `--edit` builds in edit mode (8.3). `--blocks <count>` gives the number of audio blocks (9.3).
- In the editor, **Pause**, **Stop**, a failed frame and a change that runs the graph again end the frame run. **Step** runs one frame of the paused graph (15.2). A change runs the batch domain again, and then the frames start again. The status bar shows the number of the frame, and the values and run times of the frame nodes change at each frame.
- A change runs all the nodes of the batch domain again. The engine runs only the newest change: the changes that come during a run go into one next run. A parameter field and a slider apply their value when the user ends the edit: at Enter, at a focus change, or at the release of the slider. Thus a drag runs a node that writes a file or sends to the network one time, not at each step of the drag. A cache that runs only the changed nodes and the nodes after them can come later. This is the agent decision of 2026-11-15 for question 11 (18). The owner can change it.
- A wire from the batch domain to the frame domain is a latest-value wire. At each frame, the frame node reads the value of the last batch run. The value stays until its node runs again in the next batch run.
- A wire from the frame or the audio domain to the batch domain is a graph error, because the batch domain runs before the frames and the audio. Wires between the frame and the audio domain go in the two directions.
- In milestone 4, the batch and the frame domain never run at the same time. Thus the latest value is in a shared slot with a lock that never waits. The buffer that does not lock comes with the audio domain (milestone 11).

### 9.2 Automatic parallelism

Inside each domain, the engine finds parallel work from the dependency graph. The user gives no hints.

1. Each node has a counter of inputs that are not ready.
2. Nodes with a counter of zero go into a work queue.
3. A pool of worker threads, one for each CPU core, runs nodes from the queue (work stealing, for example with `rayon`).
4. When a node finishes, the engine decrements the counter of each downstream node.

Rules:

- A value of a type that is not `Send` stays on one thread. The engine runs its producer, each node that reads or takes it, and its drop on the same fixed thread. A wire that would move it to another clock domain shows an error.
- If a type is `Send` but not `Sync`, two nodes never read the same value at the same time. The engine runs those readers one after another.
- A node whose state is not `Send` runs on one fixed thread.
- A value viewer (5.3) reads a value by these rules. If a node takes the value (`T` or `&mut T`, 4.4), the viewer reads it before that node starts, at the display rate at most.
- **Order wire:** two nodes with side effects, for example two writes to one file, can have an order wire. The order wire carries no data.
- **Delay node:** a feedback loop in the frame or audio domain uses a delay node. It gives the value from the previous frame or block.

**Tasks:** the scheduler runs tasks, not single nodes. In the steps above, a node can be a task.

- Inside a cluster (8.2), a chain of small nodes is one task: one call that runs the nodes in order, inlined.
- Independent branches are separate tasks, so they can run in parallel.
- Milestone 4 measured the pool on 2026-11-15, on Windows with 32 cores (`measure_split` in `pool.rs`). A task boundary in a chain on one thread cost about 0.1 us. Eight separate tasks of less than 1 us cost 1 to 14 us more than one task. The calling thread woke 56 to 65 us after the last task ended. Eight separate tasks of 22 us ran 1.5 to 1.7 times faster than one task, and eight tasks of 180 us ran 2.5 to 5 times faster.
- First rule, an agent decision of 2026-11-15 that the owner can change: a node joins the task of its predecessor when it has exactly one predecessor (by data wires and order wires), and that predecessor has exactly one successor. Else the node starts a new task.
- Second rule, an agent decision of 2026-11-15 that the owner can change: the pool runs the tasks of a domain only when the node run times of the last run of that domain add up to 0.1 ms or more. Else the calling thread runs the tasks in index order. The first run of a domain uses the pool.
- A value that goes from one task to another is in a lock that many readers can share. A value inside one task is a local variable of the task.
- The engine has one worker pool for each process: one thread for each CPU core. The thread that starts the run is one of them. The pool runs one cluster run at a time. A second run at the same time runs its tasks one after another on its own thread.
- A cluster with a value or a parameter of a type that is not `Send` or not `Sync` runs all its tasks one after another on one thread. This keeps the rules above.
- The log lines, the live values and the run times of a run come in task order.

**Graph checks:** the engine checks a graph before the scheduler accepts it.

- A cycle of data wires or order wires needs a delay node. The editor refuses a connection that makes another cycle, and names the nodes of the cycle.
- The batch and event domains have no delay nodes, so their graphs have no cycles.
- Patina gives the delay node. A graph file names it with `builtin = "delay"` (6.4). It has one generic input `value: T` and one output of type `T`, with `T: Default`. In the first frame, the output is `T::default()`. At the end of each frame, the delay keeps its input for the next frame. It takes the value when it is the only reader in the domain, else it clones the value (4.4). The input wire of a delay makes no cycle. This is an agent decision of 2026-11-15 that the owner can change.
- The delay node runs only in the frame domain. The audio delay, with one block of latency, comes with the audio domain. The editor palette has no delay node yet. This is an agent decision of 2026-11-15 that the owner can change.
- A node with no inputs and no declared domain runs in the batch domain. Its domain icon shows this (15.3), and the user can change the domain.

Later: start the first nodes of frame N+1 while the last nodes of frame N run.

### 9.3 Audio domain

```rust
#[node(domain = audio)]
pub fn gain(
    input: &AudioBlock,
    #[param(default = 1.0, min = 0.0, max = 4.0)] gain: f32,
) -> AudioBlock {
    input.map(|s| s * gain) // buffer comes from the pool
}
```

- Audio input and output use the `cpal` crate.
- Audio nodes must not allocate memory, wait for locks, or do file or network I/O in `process`.
- The `ui` function of an audio node never runs on the audio thread (4.3).
- `AudioBlock` comes from a buffer pool in `patina-sdk`, so a return value does not allocate.
- In edit mode, Patina checks for allocations in audio nodes and shows a warning on the node.
- **Offline render:** an audio graph can also run in the batch domain. It then renders to a file faster than real time, for example to export a sound effect.
- Small audio graphs run on one thread. Large audio graphs can run independent branches on a small set of real-time threads.

**Audio decisions of milestone 11.** These are agent decisions of 2026-11-15. The owner can change them.

- A block has 256 samples at 48 kHz (5.3 ms). All audio nodes run on one audio thread. This is the decision for question 3 (18). More audio threads can come when a graph needs them.
- A wire into or out of the audio domain is a latest-value wire without a lock: a buffer of three values. The writer clones its value into the buffer, so the value type must be `Clone`. The reader never waits, allocates or drops a value on the audio thread.
- The audio domain runs on its own thread, at the same time as the frame domain. The batch domain runs before it.
- When the computer has no audio device, a timer gives the blocks, so the CLI and CI run audio graphs without a device.
- Step 1 of milestone 11 has no audio thread. The thread of the frames runs each block, between the frames. A frame runs before the first block that starts at or after its time. The latest-value wires are the same as with the audio thread.
- A node sends sound to the device with `patina_sdk::audio::play(&block)`. There is no output node: any audio node can play. The blocks that the nodes play in one block add into one mix for each project. The mix is mono, and the device gets it on all channels.
- The mix is a value that the engine and the clusters share (`glue::shared`). The engine moves it out after each block.
- The CLI opens the default output device at the first block that a node plays. The device needs 48 kHz and f32 samples. Without such a device, the CLI shows a warning and the timer stays.
- The timer paces the blocks until the device opens. Then the device paces them, with a queue of 8 blocks (43 ms). At the end of `--blocks`, the CLI waits until the device played the queue.
- A cluster run starts with `patina_cluster_begin`, which fetches the newest values of the latest-value wires. A node output on such a wire publishes its value after the node runs, so a failed node publishes no value.

### 9.4 Event domain: async and trigger nodes

- Async nodes (4.6) run on the async runtime.
- A trigger node has no inputs. It starts a run of its downstream nodes. Trigger types:
  - Schedule (time based)
  - Webhook (HTTP)
  - File change
  - GUI event, for example a button in a node GUI or in a view
- Each run has an ID. Logs show the run ID.
- A trigger can fire while a run is active. Then the trigger setting decides: queue the new run, run both at the same time, or skip the new run.
- A stateful node has one state. When runs overlap, they take turns at a stateful node: one run at a time calls it, in the order that the runs reach it. The other nodes of the runs still run at the same time.
- A code swap does not stop active runs. Each run finishes with the builds that it started with. New runs use the new builds. The old state of an edited node drops after the last old run ends.
- Later: retry rules and a run history.

## 10. GPU

### 10.1 Backends

| Backend | Kernel language | Use |
|---|---|---|
| `cudarc` + NVRTC | CUDA C++ | NVIDIA. Full CUDA speed and libraries. `cudarc` loads the CUDA driver at run time. The CUDA Toolkit is not necessary |
| `wgpu` | WESL, linked to WGSL | NVIDIA, AMD, Intel, and Apple. Pure Rust |

- The shader language for `wgpu` is [WESL](https://wesl-lang.dev/). WESL is a strict superset of WGSL. It adds imports between files, conditional compilation with `@if`, and shader packages. This is an owner decision of 2026-11-15.
- The `wesl` crate links the WESL files into one WGSL module. Then `wgpu` compiles that module. A plain WGSL file is also valid WESL.
- Code can link at build time in `build.rs`, or at run time with the linker of the `wesl` crate.

The engine does not know about GPUs. Type crates and `patina-sdk` give GPU support.

### 10.2 GPU data

- `patina-sdk` gives one shared GPU context for each backend. All GPU nodes use the same device, so they can share buffers.
- A type crate can define GPU types, for example `GpuImage`. A GPU value stays on the GPU between GPU nodes.
- Data goes to the CPU only when a CPU node or a viewer needs it.
- Viewers of GPU images get a smaller copy at the display rate. To show GPU data at full size, use a viewport in a view (15.8).
- The `patina-gpu` crate gives the shared `wgpu` context and the `GpuImage` type. Patina copies the crate into `build/sdk/` of each project, as it does `patina-sdk`. A node adds `patina-gpu = { workspace = true }`. Agent decision of 2026-11-15, the owner can change it.
- `gpu()` opens the device one time for each project. Each cluster links its own copy of `wgpu`, so the clusters share the device through the host (ABI 13). The key of a shared value is the project root and the type name, size and alignment. So the slot and the clusters of one project share one device, and two projects stay apart. Agent decision of 2026-11-15, the owner can change it.
- `gpu()` asks for a high-performance adapter. When there is none, it uses the fallback adapter of the system, for example WARP on Windows. When there is no adapter, a GPU node fails with a message. Agent decision of 2026-11-15, the owner can change it.
- The `patina-cuda` crate gives the shared CUDA context of device 0 and its default stream, also through the host. Patina copies it into `build/sdk/` as `patina-gpu`. `Cuda::kernel` compiles CUDA C++ with NVRTC and gives the compile log as the error. Agent decision of 2026-11-15, the owner can change it.
- `cudarc` loads the driver and NVRTC at run time, so a build needs no CUDA Toolkit. NVRTC is not part of the driver: `nvrtc64_120_0.dll` comes with the CUDA Toolkit 12. Until a package can carry NVRTC, a CUDA node fails with a message on a computer without it. Agent decision of 2026-11-15, the owner can change it.
- `wgpu` 30 and `wesl` 0.6 need Rust 1.97.1 or later, so a project with GPU nodes sets that Rust or a later one in `patina.toml`. Agent decision of 2026-11-15, the owner can change it.

### 10.3 Shader nodes

- A node can output shader source as a string. Another node compiles that source at run time: WESL through the `wesl` linker and `wgpu`, or CUDA C++ through NVRTC.
  - `patina_gpu::link_source(name, source, features)` links WESL text. The text can import only packages, for example `patina_gpu`, because it has no folder. An error gives `name` in place of a file name. `Cuda::kernel` compiles CUDA C++ text. Agent decision of 2026-11-15, the owner can change it.
- Shader files have the `.wesl` extension. A node keeps its shader files in its `assets/` folder (6.2).
- The `asset!("file")` macro of `patina-sdk` gives the path of a file in the `assets/` folder of the node. The engine gives the project root to each cluster when it loads (ABI 14), so the path is also correct in a package that moved. Agent decision of 2026-11-15, the owner can change it.
- `patina_gpu::link(file, features)` links a WESL file with its `@if` features at run time. `import package::name` finds `name.wesl` in the folder of the file. `Gpu::shader` compiles the WGSL and gives the validation error as text. Agent decision of 2026-11-15, the owner can change it.
- A type crate can include a WESL shader package, for example the shader functions of `GpuImage`. Node shaders import its functions with WESL `import` statements.
  - `patina-gpu` has the WESL package `patina_gpu`, which `link` always adds. Its `image` module has `luma` and `inside`, for example `import patina_gpu::image::luma;`. A function of the package must be `public`. Each module is a `.wesl` file in `patina-gpu/src/shaders`. Agent decision of 2026-11-15, the owner can change it.
- Node settings set the `@if` conditions when the linker runs. Do not make shader variants with string joins or a custom preprocessor.
- The editor shows link and compile errors at the line of the WESL file (15.5).
  - `link` validates the linked WGSL with `naga`. An error gives the WESL file and the line of the declaration that has the error, for example "shade.wesl line 2, in `shade`: ...". The `wesl` source map has no spans, so the error does not give the exact line in the declaration. Agent decision of 2026-11-15, the owner can change it.
- A change of shader source does not need a Rust compile. This keeps shader editors fast.
