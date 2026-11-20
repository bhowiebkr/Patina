use super::*;
use crate::{Lanes, Mode, Thumbnail, Ui, Viewer};

struct NoDebug;

#[derive(Debug)]
struct Hex(u8);

impl Viewer for Hex {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(out, "0x{:02x}", self.0)
    }
}

unsafe extern "C" fn keep(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
    // SAFETY: the test gives a `Vec` as the context, and a text that is valid during the call.
    let (sent, bytes) = unsafe { (&mut *ctx.cast::<Vec<(u32, u32, String)>>(), std::slice::from_raw_parts(text, len)) };
    sent.push((node, kind, String::from_utf8(bytes.to_vec()).unwrap()));
}

#[test]
#[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
fn values_use_the_viewer_then_debug_then_the_type_name() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    send_value(&rep, 0, &2.5_f64, (&&&Probe(&2.5_f64)).view_fns());
    send_value(&rep, 1, &NoDebug, (&&&Probe(&NoDebug)).view_fns());
    send_value(&rep, 2, &Hex(10), (&&&Probe(&Hex(10))).view_fns());
    let long = "\u{e9}".repeat(TEXT_LEN);
    send_value(&rep, 3, &long, (&&&Probe(&long)).view_fns());
    send_value(&rep, 4, &(), (|_, _| panic!("no text"), None));
    assert_eq!(sent[0], (0, VALUE, "2.5".to_owned()));
    assert!(sent[1].2.ends_with("NoDebug (0 bytes)"), "{}", sent[1].2);
    assert_eq!(sent[2].2, "0x0a");
    // The quote and 2047 two-byte characters fill 4095 bytes. The next character does not fit.
    assert_eq!(sent[3].2, format!("\"{}...", "\u{e9}".repeat(TEXT_LEN / 2 - 1)));
    assert_eq!(sent[4].2, "the text of the value panicked");
}

/// A value with a thumbnail of 2 x 1 pixels and widgets, with neither, or with panics.
struct Dot(u8);

/// The widgets of `Dot(1)`.
fn dot_widgets(ui: &mut Ui) {
    ui.label(if ui.mode() == Mode::Large { "large" } else { "other" });
    ui.plot("p", &[1.0, 2.0], 0.0..=2.0);
}

impl Viewer for Dot {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        out.write_str("dot")
    }

    fn thumbnail(&self, out: &mut Thumbnail<'_>) {
        match self.0 {
            0 => {}
            1 => out.pixels(2, 1).copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]),
            _ => panic!("no picture"),
        }
    }

    fn show(&self, ui: &mut Ui) {
        match self.0 {
            0 => {}
            1 => dot_widgets(ui),
            _ => panic!("no widgets"),
        }
    }
}

#[test]
#[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
fn a_viewer_sends_its_thumbnail_and_widgets_after_the_text() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    for (i, dot) in [Dot(1), Dot(0), Dot(2), Dot(1)].iter().enumerate() {
        send_value(&rep, i as u32, dot, (&&&Probe(dot)).view_fns());
    }
    send_value(&rep, 4, &Hex(1), (&&&Probe(&Hex(1))).view_fns());
    let kinds: Vec<_> = sent.iter().map(|&(i, k, _)| (i, k)).collect();
    assert_eq!(kinds, [(0, VALUE), (0, THUMB), (0, VIEW), (1, VALUE), (2, VALUE), (3, VALUE), (3, THUMB), (3, VIEW), (4, VALUE)]);
    assert_eq!(sent[1].2, "2 1 AQIDBAUGBwg=");
    // The second value starts with an empty buffer, in the Inspector size.
    let mut ui = Ui::new(1, "");
    dot_widgets(&mut ui);
    assert!(ui.tree().contains("large"));
    assert_eq!((sent[2].2.as_str(), sent[7].2.as_str()), (ui.tree(), ui.tree()));
    let mut buf = Vec::new();
    assert_eq!(Thumbnail::new(&mut buf).pixels(100, 2).len(), 64 * 2 * 4, "a long side becomes MAX");
}

#[test]
#[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
fn an_audio_block_sends_its_wave_after_the_text() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    // Part 1 of 64 has the samples 4 to 7, and its largest sample is -0.5.
    let block = crate::AudioBlock::from_fn(|i| match i {
        0 => 2.0,
        5 => -0.5,
        6 => 0.25,
        _ => 0.0,
    });
    send_value(&rep, 0, &block, (&&&Probe(&block)).view_fns());
    let kinds: Vec<_> = sent.iter().map(|&(i, k, _)| (i, k)).collect();
    assert_eq!(kinds, [(0, VALUE), (0, WAVE)]);
    assert_eq!(sent[0].2, "AudioBlock(peak 2.000)");
    let wave: Vec<i32> = sent[1].2.split(' ').map(|s| s.parse().unwrap()).collect();
    assert_eq!((wave.len(), &wave[..3]), (WAVE_LEN, &[100, -50, 0][..]));
    let mut text = String::new();
    assert!(wave_text(&[0.1, f32::NAN, -0.004], &mut text));
    assert_eq!(text, "10 0 0");
    assert!(!wave_text(&[], &mut text));
}

#[test]
fn only_a_catch_with_a_report_keeps_the_place_of_the_panic() {
    // The inner catch gives no report, so its panic does not set the place.
    assert!(catch_quiet(|| catch_unwind(|| panic!("inner")).is_err()).unwrap());
    assert_eq!(panic_text(&"no place"), "panic: no place");
}

#[test]
fn a_panic_drops_the_state_and_an_error_keeps_it() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    let state = State::new();
    *state.lock() = Some(5);
    assert!(!finish_state(Ok(Err("bad".to_owned())), &mut *state.lock(), &rep, 1));
    assert_eq!(*state.lock(), Some(5));
    assert!(!finish_state(catch_quiet(|| -> Result<(), String> { panic!("broken") }), &mut *state.lock(), &rep, 1));
    assert_eq!(*state.lock(), None);
    assert_eq!(sent[0], (1, ERROR, "bad".to_owned()));
    // The report has the place of the panic, and the standard error does not get the message.
    let place = format!("panic: broken ({}:", file!());
    assert!(sent[1].1 == PANIC && sent[1].2.starts_with(&place), "{:?}", sent[1]);
    assert_eq!(sent.len(), 2);
}

#[test]
fn the_control_keeps_the_node_while_its_gui_draws() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let control = Control::default();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
    let (state, during) = (State::new(), std::cell::Cell::new(None));
    assert!(draw(&state, || 0, |_: &mut i32, _| during.set(control.node()), (0, ""), &rep, 3));
    assert_eq!((during.get(), control.node()), (Some(3), None));
    assert_eq!(sent.iter().filter(|s| s.1 == UI).count(), 1);
}

#[test]
fn a_gui_writes_its_widget_lines_into_the_buffer_of_the_last_gui() {
    unsafe extern "C" fn place(ctx: *mut c_void, _: u32, kind: u32, text: *const u8, _: usize) {
        // SAFETY: the test gives a `Vec` as the context.
        let places = unsafe { &mut *ctx.cast::<Vec<usize>>() };
        if kind == UI {
            places.push(text.addr());
        }
    }
    let mut places: Vec<usize> = Vec::new();
    let control = Control::default();
    let rep = Reporter { ctx: (&raw mut places).cast(), report: place, control: &raw const control };
    let buffer = String::with_capacity(1024);
    let at = buffer.as_ptr().addr();
    TREE.set(buffer);
    let (state, gui) = (State::new(), |_: &mut i32, ui: &mut Ui| ui.label("x"));
    assert!(draw(&state, || 0, gui, (0, ""), &rep, 3));
    // A user input draws two times, with the same buffer.
    assert!(draw(&state, || 0, gui, (0, "0 1"), &rep, 3));
    let buffer = TREE.take();
    assert_eq!((places, buffer.as_ptr().addr()), (vec![at, at], at));
}

#[test]
fn a_gui_sends_nothing_while_a_run_holds_the_state() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let control = Control::default();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
    let state = State::new();
    let held = state.lock();
    assert!(draw(&state, || 0, |_: &mut i32, _| panic!("the state is held"), (0, ""), &rep, 3));
    assert_eq!((control.node(), sent.len()), (None, 0));
    drop(held);
    assert!(draw(&state, || 0, |_: &mut i32, _| {}, (0, ""), &rep, 3));
    assert_eq!(sent.iter().filter(|s| s.1 == UI).count(), 1);
}

#[test]
fn node_calls_catch_errors_and_panics_with_the_copies_in_the_catch() {
    struct Bad;
    impl Clone for Bad {
        fn clone(&self) -> Bad {
            panic!("clone")
        }
    }
    fn add(a: &u8, b: u8, c: String, d: &mut Vec<u8>) -> u8 {
        d.push(a + b);
        a + b + c.len() as u8
    }
    fn number(text: String) -> Result<u8, std::num::ParseIntError> {
        text.parse()
    }
    fn bump(state: &mut u8, by: u8) -> u8 {
        assert!(by > 0, "zero");
        *state += by;
        *state
    }
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    let (mut ok, mut held) = (true, None);
    let r = super::run(add, (&1, Take(2), Cloned(&"xy".to_owned()), Hold(&mut held, Take(Vec::new()))));
    assert_eq!((done(r, &rep, 0, &mut ok), held, ok), (Some(5), Some(vec![3]), true));
    assert_eq!(done(try_run(number, (Cloned(&"x".to_owned()),)), &rep, 1, &mut ok), None);
    assert_eq!(done(super::run(drop::<Bad>, (Cloned(&Bad),)), &rep, 2, &mut ok), None);
    assert!(!ok);
    let (mut ok, mut state) = (true, None);
    assert_eq!(done_state(super::run(bump, (Init(&mut state, || 10), Take(1))), &mut state, &rep, 3, &mut ok), Some(11));
    assert_eq!((state, ok), (Some(11), true));
    assert_eq!(done_state(super::run(bump, (Init(&mut state, || 10), Take(0))), &mut state, &rep, 3, &mut ok), None);
    assert_eq!((state, ok), (None, false));
    let errors: Vec<_> = sent.iter().map(|(node, kind, text)| (*node, *kind, text.split(" (").next().unwrap())).collect();
    assert_eq!(errors, [(1, ERROR, "ParseIntError { kind: InvalidDigit }"), (2, PANIC, "panic: clone"), (3, PANIC, "panic: zero")]);
}

#[test]
fn a_state_moves_to_a_new_cluster() {
    fn make() -> Cluster {
        let state = State::new();
        let f = typed(move |cmd, rep| match cmd {
            Cmd::Task(_) => {
                *state.lock().get_or_insert(0) += 1;
                true
            }
            Cmd::End(_) => true,
            Cmd::Take(0, out) => take_state(&state, out),
            Cmd::Put(0, moved) => put_state(&state, moved, rep, 0),
            Cmd::Ui(..) => {
                rep.send(0, UI, &format!("{:?}", *state.lock()));
                true
            }
            _ => false,
        });
        cluster(f, false)
    }
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    let (old, new) = (new_cluster(make), new_cluster(make));
    // SAFETY: the clusters come from `new_cluster` and are freed one time. The moved value
    // comes from `take` of a cluster of the same closure, so its type is the state type.
    unsafe {
        assert!(take(old, 0, &rep).is_null(), "no state before the first run");
        assert!(run(old, &rep) && run(old, &rep) && run(new, &rep));
        let value = take(old, 0, &rep);
        assert!(!value.is_null() && take(old, 0, &rep).is_null());
        assert!(put(new, 0, value, &rep));
        assert!(run(new, &rep) && ui(new, 0, 0, "".as_ptr(), 0, &rep));
        free(old);
        free(new);
    }
    assert_eq!(sent, [(0, UI, "Some(3)".to_owned())]);
}

#[test]
fn lanes_send_the_bits_msb_first_and_the_label() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    send_lanes(&rep, 2, &0b0110, |v| Lanes::bits(4, *v).label("a b"));
    send_lanes(&rep, 3, &1, |v| Lanes::bits(0, *v));
    send_lanes(&rep, 4, &(), |_| panic!("no lanes"));
    send_lanes(&rep, 5, &u64::MAX, |v| Lanes::bits(99, *v).label("x"));
    assert_eq!(sent[..2], [(2, LANES, "0110 a b".to_owned()), (3, LANES, "1 ".to_owned())]);
    assert_eq!(sent[2], (5, LANES, format!("{} x", "1".repeat(64))));
}

#[test]
fn a_quiet_run_sends_no_values_and_no_times_but_logs() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let control = Control::default();
    control.set_quiet(true);
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &control };
    send_value(&rep, 0, &1_u8, (&&Probe(&1_u8)).view_fns());
    send_lanes(&rep, 1, &1, |v| Lanes::bits(1, *v));
    rep.time(2, rep.start());
    rep.send(3, LOG, "hi");
    control.set_quiet(false);
    send_value(&rep, 4, &1_u8, (&&Probe(&1_u8)).view_fns());
    assert_eq!(sent, [(3, LOG, "hi".to_owned()), (4, VALUE, "1".to_owned())]);
}

/// Runs task 0 and ends the run.
///
/// # Safety
///
/// The same as for `task`.
unsafe fn run(state: *mut Cluster, rep: &Reporter) -> bool {
    // SAFETY: the caller keeps the rules of `task`, and they include the rules of `end`.
    unsafe { task(state, 0, rep) & end(state, 0, rep) }
}

#[test]
#[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
fn only_sync_values_let_a_cluster_run_in_parallel() {
    assert!((&&check(&Shared::new(String::new()))).sync());
    assert!(!(&&check(&Shared::new(Cell::new(1)))).sync());
    assert!(!(&&check(&Shared::new(std::rc::Rc::new(1)))).sync());
    let shared = Shared::new(Some(1));
    *shared.write() = None;
    assert_eq!(*shared.read(), None);
}

#[test]
fn stop_and_missing_inputs_start_no_node_call() {
    fn make() -> Cluster {
        let f = typed(|cmd, rep| {
            let Cmd::Task(0) = cmd else { return matches!(cmd, Cmd::End(_)) };
            // SAFETY: the test gives a control that is valid during the run.
            let node = || unsafe { &*rep.control }.node();
            let skipped = !rep.enter(8, false) && node().is_none();
            let started = rep.enter(7, true);
            if started {
                rep.time(7, rep.start());
            }
            skipped && started != stop_requested() && node() == started.then_some(7)
        });
        cluster(f, true)
    }
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let control = Control::default();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
    let state = new_cluster(make);
    // SAFETY: the cluster comes from `new_cluster` and is freed one time. The reporter and the
    // control are valid during each run.
    unsafe {
        assert!(parallel(state));
        assert!(!task(state, 1, &rep));
        assert!(run(state, &rep));
        assert_eq!(control.node(), None);
        control.set_stop(true);
        assert!(run(state, &rep));
        free(state);
    }
    let time = sent.remove(1);
    assert_eq!((time.0, time.1), (7, TIME));
    assert!(time.2.parse::<u64>().is_ok(), "{}", time.2);
    assert_eq!(sent, [(8, SKIPPED, String::new()), (8, STOPPED, String::new()), (7, STOPPED, String::new())]);
    assert!(!stop_requested(), "the flag is not visible outside a run");
}

#[test]
fn log_lines_go_to_the_node_that_runs() {
    fn make() -> Cluster {
        let f = typed(|cmd, rep| {
            let mut ok = true;
            if let Cmd::Task(_) = cmd {
                ok = !log_to_run("INFO", &format_args!("before the first node"));
                rep.enter(3, true);
                for i in 0..=LOG_LINES {
                    log_line("WARN", &format_args!("line {i}"));
                }
                ok &= log_to_run("WARN", &format_args!("after the limit"));
                // A thread that the node starts has no node, so its record goes to the standard error.
                ok &= !std::thread::scope(|s| s.spawn(|| log_to_run("INFO", &format_args!("thread"))).join().unwrap());
            }
            ok
        });
        cluster(f, false)
    }
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let control = Control::default();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
    let state = new_cluster(make);
    // SAFETY: the same as in `stop_and_missing_inputs_start_no_node_call`.
    unsafe {
        assert!(run(state, &rep));
        assert!(run(state, &rep));
        free(state);
    }
    assert!(!log_to_run("INFO", &format_args!("outside a run")));
    let max = LOG_LINES as usize;
    assert_eq!(sent.len(), 2 * (max + 1));
    assert_eq!(sent[0], (3, LOG, "WARN line 0".to_owned()));
    assert_eq!(sent[max - 1].2, format!("WARN line {}", max - 1));
    assert!(sent[max].2.starts_with("WARN the run has more than"), "{}", sent[max].2);
    assert_eq!(sent[max + 1].2, "WARN line 0", "each run counts its own lines");
}

#[test]
fn a_link_reads_the_value_of_the_other_cluster() {
    let value = std::sync::Arc::new(Shared::new(None));
    let link = Link::new();
    link.witness(0_u32);
    assert_eq!(*link.read(), None);
    let mut out = std::ptr::null_mut();
    assert!(export_link(&value, &mut out));
    assert!(import_link(&link, Moved(out)));
    *value.write() = Some(4);
    assert_eq!(*link.read(), Some(4));
    assert!(export_link(&value, &mut out) && !import_link(&link, Moved(out)), "a link takes one value");
    assert_eq!(std::sync::Arc::strong_count(&value), 2);
}

#[test]
fn the_base_calls_a_node_in_the_slot() {
    unsafe extern "C" fn call_slot(state: *const c_void, node: u32, args: *const *mut c_void, len: usize, rep: *const Reporter) -> bool {
        // SAFETY: the test gives the slot cluster and the arguments of `Slot::call`.
        unsafe { call(state.cast(), node, args, len, rep) }
    }
    fn make_slot() -> Cluster {
        let f = typed(|cmd, _| match cmd {
            // SAFETY: the base gives a `&f64`, an `Option<String>`, a `&mut f64` and an
            // `Option<usize>` (below).
            Cmd::Call(7, args) => unsafe {
                let text: String = take_arg(args[1]);
                *mut_arg::<f64>(args[2]) += read_arg::<f64>(args[0]);
                put_result(args[3], text.len() + NODE.get() as usize);
                true
            },
            _ => false,
        });
        cluster(f, false)
    }
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    let base = Slot::new();
    let (value, mut text, mut total, mut out) = (2.5_f64, Some("abc".to_owned()), 1.0_f64, None::<usize>);
    assert!(!base.call(7, &[], &rep), "no slot yet");
    let state = new_cluster(make_slot);
    let slot = SlotRef { state: state.cast(), call: call_slot };
    assert!(base.set(&slot));
    assert!(base.call(7, &[arg(&value), arg_mut(&mut text), arg_mut(&mut total), arg_mut(&mut out)], &rep));
    assert!(!base.call(8, &[], &rep), "the slot has no node 8");
    // SAFETY: the cluster comes from `new_cluster` and is freed one time.
    unsafe { free(state) };
    assert_eq!((text, total, out), (None, 3.5, Some(10)));
    assert_eq!(sent, [(7, ERROR, "the node has no build in the slot".to_owned())]);
}

#[test]
fn shared_gives_one_value_and_makes_it_one_time() {
    struct Once(u32);
    let first: &Once = shared(|| Once(1));
    let second: &Once = shared(|| Once(2));
    assert!(std::ptr::eq(first, second));
    assert_eq!(second.0, 1);
}

#[test]
fn an_asset_is_in_the_node_folder_of_the_project_root() {
    let path = |root| asset_in(root, "/old/place/nodes/blur", "blur.wesl");
    assert_eq!(path(Some("/package/project")), std::path::Path::new("/package/project/nodes/blur/assets/blur.wesl"));
    assert_eq!(path(None), std::path::Path::new("/old/place/nodes/blur/assets/blur.wesl"));
}

#[test]
#[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
fn a_view_reads_a_kept_copy_after_it_asked_for_it() {
    let keeps = Keeps::new(2);
    // SAFETY: a non-null pointer of `read` points to a live copy until the next `keep`.
    let copy = |index| unsafe { keeps.read(index).cast::<Box<dyn std::any::Any + Send>>().as_ref() };
    (&&Probe(&vec![1_u8])).keep(&keeps, 0);
    assert!(copy(0).is_none(), "the first read only asks for copies");
    assert!(copy(0).is_none(), "no copy before the next run");
    (&&Probe(&vec![1_u8, 2])).keep(&keeps, 0);
    (&&Probe(&NoDebug)).keep(&keeps, 1);
    assert_eq!(copy(0).and_then(|c| c.downcast_ref::<Vec<u8>>()), Some(&vec![1, 2]));
    assert!(copy(0).unwrap().downcast_ref::<String>().is_none(), "another type gives none");
    assert!(copy(1).is_none() && copy(1).is_none(), "a type without Clone has no copy");
    assert!(copy(9).is_none());
}

#[test]
fn an_atom_keeps_all_bits_of_a_number() {
    let byte = Atom::new(Some(-5_i8));
    assert_eq!(*byte.read(), Some(-5));
    *byte.write() = None;
    assert_eq!(byte.get(), None);
    let _ = byte.write().insert(7);
    assert_eq!(byte.get(), Some(7));
    assert_eq!(Atom::new(-1.5_f32).get().to_bits(), (-1.5_f32).to_bits());
    assert_eq!(Atom::new(Some('\u{e9}')).get(), Some('\u{e9}'));
    assert_eq!(Atom::new(u64::MAX).get(), u64::MAX);
    assert!(Atom::new(true).get());
}

#[test]
fn a_net_reports_a_panic_and_runs_the_next_calls() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    let mut ran = Vec::new();
    let net = Net::new(&rep, &[4, 5, 6, 7]).unwrap();
    // The generated code has the same form: each call runs when `at` is at or before it.
    let ok = net.run(&mut |at, gate| {
        for i in at..4 {
            // The call at index 2 has no input, so it does not enter.
            if i != 2 {
                gate.enter([4, 5, 6, 7][i]);
            }
            assert!(i != 1 && i != 2, "gate {}", i + 4);
            ran.push(i);
        }
    });
    assert!(!ok);
    assert_eq!(ran, [0, 3]);
    // The panic of index 2 comes before it enters, so it goes to the node after the last panic.
    let panics: Vec<u32> = sent.iter().filter(|s| s.1 == PANIC).map(|s| s.0).collect();
    assert_eq!(panics, [5, 6]);
    assert_eq!(NODE.get(), 7);
    assert!(sent.iter().any(|s| s.1 == PANIC && s.2.contains("gate 5")));
}
