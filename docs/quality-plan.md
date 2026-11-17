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
- The first measure counted the braces of the text. Braces in Rust strings made some Rust
  numbers too high, for example 266 lines for `generate.rs` `step`, which has 56 lines.
- Clippy `too_many_lines` with a limit of 100 gives the accurate Rust count. It does not count
  the comments and the blank lines. These Rust functions had more than 100 lines:
  - `generate.rs` `source` 190, `edit.rs` `change` 173, `events.rs` `run` 151, editor `main`
    149, `subgraph.rs` `expand` 121, editor `run_jobs` 117 and `slot.rs` `source` 110.
  - The tests: CLI `run.rs` 191, `plan.rs` 172, `lib.rs` 127 and CLI `events.rs` 121.
  - The milestone 0 prototypes: `render.rs` of `viewport` 181, `hot_reload.rs` 109 and
    `types_across_clusters.rs` 101.
- The worst TypeScript functions, by the brace count: `App` in `App.tsx` 485 lines,
  `GraphView` in `GraphView.tsx` 464, `checkWires` 280, `runCheck` 205, `useRun` 178 and
  `WireEdge` 166. Check these numbers again with the guard of phase 3.
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

- [x] Write `docs/architecture.md`. Describe the crates, the three engine layers, the data path
  from an edit to a frame, and the threads. Link it from the README.
- [x] Remove the two engine cycles. Move `Outcome`, `Report`, `ReportKind` and `fill` from
  `load.rs` into a new module `outcome.rs`. Then `events` and `run` do not use `load`.
- [x] Move the folder name rules and `read_node` from `plan.rs` to `project.rs`. Then the model
  layer does not use `plan`.
- [x] Remove the duplicate blocks of section 1.4. The import tests share their helpers.
  Agent decision, 2026-11-17: the allocation counter of 25 lines stays in the two test crates.
  One shared counter needs a public test item in a library crate. The equal block of 8 lines
  in `labels.ts` and `route.ts` also stays, because a shared class adds more code.

### Phase 2: refactor the long files and functions

Split by responsibility. Keep the error handling and the validation of each moved part.

- [x] Engine: split `subgraph.rs` and `plan.rs`, because a format puts them over 1000 lines.
  Then split the functions over 100 lines: `generate.rs` `source`, `edit.rs` `change`,
  `events.rs` `run`, `subgraph.rs` `expand` and `slot.rs` `source`. `load.rs` has 458 lines
  after phase 1.
- [x] Engine: move the connection kinds and `outline` out of `node.rs`.
- [x] Editor backend: split `engine.rs` `run_jobs` and `main.rs` `main`. Move the commands of
  the start screen from `session.rs` to `session/start.rs`.
- [x] Add `#[allow(clippy::too_many_lines)]` to the long tests and prototypes of section 1.3.
  Agent decision, 2026-11-17: a test reads best as one scenario, and the prototypes are the
  records of milestone 0, so they keep their long functions. The owner can change this.
- [x] SDK: move the C function bodies of `glue.rs` to `glue/abi.rs` and the share function to
  `glue/host.rs`.
- [x] Frontend: split `App` into hooks and parts for the menus, the dialogs, the keys and the
  project state. Split `GraphView` into the canvas events, the selection and the drop parts.
  Split `check.ts` and `checkWires`, `useRun`, `WireEdge`, `route.ts` and `gui.css`.
- [x] Frontend: move the files into feature folders, for example `canvas/`, `wires/`,
  `panels/`, `dialogs/`, `run/`, `code/` and `check/`. Move the tests with their files.

### Phase 3: guards

These guards keep the result of phase 2. Add each guard when the code passes it.

- [ ] Add `rustfmt.toml` with `max_width = 120`. Format the workspace in one commit, and add
  `cargo fmt --check` to CI and to the local check. Do it only after phase 2 for
  `subgraph.rs` and `plan.rs`. The commit touches about 110 files, so do it when no other
  agent has open work in the repository.
- [x] Add `too_many_lines` to the workspace clippy lints. The default limit of clippy is 100
  lines, so the workspace needs no `clippy.toml`.
- [x] Let `tsc` read the test files, with a second `tsconfig` for the tests.
- [ ] OWNER APPROVAL: add ESLint and Prettier to the frontend. This needs an npm download.
  Use `max-lines-per-function` as the guard of the TypeScript functions.
- [x] Record each `shortcut:` of section 1.5 in STATUS.md (AGENTS.md).

### Phase 4: speed

Measure before and after each change. Write the numbers into STATUS.md.

- [x] Load Monaco only when the **Code** tab opens. Load the selfcheck and the `--shot` code
  only with their command-line flags. Measure the script size and the start time. The start
  script went from 4.1 MB to 0.6 MB, and the page start from 134 ms to 33 ms (STATUS.md).
- [x] Make a test graph with 500 and 2000 nodes. Measure the frame time of the canvas at the
  zooms 1, 0.5, 0.3 and 0.2, and the time of a pan and a drag. Fix the worst part. The pan of
  2000 nodes went from 533 ms to 279 ms for each frame (STATUS.md).
- [x] Put the pins inside the node wrapper, with no `position` on `.patina-node`. A CSS test
  gave a pan of 69 ms for each frame at 2000 nodes. Remove the padding from the node width in
  the wire obstacles, **Arrange** and the frame size. The pan of 2000 nodes went from 280 ms to
  70 ms, and the drag from 350 ms to 155 ms, against a control in the same run (STATUS.md).
- [x] Measure the size of the view and the time of an edit on the computer example. If an edit
  is slower than one frame of 16 ms, send only the changed parts of the view. A move went from
  about 500 ms to 31 ms in the debug editor (STATUS.md). Agent decision, 2026-11-17: the view
  stays whole. Its JSON costs 0.2 ms, and the time was in the engine. So the engine now reads
  each subgraph file one time, keeps the parse of the node folders, and skips the check after
  an edit of only the layout.
- [ ] Remove the allocations of the editor frame loop, or record the measured cost if it is
  small.
- [ ] Find the slowest tests of `cargo test --workspace` and of the selfcheck. Make them
  faster without less cover.
- [ ] Measure the compile time of `patina-engine` after a change to one file. Split the crate
  into the model, the build and the runtime only if the time is high. Write the decision into
  the spec.

### Phase 5: documents

- [x] Split STATUS.md (2448 lines, now 574). Keep the current milestone, what works, the checks and the
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
