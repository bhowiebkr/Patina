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
- Milestone 4 measures where to split, with a chain of small nodes and with heavy independent branches (19). Patina does not fix the cluster rules before that.

**Graph checks:** the engine checks a graph before the scheduler accepts it.

- A cycle of data wires or order wires needs a delay node. The editor refuses a connection that makes another cycle, and names the nodes of the cycle.
- The batch and event domains have no delay nodes, so their graphs have no cycles.
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
| `wgpu` | WGSL | NVIDIA, AMD, Intel, and Apple. Pure Rust |

The engine does not know about GPUs. Type crates and `patina-sdk` give GPU support.

### 10.2 GPU data

- `patina-sdk` gives one shared GPU context for each backend. All GPU nodes use the same device, so they can share buffers.
- A type crate can define GPU types, for example `GpuImage`. A GPU value stays on the GPU between GPU nodes.
- Data goes to the CPU only when a CPU node or a viewer needs it.
- Viewers of GPU images get a smaller copy at the display rate. To show GPU data at full size, use a viewport in a view (15.8).

### 10.3 Shader nodes

- A node can output shader source as a string. Another node compiles that source at run time: WGSL through `wgpu`, or CUDA C++ through NVRTC.
- A change of shader source does not need a Rust compile. This keeps shader editors fast.
