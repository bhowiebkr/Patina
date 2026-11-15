use super::*;

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
    send_value(&rep, 0, |out| (&&&Probe(&2.5_f64)).text(out));
    send_value(&rep, 1, |out| (&&&Probe(&NoDebug)).text(out));
    send_value(&rep, 2, |out| (&&&Probe(&Hex(10))).text(out));
    let long = "\u{e9}".repeat(TEXT_LEN);
    send_value(&rep, 3, |out| (&&&Probe(&long)).text(out));
    send_value(&rep, 4, |_| panic!("no text"));
    assert_eq!(sent[0], (0, VALUE, "2.5".to_owned()));
    assert!(sent[1].2.ends_with("NoDebug (0 bytes)"), "{}", sent[1].2);
    assert_eq!(sent[2].2, "0x0a");
    // The quote and 2047 two-byte characters fill 4095 bytes. The next character does not fit.
    assert_eq!(sent[3].2, format!("\"{}...", "\u{e9}".repeat(TEXT_LEN / 2 - 1)));
    assert_eq!(sent[4].2, "the text of the value panicked");
}

#[test]
fn a_panic_drops_the_state_and_an_error_keeps_it() {
    let mut sent: Vec<(u32, u32, String)> = Vec::new();
    let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
    let state = State::new();
    *state.lock() = Some(5);
    assert!(!finish_state(Ok(Err("bad".to_owned())), &mut *state.lock(), &rep, 1));
    assert_eq!(*state.lock(), Some(5));
    assert!(!finish_state(catch_unwind(|| -> Result<(), String> { panic!("broken") }), &mut *state.lock(), &rep, 1));
    assert_eq!(*state.lock(), None);
    assert_eq!(sent, [(1, ERROR, "bad".to_owned()), (1, PANIC, "panic: broken".to_owned())]);
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
    send_lanes(&rep, 2, || Lanes::bits(4, 0b0110).label("a b"));
    send_lanes(&rep, 3, || Lanes::bits(0, 1));
    send_lanes(&rep, 4, || panic!("no lanes"));
    send_lanes(&rep, 5, || Lanes::bits(99, u64::MAX).label("x"));
    assert_eq!(sent[..2], [(2, LANES, "0110 a b".to_owned()), (3, LANES, "1 ".to_owned())]);
    assert_eq!(sent[2], (5, LANES, format!("{} x", "1".repeat(64))));
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
                rep.time(7, std::time::Instant::now());
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
            if let Cmd::Task(_) = cmd {
                log_line("INFO", &format_args!("before the first node"));
                rep.enter(3, true);
                for i in 0..=LOG_LINES {
                    log_line("WARN", &format_args!("line {i}"));
                }
            }
            true
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
    log_line("INFO", &format_args!("outside a run"));
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
