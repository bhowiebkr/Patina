# Patina: circuit graphs and multi-signal connections

**Status:** design proposal. No simulator or connection
renderer was implemented by this work. **Updated:** 2026-11-14.

The [GUI guide](gui-visual-guide/connections.md) contains the connection rules.

## What the source project does

The repository is [Sebastian Lague's Digital Logic Sim](https://github.com/SebLague/Digital-Logic-Sim).
Its author describes building reusable chips from NAND gates in increasingly
complex layers on the [project page](https://sebastian.itch.io/digital-logic-sim).

[Exploring How Computers Work](https://www.youtube.com/watch?v=QZwneRb-zqA)
builds from gates into adders and an add/subtract ALU. Useful sections begin at
07:24 and 15:08. [The RAM video](https://www.youtube.com/watch?v=HGkuRp5HfH8)
adds reusable memory blocks, address decoding and multi-bit connections.
Its introduction explains movable input terminals and four- and eight-bit
pins. Around 12:16 it builds larger memory by composing smaller cells and
splitting address ranges. These are building blocks toward a computer;
the referenced examples do not establish a complete general-purpose CPU.

**Evidence inspected:** chapter metadata and selected storyboard frames from
both videos, plus English automatic captions for the RAM video. The first
video's English captions were unavailable. These were sampled visual reviews,
not continuous playback. Source files were read without building or running
the external project.

The inspected source revision is
`7aeb66ddff44f916ff7fafb043ab4476ef4409fe`. Useful implementation references:

| Source | What it establishes |
|---|---|
| [ChipDescription.cs](https://github.com/SebLague/Digital-Logic-Sim/blob/7aeb66ddff44f916ff7fafb043ab4476ef4409fe/Assets/Scripts/Description/Types/ChipDescription.cs) | A chip stores explicit input/output pins, subchips, size, wires and displays. |
| [PinDescription.cs](https://github.com/SebLague/Digital-Logic-Sim/blob/7aeb66ddff44f916ff7fafb043ab4476ef4409fe/Assets/Scripts/Description/Types/SubTypes/PinDescription.cs) | Pins include identity, position, color and a width of one, four or eight bits. Numeric display modes are separate. |
| [WireDescription.cs](https://github.com/SebLague/Digital-Logic-Sim/blob/7aeb66ddff44f916ff7fafb043ab4476ef4409fe/Assets/Scripts/Description/Types/SubTypes/WireDescription.cs) | Connections retain endpoints, route points and attachment to a wire segment. |
| [WireLayoutHelper.cs](https://github.com/SebLague/Digital-Logic-Sim/blob/7aeb66ddff44f916ff7fafb043ab4476ef4409fe/Assets/Scripts/Graphics/World/WireLayoutHelper.cs) | Multi-bit rendering offsets separate lanes along a shared path. Its sharp reversals can flip the visual offset; Patina's rounded design should avoid lane-order ambiguity. |
| [PinState.cs](https://github.com/SebLague/Digital-Logic-Sim/blob/7aeb66ddff44f916ff7fafb043ab4476ef4409fe/Assets/Scripts/Simulation/PinState.cs) | The simulation represents low, high and disconnected states and explicit four/eight-bit packing. |
| [SimPin.cs](https://github.com/SebLague/Digital-Logic-Sim/blob/7aeb66ddff44f916ff7fafb043ab4476ef4409fe/Assets/Scripts/Simulation/SimPin.cs) | A signal can reach multiple targets. Multiple driving sources have simulator-specific conflict handling. That behavior is not a Patina dataflow rule. |

Use these as design references. The source is C#/Unity; this task did not port
its renderer or execution model into Patina.

## Five things that must remain distinct

| Element | Meaning | Appearance |
|---|---|---|
| Typed bus | One value containing multiple ordered signals | One named bus socket, width badge, compact line or parallel lanes |
| Visual cable | Several existing connections share a route | Named lanes and neutral grouping clamps; each connection keeps its own endpoints and type |
| Fan-out | One source has multiple readers | Neutral branch collar; every branch retains the full width |
| Split / Join | Explicit mapping between differently shaped values | Small typed conversion node; widths and ranges shown |
| Breakout | A bus becomes separately connectable scalar outputs | Narrow adapter with one bus input and one pin per signal |

A visual cable is not an implicit conversion. Removing its grouping restores
the individual routes without disconnecting values. A fan-out does not merge
two sources. A crossing does not connect anything.

For example, `Bits<8>` can be a project type from a logic type crate.
It is a proposed name, not an existing SDK type. A wrapper value remains one
bus output; a Rust tuple currently creates multiple output pins under
[spec 4.1](spec/04-nodes.md). Use the existing type-key checks. Do not introduce
built-in electronics types into the engine.

For the ribbon studies, `0x66 = 01100110`. A fork sends that full value to
both readers. A split produces `[7:4] = 0110` and `[3:0] = 0110`.
A breakout exposes eight Bit values, ordered b7 through b0. Many readers of
`&T` already fit [spec 4.4](spec/04-nodes.md); taking ownership still follows
the move/clone rules.

## Connection appearance and editing

The shared [connection guide](gui-visual-guide/connections.md) now covers
manual route points, moving segments, routes around nodes, crossings, selection,
bus presentation and extension cases. [Spec 15.9](spec/15-editor.md#159-connections-and-routing)
records the core interface rules. Keep circuit-specific rendering subject to
those rules rather than building a separate electronics editor.

A raised crossing bridge changes visual over/under order only. It does not join values.
Automatic routing, crossing minimization and bend locks remain optional
refinements. Manual geometry must not change electrical or dataflow semantics.

## Possible routing algorithms

Existing work provides a useful starting point. [libavoid](https://www.adaptagrams.org/documentation/libavoid)
routes orthogonal and polyline connectors around objects in interactive
diagrams. Its [routing parameters](https://www.adaptagrams.org/documentation/namespaceAvoid.html)
include penalties for extra segments, tight bends and crossings, plus shape
clearance and spacing controls. [Checkpoints](https://www.adaptagrams.org/documentation/annotated.html)
can constrain a route through given positions.

This establishes an algorithm reference, not a selected dependency. libavoid
is C++; this task did not add it, test a Rust binding, or choose how it would
fit the web frontend. Node placement and connection routing can remain separate;
the [ELK libavoid integration](https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-alg-libavoid.html)
explicitly describes routing edges without moving nodes.

A possible Patina pipeline, inferred from these capabilities:

1. Build obstacles from visible node and GUI bounds. Include cable-width clearance.
2. Keep endpoint approach directions and the user's fixed route points.
3. Find paths for the affected unfixed spans. Prefer fewer bends and crossings.
4. Space unrelated parallel routes apart without merging them.
5. Round the paths and offset ribbon lanes. Check the final swept cable bounds.
6. Detect remaining crossings and draw a gap or bridge with stable over/under order.

Pathfinding and bridge rendering are separate. A useful editor can start with
manual routes and simple crossing gaps. A later router can improve those same
routes without changing pins or execution. Avoid claiming that every graph can
be made crossing-free or that a pathfinder will always find a valid route
through conflicting fixed points.

The graph file saves the route points, collars and cables (spec 6.4).
The type crate owns the meaning of logic bits, disconnected states and any
explicit multi-driver resolver. Neither a bend nor a bridge grants those
semantics to an ordinary wire.

## Public subgraph interfaces

The existing spec supports collapsing, expanding and reusing subgraphs.
The new screen proposes a public-port editor with:

1. Input and output lists with names, types, order and descriptions.
2. Movable boundary terminals inside the subgraph.
3. A collapsed-node preview using the same public ports.
4. Breadcrumbs that identify the inspected instance.
5. A stable mapping from public ports to internal endpoints.

Stable port identities survive rename and reorder. Spec 6.4 gives each public
pin a stable random key. Distinguish editing a shared
definition from inspecting an instance. A reused register must not accidentally
share mutable state with every other instance.

For the pictured Full Adder, the intended mapping is:

```text
s1 = a XOR b
c1 = a AND b
sum = s1 XOR carry_in
c2 = s1 AND carry_in
carry_out = c1 OR c2
```

Expected vectors, not observed test results:

| a | b | carry_in | sum | carry_out |
|---|---|---|---|---|
| 0 | 0 | 0 | 0 | 0 |
| 0 | 0 | 1 | 1 | 0 |
| 0 | 1 | 0 | 1 | 0 |
| 0 | 1 | 1 | 0 | 1 |
| 1 | 0 | 0 | 1 | 0 |
| 1 | 0 | 1 | 0 | 1 |
| 1 | 1 | 0 | 0 | 1 |
| 1 | 1 | 1 | 1 | 1 |

Both generated subgraph screens contain wiring/table errors. Follow this mapping,
not those pixels. The guide identifies the specific visible mistakes.

## Can Patina support this?

**The visual model fits the architecture. A faithful electrical simulator needs
additional execution design and testing.** None of the following is an
implementation result.

| Capability | Existing foundation | Remaining work |
|---|---|---|
| Reusable hierarchy | Subgraphs, spec 6.1 | Public port mappings, definition/instance editing and nested navigation |
| Rich registers and probes | Stateful nodes, custom GUIs and type viewers | Logic types, parameter controls and coherent sampled values |
| Memory and timing panels | Independent Views, spec 15.8 | Bound viewers; capture history in graph-owned state rather than losing it when a view closes |
| Ribbon buses and routes | Typed wires and editor visual rules | Lane metadata, renderer, routing, picking, style controls and persistence decisions |
| Synchronous logic | Stateful nodes and explicit delays | Defined edge/step semantics and repeatable simulation tests |
| Feedback-built latches | Gate cycles, spec 9.2 | Each node of a cycle runs one time in each frame, as in DLS2. A cycle can hold light nodes with state, for example RAM and ROM. A latch settles over some frames. Other cycles still need a delay. |
| Shared electrical bus drivers | The `bus` node of `workbenches/computer` gives the OR of the drivers that are on | A wire has one driver. A general tri-state model with contention rules is not designed. See [docs/dls2-parity.md](dls2-parity.md). |
| Fast circuit simulation | Headless engine and compiled clusters | Prototype performance and event/settling behavior; no speed claim is supported by the artwork |

A specialized simulation node or compiled subgraph could own electrical
propagation internally while exposing typed inputs and outputs. That is a
possible design, not a selected engine change. A generic scheduler extension
would be a larger decision. Preserve the current cycle checks until a deliberate
spec change is approved.

## The version 1 model

Question 13 of spec 18 is decided as an agent decision of 2026-11-15. Version 1 uses a synchronous
model and no engine change:

- One frame is one clock step. All gates settle in the frame, in the order of the wires.
- A `delay` node is a D flip-flop. It gives in each frame the value of its input in the frame before.
- A cycle needs a delay, except a cycle of light gates with no state. Since 2026-11-18, such a
  cycle runs each gate one time in each frame, as DLS2 does in each step (spec 9.2). The `latch`
  graph is a D latch of two NOR gates, as in a logic simulator. The CLI test checks that it keeps its value.
- A wire has one driver. A shared bus uses an explicit multiplexer.

The `workbenches/computer` project has one Rust node for the gate, `nand`. The gates `not`, `and`,
`or` and `xor` are subgraphs of NAND gates. The half adder, the full adder, the 4-bit adder and
the 4-bit counter are subgraphs of these subgraphs. The CLI test runs 140 frames. Readout A shows
the count, and readout B the count plus 3. So the four full adders get all 8 input vectors.

The `cpu` subgraph has 794 NAND gates and 16 operations, with a 4-bit opcode and an 8-bit value
k. The memory is a Rust node, `ram`, with 256 bytes, because a RAM of NAND gates makes the graph
too large for the editor. The logic of the CPU stays NAND gates. Addresses 0x00 to 0xEF go to the
RAM, and 0xF0 to 0xFF go to the IO devices. (Agent decision, 2026-11-16. The owner can change
it.)

The screen has two Rust nodes on the IO ports. The `gpu` keeps the frame, and a read of its plot
port gives the color of a pixel, so a game can test the pixel in front of a player. The `sprites`
node draws 8 sprites over the frame on the CPU, so the `screen` node and the CLI see them. The
`monitor` view draws the same frame on the real GPU. The view does not send keys to the
`joystick`, because a parameter change from a view is an edit with an undo step. (Agent decision,
2026-11-17. The owner can change it.)

## Checks for a future implementation

These are proposed acceptance checks. The full-adder vectors and the headless run are checked
by the CLI test of `workbenches/computer`. The other checks have **not** run.

- Reuse a full-adder definition multiple times. Rename/reorder a public port.
  Verify connections and independent instance state survive.
- Check all eight full-adder vectors against the mapping above.
- Fork a bus to two readers. Verify identical values and correct ownership rules.
- Split, join and break out a known bit pattern. Verify every lane and index.
- Move nodes, junctions and manual bends. Verify connectivity is unchanged.
- Change between compact, ribbon and curved/orthogonal routes. Verify values
  and endpoints do not change.
- Save/reopen and undo/redo grouping, routing and interface edits once the
  storage format is defined.
- At Windows display scales 100%, 150% and 200%, inspect narrow lanes, picking,
  labels, keyboard focus and panel boundaries.
- Reject incompatible widths, unresolved multiple drivers and unsupported cycles.
- Run the same defined logic headlessly without the custom Views.

