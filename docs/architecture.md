# Architecture

This file describes the parts of the Patina code and how they work together. The
[specification](spec/README.md) defines the behavior. This file tells where the code of each
part is. Keep it short, and update it when a crate, a layer or a thread changes.

## 1. Crates

All code is in one Cargo workspace. The web frontend of the editor is the only code that is not
Rust.

| Crate | Job |
| --- | --- |
| `patina-engine` | Reads a project, edits and saves its graph, builds the graph into clusters and runs them (spec 3, 7, 8). It has no UI code. |
| `patina-sdk` | The crate that node code uses (spec 3), with the C ABI `glue` between a cluster and the engine. The engine copies it into the `build/` folder. |
| `patina-macros` | The `#[node]`, `#[connection]` and `#[view]` attributes. |
| `patina-event` | The async runtime of the event nodes (spec 4.6, 9.4). |
| `patina-gpu` | The GPU context, the GPU types and the WESL linker (spec 10). |
| `patina-cuda` | The CUDA context and the NVRTC compiler (spec 10.1). |
| `patina-capi` | The C API of a package (spec 14.2). |
| `patina-cli` | Runs a project or a package from the command line (spec 3, 14). |
| `patina-editor` | The editor (spec 15): a Tauri program with a Rust backend and a web frontend in `frontend/`. |
| `tools/repo-checks` | The checks of the repository, for example the file length rule. |

The `unsafe` code is at the C ABI and the loader: `patina-sdk` (`glue`), `patina-capi` and the
library loader of `patina-engine`.

## 2. Layers of `patina-engine`

The engine modules are in three layers:

1. The model: `project`, `graphs`, `node`, `types`, `edit`, `clip`, `subgraph`, `folders`,
   `import` and `git`. These modules read, change and save the project files. `project` also
   holds the rules of the folder names.
2. The build: `plan`, `generate`, `cargo`, `checked`, `package`, `install` and `update`. `plan`
   checks a graph and puts the nodes in run order. `generate` writes one cluster crate for each
   clock domain. `cargo` builds the crates.
3. The runtime: `library`, `load`, `run`, `pool`, `clock`, `device`, `events`, `view` and
   `outcome`. `load` loads the cluster libraries and calls them through the C ABI. `run` and
   `pool` run the domains. `outcome` holds the reports, the live values, the times and the logs
   of one call.

A module uses the modules of its layer and of the layers before it. The layers have no module
cycles. These are the known exceptions:

- `checked` and `package` build the clusters and then load them with `load`.
- `import`, `clip` and `git` use `cargo` and `update`, because an import or a paste builds the
  packages of the new folders, and `git` downloads a tag.

`lib.rs` holds `Project`, the entry point that the CLI and the editor use.

## 3. From an edit to a frame in the editor

1. The page calls a Tauri command. The commands are in `patina-editor/src/session.rs` and in
   `session/`. One `Mutex` keeps the open `Session`.
2. The command changes the project through `Project` of the engine. Then it sends the full view
   back to the page: the graph, the nodes and the kinds.
3. When the graph is live, the session checks the graph (`Project::check`) and sends the parts
   of the checked graph to the node process (spec 16.1). The node process is the same program,
   started with a hidden argument. The messages are lines of JSON.
4. In the node process, the engine thread (`engine.rs`) receives a `Job::Run`. It builds the
   clusters with cargo (`Checked::build_with`), loads them and runs the frames.
5. After each frame, `engine/frame.rs` collects the values and the node GUIs. The node process
   sends them to the editor, and the editor sends them to the page as an event.
6. The page keeps the graph in a store. React draws the nodes, and WebGL draws the wires.

The CLI does steps 3 to 5 in one process, with no node process and no page.

## 4. Threads

| Thread | Where | Job |
| --- | --- | --- |
| Main thread | `patina-editor/src/main.rs` | The Tauri window and the commands. |
| Engine thread | `patina-editor/src/engine.rs` | Owns the loaded clusters and the views. Builds and runs the graph. |
| Worker pool | `patina-engine/src/pool.rs` | One thread for each CPU core. Runs the nodes of a domain in parallel. The thread that starts a run is one of them. |
| Audio thread | `engine.rs`, `device.rs` | Runs the audio domain in blocks during a frame run (9.3). |
| Event threads | `patina-engine/src/events.rs` | One thread for each trigger run, and the webhook servers (9.4). |
| Fuse thread | `engine/fuse.rs` | The bake build with no slot in the background (7.2). |
| View and package threads | `views.rs`, `engine.rs` | The build of a view and of a package. |
| Output and process readers | `output.rs`, `node_process.rs` | Read the output of the node process and of the program. |

The audio thread, the frame loop, the scheduler and the live-value path must not allocate
(AGENTS.md). Tests with an allocation counter check the scheduler and the audio blocks.

## 5. Frontend

The frontend is React with `@xyflow/react` for the canvas and Monaco for the **Code** tab.
`api.ts` holds the calls of the Tauri commands, and `App.tsx` holds the window. The other
files are in one folder for each part of `src/`:

| Folder | Part |
| --- | --- |
| `canvas/` | The graph canvas: `GraphView.tsx`, the nodes, the edits, the menus and the keys. |
| `wires/` | The wires: the routes, the WebGL layer, the edges and the labels. |
| `controls/` | The shared controls of the GUI guide and the node GUIs. |
| `panels/` | The dock, its panels and the dock layout. |
| `dialogs/` | The start screen, the settings and the dialogs. |
| `run/` | The run state, the run status and the Logs. |
| `code/` | The **Code** tab and the language server. |
| `check/` | The selfcheck that `patina --selfcheck` runs, and the `--shot` code. |

A test file is next to the file that it tests.

The [quality plan](quality-plan.md) lists the open changes to this structure.
