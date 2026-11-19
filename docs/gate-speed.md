# Gate speed

This file compares the speed of a NAND circuit in Patina with Digital Logic Sim 2 (DLS2). It
gives the causes of the difference and the plan to make the gates faster. It started on
2026-11-17, from a question of the owner.

## How DLS2 runs a circuit

The source is [SebLague/Digital-Logic-Sim](https://github.com/SebLague/Digital-Logic-Sim),
version 2.1.6, a Unity project in C# on Mono. The file `Assets/Scripts/Simulation/Simulator.cs`
has the simulator.

- One step runs each chip one time. The first step finds the order in which the chips get all
  their inputs. The next steps use this order again.
- NAND is a built-in chip. Its code is one line in a `switch`: `1 ^ (a & b)`.
- The state of a pin is one `uint`. A bus of up to 16 bits is one value.
- An output writes its state into the input pins that it feeds. Then it counts the inputs.
- ROM, RAM, split, merge, clock, key and display are built-in chips.
- The simulation runs on its own thread. The default is 1000 steps each second, and a clock edge
  each 250 steps. A flip-flop is a loop of NAND gates, so it needs some steps to settle.
- A loop does not repeat in a step. When no chip is ready, the first step runs a random waiting
  chip with the value that its input pin kept from the step before. A pin keeps its value from
  step to step, so a latch keeps its state. Each 100 steps, two waiting chips can change places
  at random, so a race, for example in an SR latch, can end in each state. There is no pass limit
  and no warning: a NOT gate that reads itself changes at each step. (Read on 2026-11-18 in
  `Simulator.cs` and `SimPin.cs` of the `main` branch.)
- The source has these open optimizations: lookup tables for combinational chips, a skip of a
  chip with the same inputs, and a network of only the built-in chips.

The download in `reference_logic_sim/` is not DLS2. Its code has `AndGate` and `NotGate`, and no
NAND, ROM or RAM chips. Its files are from 2024-01-05, before DLS2. It is the older version 1.
Patina did not run it.

Patina has one frame for each clock step (spec 9.2), so a gate cycle must settle in the frame. It
runs the cycle again in the frame until no back value changes. This does the work of the DLS2
steps between two clock edges. The pass order is fixed, so a race always ends in the same state.
The cap of 64 passes is an agent decision. DLS2 has 250 steps between two clock edges.

## How Patina runs a circuit

This section shows the engine before the plan of this study. The plan changed these steps.

- One frame is one clock step. A `delay` node is a D flip-flop, so a frame needs no settle steps.
- Each NAND is a Rust node: `!(*a && *b)`. The code generator puts the gates in one cluster.
- The planner joins a node to the task of its predecessor only in a plain chain (spec 9.2). A
  gate net has many branches, so most gates are a task of their own.
- A value that goes from one task to another is a `Shared` value, an `RwLock<Option<T>>`.
- For each gate, the engine and the generated code do these steps:
  1. Lock the sink of the task, set a thread-local value and call the task through the C ABI.
  2. Read the two inputs to check that they are ready (two read locks).
  3. Call `glue::step`. It reads the clock, reads the inputs again (two read locks), calls the
     node in a catch and writes the output (one write lock).
  4. Read the output again for the value probe (one read lock).

## Measured results

All numbers are from the owner's Windows computer, on 2026-11-17, with a release build.

| Item | Result |
|---|---|
| A frame of `examples/computer`, bake build, quiet, no frame pacing | about 175 us (median of 5000) |
| The same frame, after plan item 1 | about 141 us (median of 5000, three runs) |
| The same frame, after plan item 2 | about 52 us (median of 5000, three runs) |
| The same frame, after plan item 3, without the standard output of the example nodes | about 13.5 us (median of 5000) |
| The same frame, after plan item 3, with that output into a file | about 28 us (median of 5000) |
| The same frame, with the node record of a net as a closure argument | about 10.2 us (median of 5000, three runs) |
| The same frame, also with no clock read for the other nodes in a quiet run | about 8.4 us (median of 5000, three runs) |
| The 10.2 us frame, with no `Option` checks (a test, not a change) | about 8.9 us (median of 5000) |
| The 8.4 us frame, then with the fast copy of each net, in one run, under load, output into a pipe | 14.1 to 15.2 us, then 12.2 to 12.4 us (medians of 5000, three rounds each) |
| Generated Rust of the frame cluster | 2.26 MB before item 3, 2.02 MB after item 3, 2.38 MB with the fast copy (2.14 MB before it) |
| Build of the frame cluster crate alone, before and with the fast copy, under load | bake 12.7 to 12.9 s and 14.2 to 14.3 s, edit with no change to the source 0.85 s and 0.91 to 0.95 s (two rounds) |
| Nodes and tasks in that frame | 1577 NAND gates and 63 other nodes, in 1330 tasks (13 after item 2) |
| Time for each node | about 107 ns (about 32 ns after item 2) |
| Benchmark, one net of 1577 gates, the Patina steps above | 71 ns for each gate |
| Benchmark, the same net, DLS2 steps, in Rust | 1.8 ns for each gate |
| Benchmark, the same net, a loop over a table | 0.32 ns for each gate |
| Benchmark, the same net, straight-line code | 0.13 ns for each gate |
| `Instant::now` | 20 ns |
| `RwLock` read lock, write lock, `Mutex` lock | 11 ns, 9 ns, 8 ns |
| `catch_unwind` with no panic | less than 1 ns |

The benchmark is a copy of the steps, not the engine. DLS2 runs on Mono, so its real time for
each gate is more than the Rust copy. Patina did not measure DLS2.

## The causes

The Rust compiler and the NAND function are not the problem. The NAND is less than 1 ns. The
glue around each gate is about 100 ns:

1. Five read locks and one write lock for each gate: about 65 ns.
2. A clock read for each gate, also in a quiet run that sends no times: about 20 ns.
3. A task for each gate: a `Mutex` lock, a thread-local value and a call through a pointer.
4. Large generated code: 2.26 MB of Rust for the computer. The bake build takes about 52 s.

Patina also has a limit of 1000 frames each second (spec 6.4), and one frame is one clock step.
The example runs at 240 FPS, which is 60 instructions each second.

## The plan

The items are in order of the gain for each change.

1. Done: a quiet run does not read the clock for each gate. Later, it also does not read the
   clock for the other nodes. The frame went from about 10.2 us to about 8.4 us.
2. Done: one task for a gate net. A light node (a delay, or a node with only `bool`, `char` and
   number values) joins the newest light task of its domain (spec 9.2). A wire in that task is a
   local `Option`, with no lock, no sink lock and no call for each gate. The frame went from
   about 141 us to about 52 us. The frame cluster has 13 tasks, not 1330. A light node that does
   much work can set `#[node(heavy)]` to keep a task of its own, so it can run in parallel.
3. Done: nets (spec 9.2). A gate is a light node with no state, no `Result`, no `&mut` input
   and one output. A row of gates in one task is a net, with one **Stop** check, one catch and
   one clock read. Each gate is one line that copies its inputs. The probes of the net come
   after the net, behind one `quiet` check. A number value between two tasks, and a number
   parameter, is an `Atom` (atomic cell) and not an `RwLock`. The frame went from about 52 us
   to about 13.5 us: 25 us with nets, 18.5 us with atoms, 13.5 us with the grouped probes. The
   computer has 1605 gates in 36 nets and 63 other nodes.
   Then the closure of a net got the node record as an argument (`glue::Gate`). Before, each gate
   stored its node index through a pointer that the closure read from the net, so the compiler
   read each wire from memory again after each gate. Now the wires stay in registers, and the
   frame went from about 13.5 us to about 10.2 us.
   Then each net got a second, fast copy. When each wire from outside the net has a value, the
   gates of the fast copy pass plain values with no `Option` check. After a panic, the slow copy
   with the checks continues from the next gate, so the results stay the same. In one run on a
   computer under load, the frame went from about 14.6 us to about 12.3 us.
   The example nodes `readout`, `display` and `lcd` write a line to the standard output only
   when their value changes. The CLI test reads these lines. Before, they wrote a line in each
   frame, and this output cost more than all the gates together.
4. Done: more frames each second. The frame rate can be 1 to 100000 FPS, not 1 to 1000. The
   editor already ran many frames in each 60 Hz tick, as DLS2 does with its steps each second.
   Now the frames of a tick stop after one tick period, so the page shows each tick and **Stop**
   works at once, also at a rate that the computer cannot reach (spec 9.1).
5. Optional, later: lookup tables for small combinational subgraphs, and a skip of a net with the
   same inputs. Bit-packed buses, as in DLS2, help only when items 2 and 3 are done.
