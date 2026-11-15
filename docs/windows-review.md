# Patina review for Claude Code

Reviewed by Codex on 2026-11-14 (America/Toronto).

Focus: the Windows implementation plan and the product concept. Recommendations
below do not change the specification. Ask the owner about product decisions,
including the existing questions in section 18.

## What was reviewed

Read every file in [the specification](spec/README.md), the README, agent rules,
and [the mockup plan](mockup-plan.md). Inspected both original mockups. Read image
briefs 0003 and 0004 and checked relevant text in the other draft briefs.

The folder contains documentation and images. There is no application source,
Cargo workspace, frontend, CI, `STATUS.md`, or Git metadata. No build, application,
installer, performance test, or runtime reproduction was run. Missing `STATUS.md`
is consistent with the rule to create it with the first prototype result.

**NOT_ENOUGH_INFO:** Windows implementation behavior cannot be assessed until code
and prototype results exist. Findings here are observed specification gaps,
contradictions, and reasoning about their consequences. Checks below are proposed,
not completed. There is no known repository remote in this checkout, so open and
closed GitHub issues were not searched. Nothing was filed or published.

The concept is coherent: source-based nodes, native execution, and custom GUIs and
views can support useful tools. Keep that direction. The main risk is that several
strong guarantees lack an implementation contract. Resolve them around the small
Windows prototypes, before broad editor work depends on them.

Each numbered finding is an issue draft with evidence, a proposed fix, and a check.
**P1** means a core guarantee is at risk. **P2** means a feature needs clarification
before implementation. These findings do not all block the first prototype.

## P1: core implementation contracts

### R01. Prove the complete Windows compiler path on a clean machine

**Evidence:** [13.2](spec/11-security-and-release.md#132-install-toolchain-button)
chooses Zig with a system-compiler fallback. Sections 2 and 13 promise no global
tools, `PATH` changes, or admin rights for portable use. The target triple and
required Windows SDK, import libraries, and runtime dependencies are unspecified.

**Consequence:** A successful build on a developer PC could depend on an existing
Visual Studio installation. The fallback does not satisfy the clean-machine
promise by itself. Tauri documents MSVC and C++ Build Tools for building the shell;
that is distinct from what a released Patina needs to compile user nodes.
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

**Proposed fix:** Separate the shell's build environment from the shipped node
toolchain. Specify the latter's target, compiler, linker, runtime files, and child
process environment. Treat support for arbitrary crates as limited by their native
dependencies until demonstrated.

**Proposed check:** On clean Windows under a standard account, install the local
toolchain, compile a Rust node and a dependency with C code, load their DLLs, and
run the graph. Repeat from a moved folder with spaces and non-ASCII characters.
Record exactly which prerequisites were present. This extends existing milestone 0.

### R02. Define the actual DLL dependency graph and its lifetime

**Evidence:** [5.2](spec/05-types-and-projects.md#52-the-library-boundary) describes
`std` plus type crates as one shared Rust library through `-C prefer-dynamic`.
[8.5](spec/07-layers-and-compilation.md#85-hot-reload) assigns new build filenames
and retains old libraries.

**Consequence:** `prefer-dynamic` is a linking preference, not a recipe that creates
that combined library. The exact crate types and dependency placement still need
proof. Windows also resolves dependent DLLs separately, even when the first DLL
was loaded by absolute path. Renaming only the cluster does not address every
dependency version or search-path problem.
[rustc linking option](https://doc.rust-lang.org/rustc/codegen-options/index.html#prefer-dynamic),
[Windows DLL search order](https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-search-order).

**Proposed fix:** Document the artifacts actually produced by the prototype.
Specify the versioned C interface, dependency lookup, and ownership of every
allocation and destructor. Keep old state, values, futures, and callbacks tied to
their originating library generation. Define cleanup after exit and recovery from
a failed build without deleting loaded files.

**Proposed check:** Pass owned `String`, `Vec`, and a custom value with a destructor
between two clusters. Retain an old value across reload. Load a new shared-type
generation. Check module paths and destruction. Measure memory and disk growth
over repeated reloads. Do not assume unloading is required to pass.

### R03. Strengthen type identity and correct the Cargo version claim

**Evidence:** [5.1](spec/05-types-and-projects.md#51-ownership-of-types) hashes crate
name/version, type path, size, alignment, and Rust version. A local type can change
without changing those fields. [8.1](spec/07-layers-and-compilation.md#81-toolchain-and-dependencies)
says one workspace and lockfile mean all nodes use the same crate versions.

**Consequence:** The proposed key can accept values from incompatible local builds.
Cargo can also resolve multiple incompatible versions of a package in one graph;
a lockfile records that result rather than enforcing a single version.
[Cargo version incompatibility](https://doc.rust-lang.org/cargo/reference/resolver.html#version-incompatibility-hazards).

**Proposed fix:** Tie runtime compatibility to a build identity that covers the
actual dependency graph, local source revisions, features, target, and relevant
compiler settings. Distinguish this identity from a user-facing type name. Define
Patina's policy for duplicate type-crate versions. Do not rely on size and alignment
as a compatibility proof.

**Proposed check:** Change a local struct from two `u32` fields to two `f32` fields
without changing its name or crate version. Reject retained values from the old
build. Also test two dependency versions exposing identically named types.

### R04. Make a slot update a transaction over all affected users

**Evidence:** [6.2](spec/05-types-and-projects.md#62-project-folders) permits many
instances of one node folder. [7.1](spec/07-layers-and-compilation.md#71-model) says
each instance calls the slot. But 7.2 rebuilds only the cluster holding N. Instances
can occupy different clusters and domains. Changes to signatures, shared widgets,
and dependencies do not have a complete invalidation rule.

**Consequence:** Some callers could keep the old call shape or old implementation.
A build that finishes late could overwrite newer code. An old graph can run after
a syntax error, but that alone does not explain what happens after a pin change.

**Proposed fix:** Define the dependency closure for each edit. Build and validate
that set before activation. Attach a revision to each build and diagnostic. Reject
obsolete results. Specify what stays active if preparation fails. Reset each
affected instance deliberately. Cover generic instantiations and shared widgets.

**Proposed check:** Use one node folder twice across clusters. Change its output
type, then edit it again before the first build finishes. Verify that activation is
consistent and diagnostics refer to the current source. Repeat with a widget edit.

### R05. Enforce ownership and thread affinity across the entire value path

**Evidence:** [4.4](spec/04-nodes.md#44-input-forms) allows borrowed, moved, and mutable
inputs. [9.2](spec/09-execution-and-gpu.md#92-automatic-parallelism) assigns a node
using a non-`Send` or non-`Sync` type to one fixed thread.

**Consequence:** Pinning one node is insufficient if another node creates, reads,
or drops that same value on another thread. `Send` and `Sync` describe different
permissions. A viewer retaining a value also affects whether it can be moved or
mutated. A static count of graph consumers is not enough.
[Rust's Send and Sync rules](https://doc.rust-lang.org/nomicon/send-and-sync.html).

**Proposed fix:** Specify lifetime and access rules for producers, consumers,
viewers, views, and destructors. Either assign connected thread-bound work together
or reject unsupported edges. Define the lifetime of a temporary `&mut T` input and
what happens to its backing value after the call.

**Proposed check:** Cover `Rc`, a `Send` but non-`Sync` value, parallel read branches,
and a mutable consumer while a viewer holds a result. Test both slot and fused code.

### R06. Separate arbitrary GUI execution from audio deadlines

**Evidence:** [4.3](spec/04-nodes.md#43-node-gui) gives `ui` mutable access to the node
state between `process` calls. [9.3](spec/09-execution-and-gpu.md#93-audio-domain)
forbids blocking and allocation in audio processing. [15.8](spec/15-editor.md#158-views)
says views never stop a clock domain.

**Consequence:** Running an unrestricted GUI callback between audio blocks can
miss the next deadline. Moving it to another thread introduces concurrent access
to the same mutable state. Updating at display rate does not solve this ownership
problem. The sound mockup already relies on GUIs attached to audio nodes.

**Proposed fix:** Define how GUI events and display snapshots cross into audio
state with bounded work. Specify where GUI callbacks execute. Keep graph mutation,
allocation, and old-state destruction off the audio callback. If the proposed
`ui(&mut self)` model cannot provide this, take the API tradeoff to the owner.

**Proposed check:** Exercise an intentionally slow GUI callback while audio runs.
Measure missed deadlines and confirm that UI activity cannot hold an audio lock.

### R07. Define concurrent event runs and reload of active work

**Evidence:** [9.4](spec/09-execution-and-gpu.md#94-event-domain-async-and-trigger-nodes)
allows overlapping runs, including runs lasting minutes. Each node instance has
one state. [7.2](spec/07-layers-and-compilation.md#72-edit-cycle) swaps code between
two graph runs without defining a boundary for this case.

**Consequence:** Two runs can reach the same mutable node state. A long-lived future
may retain an old code generation. There may be no global moment between runs.

**Proposed fix:** Define serialization or independent run state for overlapping
events. Specify whether active runs finish on their original graph revision,
drain before replacement, or are cancelled. Preserve the existing **Pause** promise
that active event runs finish. Keep a separate decision for forced termination.

**Proposed check:** Start two events using a stateful node, reload it while one
awaits I/O, and pause. Account for both results and state destruction.

### R08. Limit the panic guarantee and preserve node-level error attribution

**Evidence:** [4.7](spec/04-nodes.md#47-errors) promises a node panic will not stop the
engine and catches panics at library boundaries. A baked cluster contains multiple
nodes. A library-level catch alone does not identify which fused node failed.

**Consequence:** The guarantee is too broad. `catch_unwind` catches unwinding
panics, not aborts. A panic escaping an ordinary C ABI function aborts the process.
The catch must run inside the appropriate boundary. Native crashes and process
termination also require a different containment model.
[Rust catch_unwind contract](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html).

**Proposed fix:** State the supported panic strategy. Define per-node attribution
and treatment of partially modified state in fused code. Cover `init`, `process`,
GUI callbacks, viewers, and destruction. Document the limit of in-process recovery.
Do not choose the open process-isolation default for the owner.

**Proposed check:** Panic in the middle of a fused cluster and in the slot. Verify
node identification and downstream skipping. Exercise abort behavior only in an
isolated test process and record that it is outside the unwind guarantee.

### R09. Apply the trust gate before language tooling and imported builds

**Evidence:** [11](spec/11-security-and-release.md#11-security) forbids building or
running an untrusted project. Monaco connects to rust-analyzer. Imported untrusted
folders become workspace content, but the build exclusion mechanism is unspecified.

**Consequence:** Read-only editor controls do not establish that guarantee.
rust-analyzer documents that it assumes trusted code, executes build scripts and
procedural macros by default, and can follow executable overrides in project
configuration. An ordinary workspace build could also include an untrusted import.
[rust-analyzer security](https://rust-analyzer.github.io/book/security.html).

**Proposed fix:** Use a static inspection path until trust is granted. Gate project
loading by language tooling, formatters, test/build commands, native views, and
imports. Do not assume disabling two analyzer settings covers every execution path.
Keep untrusted imports outside the active build dependency graph.

**Proposed check:** Use a locally authored test fixture whose build script and
configured helper each write a marker. Neither marker may appear during untrusted
inspection, including opening the code tab. This is a proposed fixture, not a
request to execute someone else's untrusted project.

## P2: Windows delivery and product behavior

### R10. Add Windows startup behavior for missing WebView2 and writable data

**Evidence:** [13.4](spec/11-security-and-release.md#134-known-system-dependencies)
states WebView2 is included in Windows 10 and 11. Microsoft still instructs apps
to detect a missing Runtime and provide it. The portable folder layout omits the
webview profile, editor settings, logs, and recovery files.
[WebView2 distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

**Consequence:** On an affected PC the web-based **Install toolchain** button may
never appear. Unspecified data locations also undermine portability or fail under
an **All users** installation. WebView2 needs a writable user-data folder.
[WebView2 data folders](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder).

**Proposed fix:** Define detection and a startup recovery path outside the webview.
Have the owner confirm the portable runtime distribution tradeoff. Locate persistent
Patina data explicitly for each edition. Specify a useful error for a read-only
portable folder. Define cancellation and retry after an interrupted toolchain install.

**Proposed check:** Launch portable and installed editions as a standard user, with
the Runtime present and absent. Test offline startup, interrupted download, a moved
portable folder, and a second Windows user after an **All users** installation.

### R11. Make the viewport prototype prove docking and input, not only FPS

**Evidence:** [19](spec/16-roadmap.md#19-milestones) asks for a 1080p, 60 FPS viewport.
[15.2](spec/15-editor.md#152-layout) also requires detached panels and multiple
windows. [15.8](spec/15-editor.md#158-views) permits multiple view instances.

**Reasoning:** A native surface over a web panel can render quickly while failing
on clipping, overlapping menus, tab changes, focus, or docking. A copied frame can
meet a simple FPS target while creating too much latency or bandwidth under load.
Neither outcome has been measured here.

**Proposed fix:** Keep the existing milestone 0 prototype. Add one real interaction
sequence: dock, float, resize, hide, restore, close, and reopen. Include Windows
scaling changes, keyboard focus, overlays, and two viewports using shared GPU data.

**Proposed check:** Measure frame time, input latency, CPU/GPU cost, and copy volume
for the sequence. Record device-loss behavior. Run normal automation without taking
desktop focus; reserve visible multi-monitor checks for an agreed GUI session.

### R12. Define bounded streams and distinguish state updates from events

**Evidence:** [3.1](spec/01-overview.md#31-threads) keeps only the latest value when
the UI is slow. [9.1](spec/09-execution-and-gpu.md#91-clock-domains) has queues and
latest-value buffers. Neither defines capacity, overflow, or delivery guarantees.

**Consequence:** A slow spectrum view could accumulate audio blocks indefinitely.
Coalescing messages indiscriminately could lose logs or trigger edges. A brief
**Hit** press cannot always be represented by the latest boolean value.

**Proposed fix:** Classify replaceable snapshots separately from commands, trigger
events, diagnostics, and logs. Define bounded queues, overflow behavior, sequence
IDs, and visible dropped-data counts. Choose policies suitable for each domain.

**Proposed check:** Stop a viewer consuming data while audio and events continue.
Verify bounded memory, uninterrupted processing, and documented event loss behavior.

### R13. Give custom values a parameter and persistence contract

**Evidence:** [4.8](spec/04-nodes.md#48-parameters) turns disconnected value inputs
into parameters, but defines controls only for numbers, booleans, and strings.
[5.3](spec/05-types-and-projects.md#53-value-viewers) defines display support rather
than editing or serialization. Views also issue typed parameter changes.

**Consequence:** An arbitrary crate type need not have a default, editor, clone
operation, or saved representation. A `Debug` string cannot restore its value.
Undo and save/reopen therefore lack a contract for these inputs.

**Proposed fix:** Specify how type crates provide parameter editing, validation,
defaults, and serialization, or when an input must be connected. Keep this metadata
outside the type-agnostic engine. Align it with the owner's project-format decision.

**Proposed check:** Save, reopen, change, and undo a custom enum and a curve value.
Show a clear unsupported-input state for a resource handle with no parameter editor.

### R14. Make folder import and update a dependency transaction

**Evidence:** [6.2](spec/05-types-and-projects.md#62-project-folders) copies dependent
type/widget folders, allows rename or replacement, and resolves them in the receiving
workspace. It does not define how dependent manifests and references are rewritten.

**Consequence:** Importing a renamed type crate can leave broken paths or bind a node
to a different existing type. Replacing a shared widget can change unrelated nodes.
An update can overwrite local changes even if the changed folder itself is valid.

**Proposed fix:** Preview the full dependency closure, conflicts, and dependent
impact. Define package aliases or renaming rules. Validate before activation and
make failure leave the previous project usable. Keep this within the selected copy
model; a new package-management framework is not needed.

**Proposed check:** Import a node whose type and widget names both collide. Preserve
local modifications through a cancelled or failed update. Verify trust handling too.

### R15. Specify the first usable editor workflow and source/runtime distinction

**Evidence:** [the mockup plan](mockup-plan.md#needs-the-owner-first) already lists
missing creation and dependency flows. The editor spec also lacks complete save,
external-file-change, and recovery behavior. Compile errors may coexist with a graph
running an earlier successful build.

**Proposed fix:** Ask the owner for the minimum path from **New project** to creating
two nodes, connecting them, running, saving, and reopening. Define unsaved code versus
saved source versus active compiled revision. Show **Running previous build** when
that distinction matters. Define the no-successful-build case separately. Do not
label valid ongoing output stale merely because newer source fails to compile.

The existing **Pause** semantics let event work finish. Decide whether users also
need **Stop** or **Cancel**, and what those actions can promise for native code.
Specify whether a parameter edit reruns only dependent batch nodes and how users
avoid repeating file/network side effects while editing.

**Proposed check:** Walk this sequence in a small Windows editor prototype, including
a compile failure, an external file edit, and restart after an interrupted save.

## Small corrections and follow-up decisions

| Draft | Evidence | Suggested action |
|---|---|---|
| R16. Correct readiness language | The README says there is no code, then ends with "Patina is built and tested on Windows." Compile latency is stated as "a few seconds" without measurements. | Say Windows is the primary build/test target. Label compile latency as a target. Report cold build, warm slot build, and fuse time separately when measured. |
| R17. Reconcile visual rules before final images | `docs/mockup-plan.md` permits accent color for focused controls. Spec 15.3 reserves it for selected nodes/wires. The large GUI above the inspector tabs is explicitly a proposal. | Get the owner's choice and update the spec before treating either choice as settled. Keep comparison images labeled as proposals. |
| R18. Clarify the moment shown in briefs 0003 and 0004 | Both show **Code** active with **Slot: Empty** and **Fused**. Opening code prepares a slot, but idle fuse or **Bake** can legitimately return it to the base. | Describe the image as after fuse, or show **Parameters** for the initial fused state. This is an ambiguous snapshot, not a proven contradiction. |
| R19. Define graph validation | Section 9.2 mentions delay nodes but does not specify rejection of other cycles, ordering cycles, or a domain for a source with no inputs. | Add graph validation rules and useful diagnostics before the scheduler accepts a graph. Keep the domain choice visible to users. |
| R20. Measure the optimization tradeoff | Sections 7 and 8 seek cross-node inlining, while 9.2 schedules nodes individually. The generated execution plan is unspecified. | Describe which sequences are fused calls and which are parallel tasks. Benchmark a small-node chain and independent heavy branches. Avoid an unmeasured fixed cluster heuristic. |

## Suggested order for Claude

1. Correct the factual documentation claims and retain the existing milestone 0 order.
2. Turn R01-R04 into explicit acceptance cases for the compiler and reload prototypes.
3. Resolve the ownership, panic, and trust contracts before the first public editor build.
4. Extend the Windows viewport prototype with the interactions in R11.
5. Ask the owner only for the product choices that block the next feature. Record answers
   in the spec. Do not convert this review into approval for every proposed design.

This report leaves application work, spec edits, and image requests untouched. It
does not ask for new Linux or macOS work. Git commits and pushes were unavailable
because this checkout has no Git repository. Each finding can become a separate
issue after the destination and publishing approval are known.
