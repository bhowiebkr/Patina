# Code quality, architecture and speed plan

This plan puts the code quality, the architecture and the speed of Patina back on track. Do it
before more major work. All milestones of the roadmap (spec section 19) ended on 2026-11-16.
Nobody reviewed the architecture of the whole code before 2026-11-17.

The review of 2026-11-17 measured the code with scripts on the commit "Show the node GUIs down
to a zoom of 0.25". The numbers below come from these measurements. The plan is an agent plan.
The owner can change each item.

## 1. Review results

### 1.1 Size

| Area | Lines | Files |
| --- | ---: | ---: |
| Editor frontend (TypeScript, CSS) | 17368 | 92 |
| `patina-engine` | 13081 | 37 |
| `patina-editor` (Rust backend) | 5284 | 23 |
| `patina-sdk` | 4357 | 21 |
| `patina-cli` | 1994 | 11 |
| Other crates and tools | 1859 | 14 |

- The Rust tests have about 6300 lines. `patina-editor` has 618 test lines for 4666 code lines,
  so its backend has the smallest test cover. The selfcheck covers the editor frontend.
- `cargo test --workspace` takes about 51 s. The selfcheck takes about 117 s.

### 1.2 Structure

- The crates are in good layers. `patina-engine` has no UI code. `patina-sdk` and
  `patina-capi` hold the C ABI and the `unsafe` code, as AGENTS.md tells.
- `patina-engine` is one large crate with three layers:
  - The model: the graph, the nodes, the edits and the subgraphs.
  - The build: the plan, the code generation, Cargo and the packages.
  - The runtime: the loader, the scheduler, the audio and the events.
- The engine has two module cycles: `events` and `load` use each other, and `load` and `run`
  use each other. The cause is that `Outcome`, `Report` and `fill` are in `load.rs`.
- The 92 frontend files are in one flat `src/` folder. The canvas, the panels, the dialogs,
  the run state and the selfcheck are mixed.
- The editor backend keeps the session in one `Mutex<Option<Session>>`. Each command sends the
  full view back: the graph, the nodes and the kinds. Nobody measured the size of this view or
  the time of an edit on a large graph.

### 1.3 Files and functions that are too long

- No file has more than 1000 lines. These files have more than 700 lines: `plan.rs` 849,
  `check.ts` 830, `subgraph.rs` 798, editor `engine.rs` 796, `glue.rs` 784, `load.rs` 776,
  `gui.css` 771, `node.rs` 745, `route.ts` 728, `cargo.rs` 727, `session.rs` 707,
  `generate.rs` 697 and `edit.rs` 695.
- 62 functions have more than 70 lines. These are the worst:
  - `generate.rs` `source`: 589 lines, with the nested function `make`.
  - `App` in `App.tsx`: 485 lines. `GraphView` in `GraphView.tsx`: 464 lines.
  - `checkWires` 280, `generate.rs` `step` 266 and `net` 210, `runCheck` 205.
  - `plan.rs` `check` 184, `useRun` 178, `edit.rs` `change` 177, `WireEdge` 166.
  - `events.rs` `run` 159, editor `main` 155, `generate.rs` `inline` 148.
- `App` and `GraphView` hold too many responsibilities. Each change to the editor touches them.

### 1.4 Duplicate code

- The CLI test `run.rs` and the editor `engine/blocks.rs` have the same allocation counter and
  6 more equal blocks of 8 lines.
- `import.rs` and `import/update.rs` have 3 equal blocks.
- `labels.ts` and `route.ts` have 1 equal block.

### 1.5 Tools and guards

- CI runs clippy and the tests. It does not check the format. The workspace has no
  `rustfmt.toml`. The default rustfmt changes 115 files.
- A format at the default width puts `plan.rs` (1073) and `subgraph.rs` (1164) over the limit of
  1000 lines. At a width of 120, only `subgraph.rs` (1027) goes over.
- The frontend has no linter and no formatter. `tsc` does not read the test files.
- 22 `shortcut:` comments are in the code. STATUS.md names only some of them.

### 1.6 Speed

- The editor script is one file of 4.1 MB. It holds Monaco and the selfcheck code (about
  1900 lines), and the start loads all of it. The `shortcut:` in `code.ts` names this.
- The editor frame loop copies each frame into a JSON event, so it allocates. The `shortcut:`
  in `engine/frame.rs` names this.
- Since 2026-11-17 the node GUIs draw down to a zoom of 0.25. Nobody measured the frame time of
  a graph with 500 nodes or more.
- The engine has an allocation counter test for the scheduler. The editor frame loop and the
  frontend have no speed test.

## 2. Plan

Do the phases in this order. Inside a phase, the items are independent. Each item ends with
`cargo clippy`, the tests and, for editor changes, the selfcheck. Each item is one commit.

### Phase 1: architecture first

- [ ] Write `docs/architecture.md`. Describe the crates, the three engine layers, the data path
  from an edit to a frame, and the threads. Link it from the README.
- [ ] Remove the two engine cycles. Move `Outcome`, `Report`, `ReportKind` and `fill` from
  `load.rs` into a new module `outcome.rs`. Then `events` and `run` do not use `load`.
- [ ] Put the allocation counter of the tests into one place. Use it from the CLI test and
  from `engine/blocks.rs`. Remove the other duplicate blocks of section 1.4.

### Phase 2: refactor the long files and functions

Split by responsibility. Keep the error handling and the validation of each moved part.

- [ ] Engine: `subgraph.rs` and `plan.rs` first, because a format puts them over 1000 lines.
  Then `generate.rs` `source`, `step`, `net` and `inline`, `edit.rs` `change`, `events.rs`
  `run`, `load.rs` and `node.rs`.
- [ ] Editor backend: `engine.rs` with `run_jobs`, `main.rs` `main` and `session.rs`.
- [ ] SDK: `glue.rs`.
- [ ] Frontend: split `App` into hooks and parts for the menus, the dialogs, the keys and the
  project state. Split `GraphView` into the canvas events, the selection and the drop parts.
  Split `check.ts` and `checkWires`, `useRun`, `WireEdge`, `route.ts` and `gui.css`.
- [ ] Frontend: move the files into feature folders, for example `canvas/`, `wires/`,
  `panels/`, `dialogs/`, `run/`, `code/` and `check/`. Move the tests with their files.

### Phase 3: guards

These guards keep the result of phase 2. Add each guard when the code passes it.

- [ ] Add `rustfmt.toml` with `max_width = 120`. Format the workspace in one commit, and add
  `cargo fmt --check` to CI and to the local check. Do it only after phase 2 for
  `subgraph.rs` and `plan.rs`. The commit touches about 110 files, so do it when no other
  agent has open work in the repository.
- [ ] Add `too_many_lines` to the workspace clippy lints, with a limit of 100 lines in
  `clippy.toml`. Do it when no function is over the limit.
- [ ] Let `tsc` read the test files, with a second `tsconfig` for the tests.
- [ ] OWNER APPROVAL: add ESLint and Prettier to the frontend. This needs an npm download.
  Use `max-lines-per-function` as the guard of the TypeScript functions.
- [ ] Record each `shortcut:` of section 1.5 in STATUS.md (AGENTS.md).

### Phase 4: speed

Measure before and after each change. Write the numbers into STATUS.md.

- [ ] Load Monaco only when the **Code** tab opens. Load the selfcheck and the `--shot` code
  only with their command-line flags. Measure the script size and the start time.
- [ ] Make a test graph with 500 and 2000 nodes. Measure the frame time of the canvas at the
  zooms 1, 0.5, 0.3 and 0.2, and the time of a pan and a drag. Fix the worst part.
- [ ] Measure the size of the view and the time of an edit on the computer example. If an edit
  is slower than one frame of 16 ms, send only the changed parts of the view.
- [ ] Remove the allocations of the editor frame loop, or record the measured cost if it is
  small.
- [ ] Find the slowest tests of `cargo test --workspace` and of the selfcheck. Make them
  faster without less cover.
- [ ] Measure the compile time of `patina-engine` after a change to one file. Split the crate
  into the model, the build and the runtime only if the time is high. Write the decision into
  the spec.

### Phase 5: documents

- [ ] Split STATUS.md (2448 lines). Keep the current milestone, what works, the checks and the
  known limits in STATUS.md. Move the records of the ended milestones into
  `docs/history.md`.

## 3. Keep it on track

- Run the measurements of section 1 again at the end of each milestone. Update this plan.
- Refactor a file when it gets to 800 lines, before it gets to 1000 (AGENTS.md).
- Refactor a function when it gets to 100 lines. The guard of phase 3 checks this.

## 4. Items that need the owner

- The npm download of ESLint and Prettier (phase 3).
- The time of the format commit, because another agent works in the repository (phase 3).
- A split of `patina-engine` into more crates, if the measurement shows a need (phase 4).
