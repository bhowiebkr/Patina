# Computer

A small 8-bit computer, made of NAND gates. The style is the style of a digital logic
simulator: each gate, adder, register and memory is a subgraph that you can open.

This example is in progress. The text and the images will change.

![The main graph of the computer in the editor](images/editor.png)

## Run the example

Run these commands from the root of the repository.

- Run 16 frames in the terminal:

  ```
  cargo run -p patina-cli -- run examples/computer --frames 16
  ```

- Open the editor, then open the `examples/computer` folder. Click **Run**.

Each frame is one clock step. Each frame writes ten lines: the count, the count plus 3, an
accumulator, a memory, the three registers of the CPU and the segments of the three displays.

## What it shows

- **The CPU.** The `cpu` subgraph runs the program of the `rom-16x7` subgraph. The program
  writes the Fibonacci numbers 1 to 233 to the OUT register.
- **Byte wires.** A wire of the `byte` kind is a ribbon of 8 lanes. The badge shows the value
  of the byte. Each lane is lit when its bit is true.
- **Segment buses.** A `segment-bus` wire has the 7 lanes a to g of a 7-segment display. It
  is copper.
- **Live subgraphs.** Double-click a use of a subgraph, for example the `cpu` node. The
  subgraph opens with the live values of that use.
- **Node GUIs.** The `readout`, `display`, `lcd`, `clock` and `analyzer` nodes draw their
  values in the graph.

![A byte wire with the value 34 in the ram-4x8 subgraph](images/ram-4x8.png)

![The segment bus of the hex-7seg subgraph](images/hex-7seg.png)

## The parts

| Folder | Parts |
| --- | --- |
| `nodes/` | `nand`, `switch`, `clock`, `split`, `join`, `seg-join`, `readout`, `display`, `lcd`, `analyzer` |
| `connections/` | `byte`, `segment-bus` |
| `types/` | `segments` |
| `graphs/` | `main` and 24 subgraphs |

The subgraphs, from small to large:

- **Gates:** `not`, `and`, `or`, `xor`, `mux`.
- **Arithmetic:** `half-adder`, `full-adder`, `adder-4`, `adder-8`.
- **Selection:** `mux-4`, `mux-8`, `decoder-2`, `decoder-3`, `decoder-4`.
- **Memory:** `register-4`, `register-8`, `ram-4x4`, `ram-4x8`, `rom-16x7`, `counter-4`.
- **Output:** `hex-7seg`, `hex-ascii`, `lcd-writer`.
- **The computer:** `cpu`.

![The readouts, the logic analyzer and the hex display of the main graph](images/readouts.png)

## The instruction set

Each instruction has a 3-bit opcode and a 4-bit value k. The
[Logic circuits](../../README.md#logic-circuits) section of the main README gives the full
list, the clock and the LCD. To change the program, open `rom-16x7` and set the switches.
