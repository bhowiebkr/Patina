# Build the computers of Digital Logic Sim 2

This page compares Patina with Digital Logic Sim 2 (DLS2) by Sebastian Lague. Use it when you
build the circuits of his videos in Patina. It lists the parts that match, the workarounds and
the gaps.

## What works

Each gate of a cycle runs one time in each frame, as each chip runs one time in each step of
DLS2 (spec 9.2). A wire back in the cycle gives the value of the frame before. There is no pass
limit. The frame rate is the step rate. It can be 1 to 100000 frames each second.

The CLI test `the_parts_of_digital_logic_sim_2_work_in_cycles_with_no_delay` in
`crates/patina-cli/tests/cli/computer.rs` runs these circuits with no delay node:

- A 4-bit counter of `d-flip-flop` subgraphs. Each flip-flop is two D latches of NOR gates.
- A `rom` node that reads its own output as the next address.
- A `ram` node that reads its own output.
- A `clock`, a `pulse` and a `bus` node with three drivers.
- A `dot-display` and an `rgb-display` that write a pixel at a clock tick. The dot lights an
  `led`.

A cycle can hold a node with state, for example `ram` or `rom`, because each node of the cycle
runs one time in each frame. Each node of the cycle must be a light node (spec 4.1).

## The parts

| DLS2 part | Patina part in `workbenches/computer` | Notes |
| --- | --- | --- |
| NAND | `nand` | The same. |
| Clock | `clock` | A one-frame tick each `every` frames. For a square wave, connect it to a `pulse` with `ticks` = `every` / 2. |
| Pulse | `pulse` | The same rule: a rising edge starts a pulse of `ticks` frames. An edge during a pulse starts no new pulse. |
| Key | `button`, `joystick`, `keyboard` | Gap. See the limits. |
| In and Out pins | The inputs and outputs of a subgraph | Each subgraph is a chip that you can open. |
| Tri-state buffer and Bus | `bus` | Gives the OR of the drivers that are on, and 0 when no driver is on. |
| Split and Merge, 8 bits | `split` and `join` | 8 bits only. For 4 bits, do not connect the high bits. |
| 7-segment display | `display`, `seg-join`, `hex-7seg` | |
| LED | `led` | Lit when its input is true. It has one color. |
| Dot display and RGB display | `dot-display` and `rgb-display` | The same pins and the same rule: at a rising edge of **clock**, **reset** or **write** changes the back buffer, then **refresh** shows it. The address is the row x 16 + the column, with row 0 at the bottom. The color inputs are bytes, and only the low 4 bits count. |
| ROM 256 x 16 | `rom` | Type the 256 words in the **words** text. Decimal, `0x` hex and `0b` binary are OK. |
| RAM | `ram` | 256 bytes. |
| Buzzer | None | Gap. |

## Differences and limits

- **One driver for each wire.** A DLS2 pin can have many drivers. In Patina, use the `bus` node.
  A conflict on the bus gives the OR of the values. DLS2 gives OR or AND at random.
- **A fixed order in a race.** DLS2 changes the order of waiting chips at random. Patina uses a
  fixed order, so a race always ends in the same state.
- **No Key chip.** A node canvas gets the keys only while it has the focus. Thus one node for
  each key can not read the keyboard at the same time. Use `button` nodes, the `joystick` or the
  `keyboard` node.
- **No buzzer.** Write a custom node (see [Write a custom node](custom-nodes.md)). The `led`
  has one color, not a color input.
- **The ROM editor is a text.** Select the `rom` node and edit **words** in the **Inspector**.
  There is no grid editor.
- **A ripple shows for one frame.** A counter of flip-flops can show a wrong value for one frame
  between two counts.
- **A rebuild resets a cycle.** A graph edit rebuilds the cluster. Then each value of a cycle is
  the default again, so a latch forgets its value.

## Not checked

- The `bus` node has no custom GUI, because its 16 inputs are the most that a node function takes.
  A hidden `--shot` drew the `pulse` and `bus` nodes. No shot drew `rom`, and no person used the
  three nodes in the editor.
- A hidden `--shot` drew the `led`, `dot-display` and `rgb-display` nodes. No person used them in
  the editor.
- No check ran these circuits on Linux or macOS.
