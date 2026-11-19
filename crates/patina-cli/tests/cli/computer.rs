//! The computer example: NAND gates that count and add, and a CPU of NAND gates that runs programs.

use std::fs;
use std::path::Path;

use crate::common::{copy, run};
use patina_engine::view::{View, snapshot};
use patina_engine::{Control, Mode, Project, Toolchain};

/// The clock of the CPU ticks each 4 frames.
const EVERY: u32 = 4;

/// The segments a to g of each hex digit.
const SEGMENTS: [&str; 16] = [
    "1111110", "0110000", "1101101", "1111001", "0110011", "1011011", "1011111", "1110000", "1111111", "1111011", "1110111", "0011111", "1001110",
    "0111101", "1001111", "1000111",
];

/// The words (operation, k) of `programs/fibonacci.asm`.
const FIBONACCI: [(u8, u8); 12] = [(0, 0), (8, 0), (0, 1), (12, 0), (8, 1), (5, 0), (14, 0), (8, 2), (4, 1), (8, 0), (4, 2), (13, 3)];

/// A program that uses each operation, and its words.
const TEST: &str = "; Each operation of the CPU. OUT shows each result.
TABLE = 0x20
LDI 200
ADDI 100       ; 44, C = 1
OUT
JC carry
OUT
carry: SUBI 50 ; 250, C = 0
OUT
JC 0
ANDI 0x0F      ; 10
OUT
ST TABLE
LDI 3
TAX
LDI 7
STX TABLE      ; M[TABLE + 3] = 7
LDI 0
JZ zero
OUT
zero: LDX TABLE
OUT
ADD TABLE      ; 7 + 10
OUT
SUB 0x23       ; 17 - 7, C = 1
OUT
AND 0x23       ; 10 and 7
OUT
SUB TABLE      ; 2 - 10, C = 0
JC 0
OUT
JMP 0
";
const TEST_WORDS: [(u8, u8); 30] = [
    (0, 200),
    (1, 100),
    (12, 0),
    (14, 5),
    (12, 0),
    (2, 50),
    (12, 0),
    (14, 0),
    (3, 15),
    (12, 0),
    (8, 32),
    (0, 3),
    (11, 0),
    (0, 7),
    (9, 32),
    (0, 0),
    (15, 18),
    (12, 0),
    (10, 32),
    (12, 0),
    (5, 32),
    (12, 0),
    (6, 35),
    (12, 0),
    (7, 35),
    (12, 0),
    (6, 32),
    (14, 0),
    (12, 0),
    (13, 0),
];

#[test]
fn the_d_latch_of_two_nor_gates_keeps_its_value() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/computer"), "computer-latch");
    let (output, stdout, stderr) = run(&project, &["--graph", "latch", "--frames", "16"]);
    assert!(output.status.success(), "{stderr}");
    // The bits 0 to 3 are Q, STORE, DATA and not Q. With STORE on, Q follows DATA. With STORE off,
    // Q keeps its value: 1 after 7, and 0 after 10. Each gate runs one time in each frame, so at
    // the second set, Q reads the not Q of the frame before and comes one frame later (6, 9.2).
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, ["00000111 7", "00000001 1", "00001010 10", "00001000 8", "00000110 6", "00000001 1", "00001010 10", "00001000 8"]);
    // A NAND gate that reads its own output runs one time in each frame, so it changes at each frame (9.2).
    let osc = "format = 1\ntop = true\n[[node]]\nid = \"5a0c3e7b9d1f2468\"\nfolder = \"nodes/nand\"\ndomain = \"frame\"\n[node.params]\nb = \"true\"\n\
               [[node]]\nid = \"2d9b5f1c7e3a0846\"\nfolder = \"nodes/join\"\n[[node]]\nid = \"a8e4c0b6d2f19573\"\nfolder = \"nodes/readout\"\n\
               [[wire]]\nfrom = \"5a0c3e7b9d1f2468.out\"\nto = \"5a0c3e7b9d1f2468.a\"\n[[wire]]\nfrom = \"5a0c3e7b9d1f2468.out\"\nto = \"2d9b5f1c7e3a0846.b0\"\n\
               [[wire]]\nfrom = \"2d9b5f1c7e3a0846.out\"\nto = \"a8e4c0b6d2f19573.value\"\n";
    fs::write(project.join("graphs/osc.toml"), osc).unwrap();
    let (output, stdout, stderr) = run(&project, &["--graph", "osc", "--frames", "4"]);
    assert!(output.status.success() && !stderr.contains("WARN"), "{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["00000001 1", "00000000 0", "00000001 1", "00000000 0"]);
}

/// A top graph of `nodes`, each an id and the rest of its TOML, and `wires`, each two pins.
fn graph(nodes: &[(&str, &str)], wires: &[(String, String)]) -> String {
    let nodes = nodes.iter().map(|(id, rest)| format!("\n[[node]]\nid = \"{id}\"\n{rest}\n"));
    let wires = wires.iter().map(|(from, to)| format!("\n[[wire]]\nfrom = \"{from}\"\nto = \"{to}\"\n"));
    format!("format = 1\ntop = true\n{}{}", nodes.collect::<String>(), wires.collect::<String>())
}

/// Runs the top graph `name` for `frames` frames and gives the values that the readout wrote.
fn values(project: &Path, name: &str, frames: &str) -> Vec<u8> {
    let (output, stdout, stderr) = run(project, &["--graph", name, "--frames", frames]);
    assert!(output.status.success() && !stderr.contains("WARN"), "{stderr}");
    stdout.lines().map(|line| line.split(' ').nth(1).unwrap().parse().unwrap()).collect()
}

#[test]
fn the_parts_of_digital_logic_sim_2_work_in_cycles_with_no_delay() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/computer"), "computer-dls2");
    let w = |from: &str, to: &str| (from.to_owned(), to.to_owned());
    // A counter of 4 D flip-flops of NAND gates, as in the videos of Digital Logic Sim 2.
    let (q, d) = (|i: usize| format!("f00000000000000{i}.c3a96e04"), |i: usize| format!("f00000000000000{i}.4d1c7a90"));
    let mut wires = vec![(q(0), "a000000000000000.5ade65ce".to_owned()), ("a000000000000000.7ff633ef".to_owned(), d(0))];
    let mut carry = q(0);
    for i in 0..4 {
        wires.extend([w("c0c0c0c0c0c0c001.out", &format!("f00000000000000{i}.8b2e5f13")), (q(i), format!("d000000000000001.b{i}"))]);
        if i > 0 {
            let add = format!("a00000000000000{i}");
            wires.extend([(q(i), format!("{add}.9131f68f")), (carry, format!("{add}.1e8b06d1")), (format!("{add}.301a2640"), d(i))]);
            carry = format!("{add}.d84ec94f");
        }
    }
    wires.push(w("d000000000000001.out", "d000000000000002.value"));
    let (ff, half) = ("subgraph = \"d-flip-flop\"", "subgraph = \"half-adder\"");
    let nodes = [
        ("c0c0c0c0c0c0c001", "folder = \"nodes/clock\"\n[node.params]\nevery = \"4\""),
        ("f000000000000000", ff), ("f000000000000001", ff), ("f000000000000002", ff), ("f000000000000003", ff),
        ("a000000000000000", "subgraph = \"not\""), ("a000000000000001", half), ("a000000000000002", half), ("a000000000000003", half),
        ("d000000000000001", "folder = \"nodes/join\""), ("d000000000000002", "folder = \"nodes/readout\""),
    ];
    fs::write(project.join("graphs/count.toml"), graph(&nodes, &wires)).unwrap();
    // The first value comes from the latches at power on. Then the counter counts 0 to 15 and
    // again 0. A bit that goes off changes one frame before a bit that goes on, so a short value
    // can come between two counts, as for one step in Digital Logic Sim 2 (9.2).
    let counts = [15, 0, 1, 0, 2, 3, 0, 4, 5, 4, 6, 7, 0, 8, 9, 8, 10, 11, 8, 12, 13, 12, 14, 15, 0];
    assert_eq!(values(&project, "count", "70"), counts);
    // A ROM, a RAM and a pulse in cycles. Each reads its own output of the frame before.
    let (rom, ram, out) = ("e000000000000003", "e000000000000001", "d000000000000002");
    let rom_node = [(rom, "folder = \"nodes/rom\"\ndomain = \"frame\"\n[node.params]\nwords = \"1 2 3 0x10\""), (out, "folder = \"nodes/readout\"")];
    fs::write(project.join("graphs/rom.toml"), graph(&rom_node, &[w(&format!("{rom}.out1"), &format!("{rom}.address")), w(&format!("{rom}.out1"), &format!("{out}.value"))])).unwrap();
    assert_eq!(values(&project, "rom", "6"), [1, 2, 3, 16, 0, 1]);
    let ram_node = [(ram, "folder = \"nodes/ram\"\ndomain = \"frame\"\n[node.params]\ndata = \"5\"\nwrite = \"true\""), (out, "folder = \"nodes/readout\"")];
    fs::write(project.join("graphs/ram.toml"), graph(&ram_node, &[w(&format!("{ram}.out"), &format!("{ram}.address")), w(&format!("{ram}.out"), &format!("{out}.value"))])).unwrap();
    assert_eq!(values(&project, "ram", "6"), [5, 0, 5]);
    // A pulse of 2 frames at each clock tick drives the bus with a second driver of 64.
    let parts = [
        ("c0c0c0c0c0c0c001", "folder = \"nodes/clock\"\n[node.params]\nevery = \"4\""),
        ("e000000000000004", "folder = \"nodes/pulse\"\n[node.params]\nticks = \"2\""),
        ("e000000000000005", "folder = \"nodes/bus\"\n[node.params]\non0 = \"true\"\nd1 = \"64\"\non1 = \"true\"\nd2 = \"128\""),
        ("d000000000000001", "folder = \"nodes/join\""), (out, "folder = \"nodes/readout\""),
    ];
    let wires = [w("c0c0c0c0c0c0c001.out", "e000000000000004.input"), w("e000000000000004.out", "d000000000000001.b0"), w("d000000000000001.out", "e000000000000005.d0"), w("e000000000000005.out", &format!("{out}.value"))];
    fs::write(project.join("graphs/parts.toml"), graph(&parts, &wires)).unwrap();
    assert_eq!(values(&project, "parts", "9"), [65, 64, 65, 64, 65]);
    // At a clock tick, the displays write a pixel and show it. The dot of the screen lights an LED.
    let (dots, rgb, lamp) = ("e000000000000006", "e000000000000007", "e000000000000008");
    let shows = [
        ("c0c0c0c0c0c0c001", "folder = \"nodes/clock\"\n[node.params]\nevery = \"4\""),
        (dots, "folder = \"nodes/dot-display\"\n[node.params]\naddress = \"17\"\npixel = \"true\"\nwrite = \"true\"\nrefresh = \"true\""),
        (rgb, "folder = \"nodes/rgb-display\"\n[node.params]\naddress = \"1\"\nred = \"15\"\nblue = \"5\"\nwrite = \"true\"\nrefresh = \"true\""),
        (lamp, "folder = \"nodes/led\""),
    ];
    let wires = [w("c0c0c0c0c0c0c001.out", &format!("{dots}.clock")), w("c0c0c0c0c0c0c001.out", &format!("{rgb}.clock")), w(&format!("{dots}.out"), &format!("{lamp}.on"))];
    fs::write(project.join("graphs/shows.toml"), graph(&shows, &wires)).unwrap();
    let (output, stdout, stderr) = run(&project, &["--graph", "shows", "--frames", "6"]);
    assert!(output.status.success() && !stderr.contains("WARN"), "{stderr}");
    let line = |name: &str| stdout.lines().filter(|l| l.starts_with(name)).collect::<Vec<_>>();
    assert_eq!(line("led"), ["led on"], "{stdout}");
    let row = |l: &str, n: usize| l.split(' ').nth(16 - n).unwrap().to_owned();
    assert_eq!(line("dots").iter().map(|l| row(l, 1)).collect::<Vec<_>>(), ["4000"], "{stdout}");
    assert_eq!(line("rgb").iter().map(|l| row(l, 0)[3..6].to_owned()).collect::<Vec<_>>(), ["f05"], "{stdout}");
}

#[test]
fn the_computer_example_of_nand_gates_counts_adds_and_runs_programs() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/computer"), "computer");
    // 560 frames run 140 instructions. At the carry, the program starts again.
    assert_eq!(check(&project, 560, &FIBONACCI), [1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 1, 1, 2]);
    // The program node reads its file again at the start of the run.
    fs::write(project.join("programs/fibonacci.asm"), TEST).unwrap();
    assert_eq!(check(&project, 480, &TEST_WORDS)[..9], [44, 250, 10, 7, 17, 10, 2, 248, 44]);
    // LD at the plot port gives the color of a pixel. The sprite chip draws one dot over the frame.
    fs::write(project.join("programs/fibonacci.asm"), SPRITE).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "100"]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.lines().rfind(|l| l.starts_with("lcd ")), Some("lcd |05 06           |                |"));
    // The dot shows at 0,0, then moves to 10,0 and 10,10.
    let screens: Vec<_> = stdout.lines().filter(|l| l.starts_with("screen ")).collect();
    assert_eq!(screens, ["1 pixels on", "2 pixels on", "2 pixels on", "2 pixels on"].map(|n| format!("screen 32 x 32, {n}")));
    // Snake at one instruction each frame: the snake grows to 5 cells and the tail follows the head.
    let graph = project.join("graphs/main.toml");
    fs::write(&graph, fs::read_to_string(&graph).unwrap().replacen("every = \"4\"", "every = \"1\"", 1)).unwrap();
    // The time seeds the random chip, so the test puts the food at 3,3, off the path of the snake.
    let snake = fs::read_to_string(project.join("programs/snake.asm")).unwrap();
    assert_eq!(snake.matches("LD RANDOM").count(), 2);
    fs::write(project.join("programs/fibonacci.asm"), snake.replace("LD RANDOM", "LDI 3")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "400"]);
    assert!(output.status.success(), "{stderr}");
    let on: Vec<u32> = stdout.lines().filter_map(|l| l.strip_prefix("screen 32 x 32, ")?.split(' ').next()?.parse().ok()).collect();
    // The food is one more pixel. Between two steps, the tail cell is clear and the head cell is not.
    assert_eq!(on.iter().max(), Some(&6), "{on:?}");
    assert!(on[on.len() - 6..].iter().all(|n| (5..=6).contains(n)), "{on:?}");
    // The monitor view builds. Without a window, it draws nothing (15.8).
    let (opened, toolchain, control) = (Project::open(&project).unwrap(), Toolchain::detect().unwrap(), Control::default());
    let library = opened.build_view(&toolchain, "monitor", Mode::Bake).unwrap();
    let mut view = View::load(&library, &project).unwrap();
    let graph = snapshot(opened.graph(), &[], &[], &[]);
    let tree = view.ui(0, "", &graph, None).unwrap().tree;
    assert!(tree.starts_with("viewport	Monitor	320
label	No frame yet."), "{tree}");
    // A view of the build mode of the cluster links the same frame type, so it reads the frame of
    // the sprite chip. The first read only asks for copies (15.8 rule 3).
    let mut cluster = opened.check().unwrap().build(&toolchain, Mode::Bake).unwrap();
    cluster.set_params(opened.graph());
    assert!(cluster.run(&control).ok);
    let mut frame = || {
        assert!(cluster.frame(&control).ok);
        view.ui(0, "", &graph, Some(&mut cluster.frame_part())).unwrap().tree
    };
    assert!(frame().contains("No frame yet."));
    let tree = frame();
    assert!(tree.contains("label	32 x 32, ") && tree.contains("Click the monitor"), "{tree}");
}

/// A program for the gpu read port and the sprite chip.
const SPRITE: &str = "LDI 3
ST 0xF1      ; X = 3
ST 0xF2      ; Y = 3
LDI 5
ST 0xF3      ; color 5 at 3,3, and X goes to 4
LDI 3
ST 0xF1
LD 0xF3      ; A = 5
OUT
ADDI 1
OUT
LDI 14
ST 0xFC      ; sprite 0 gets color 14
LDI 10
ST 0xFA
ST 0xFB
end: JMP end
";

/// Runs the example and compares each frame with a model of the CPU. Gives the values of OUT.
fn check(project: &Path, frames: u32, program: &[(u8, u8)]) -> Vec<u8> {
    let (output, stdout, stderr) = run(project, &["--frames", &frames.to_string()]);
    assert!(output.status.success(), "{stderr}");
    let (expected, shown, lcd) = model(program, frames);
    let (lcd_lines, lines): (Vec<_>, Vec<_>) = stdout.lines().partition(|l| l.starts_with("lcd "));
    assert_eq!(lines, expected);
    // After 32 characters, each new line moves the lines up.
    let (old, line) = lcd.split_at(lcd.len() - (lcd.len() - 17) % 16 - 1);
    assert_eq!(lcd_lines.last(), Some(&format!("lcd |{}|{line:<16}|", &old[old.len() - 16..]).as_str()));
    shown
}

/// The lines that the CLI writes, the values of OUT and the LCD characters.
///
/// Each frame is one step: readout A shows the count, and readout B the count plus 3. With
/// b = 0011, the four full adders get all 8 input vectors. The register of readout C loads its
/// value plus 5 at each step. The memory of readout D writes C to address n % 4, so D shows C of 4
/// steps before. Readouts E, F and G show the PC, A and OUT of the CPU, which runs one instruction
/// at each tick. The displays show the count, and OUT as two hex digits, segments a to g. One step
/// after OUT loads, the LCD writer writes the two hex digits of OUT and a space, one each step.
fn model(program: &[(u8, u8)], frames: u32) -> (Vec<String>, Vec<u8>, String) {
    let (mut pc, mut acc, mut x, mut out, mut carry, mut zero, mut ram) = (0u8, 0u8, 0u8, 0u8, false, false, [0u8; 256]);
    let (mut lines, mut shown, mut lcd, mut strobes) = (Vec::new(), Vec::new(), String::new(), 0u8);
    // A readout or a display writes a line only when its value changes from the last value.
    let mut last: Vec<String> = [0; 7].map(|n: u8| format!("{n:08b} {n}")).into_iter().chain(std::iter::repeat_n("0000000".to_owned(), 3)).collect();
    let hex = |d: u8| char::from_digit(d.into(), 16).unwrap().to_ascii_uppercase();
    for n in 0..frames {
        // Bit i of `strobes` is on when OUT loaded i + 1 steps before.
        if let Some(i) = (0..3).find(|i| strobes >> i & 1 != 0) {
            lcd.push([hex(out >> 4), hex(out % 16), ' '][i]);
        }
        let readouts = [n % 16, (n + 3) % 16, n * 5 % 16, n.saturating_sub(4) * 5 % 16, pc.into(), acc.into(), out.into()];
        let digits = [n as usize % 16, usize::from(out >> 4), usize::from(out % 16)].map(|d| SEGMENTS[d].to_string());
        for (was, now) in last.iter_mut().zip(readouts.map(|n| format!("{n:08b} {n}")).into_iter().chain(digits)) {
            if *was != now {
                lines.push(now.clone());
                *was = now;
            }
        }
        if n % EVERY != 0 {
            strobes = strobes << 1 & 7;
            continue;
        }
        let (op, k) = program.get(usize::from(pc)).copied().unwrap_or((0, 0));
        // STX and LDX add X to the address. The programs read no IO port.
        let at = usize::from(if matches!(op, 9 | 10) { k.wrapping_add(x) } else { k });
        let operand = if op < 4 { k } else { ram[at] };
        pc = pc.wrapping_add(1);
        let result = match op {
            0 | 4 | 10 => Some(operand),
            1 | 5 => {
                let (sum, c) = acc.overflowing_add(operand);
                carry = c;
                Some(sum)
            }
            2 | 6 => {
                let (difference, borrow) = acc.overflowing_sub(operand);
                carry = !borrow;
                Some(difference)
            }
            3 | 7 => Some(acc & operand),
            8 | 9 if at < 0xF0 => {
                ram[at] = acc;
                None
            }
            11 => {
                x = acc;
                None
            }
            12 => {
                out = acc;
                shown.push(acc);
                None
            }
            13 => {
                pc = k;
                None
            }
            14 | 15 if [carry, zero][usize::from(op - 14)] => {
                pc = k;
                None
            }
            _ => None,
        };
        if let Some(value) = result {
            (acc, zero) = (value, value == 0);
        }
        strobes = (strobes << 1 | u8::from(op == 12)) & 7;
    }
    (lines, shown, lcd)
}
