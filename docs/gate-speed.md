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
- The source has these open optimizations: lookup tables for combinational chips, a skip of a
  chip with the same inputs, and a network of only the built-in chips.

The download in `reference_logic_sim/` is not DLS2. Its code has `AndGate` and `NotGate`, and no
NAND, ROM or RAM chips. It is the older version 1. Patina did not run it.

## How Patina runs a circuit

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

1. Done: a quiet run does not read the clock for each gate.
2. Done: one task for a gate net. A light node (a delay, or a node with only `bool`, `char` and
   number values) joins the newest light task of its domain (spec 9.2). A wire in that task is a
   local `Option`, with no lock, no sink lock and no call for each gate. The frame went from
   about 141 us to about 52 us. The frame cluster has 13 tasks, not 1330.
3. Straight-line code for plain gates. A node with no state, no panic and `bool` values becomes
   one line, for example `v[12] = !(v[3] & v[7]);`. The catch and the probe move to the end of
   the net. The goal is less than 1 ns for each gate. This also makes the bake build faster.
4. More frames each second. After items 2 and 3, a frame of the computer can take less than
   20 us. Then a limit of 1000 FPS is too low. Let the editor run many frames in each 60 Hz tick,
   as DLS2 does with its steps each second.
5. Optional, later: lookup tables for small combinational subgraphs, and a skip of a net with the
   same inputs. Bit-packed buses, as in DLS2, help only when items 2 and 3 are done.
