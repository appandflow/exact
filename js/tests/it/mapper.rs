//! A stream's mapper that returns a promise (LLP 1016.000 D1, D7; Charlie,
//! 2026-11-10), through the runner: each later turn of its call is a request
//! of its own, its value is the stream's when it settles, and what lands on
//! the stream meanwhile waits behind it in order.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Dispatch, Message, Outcome, Runner, SocketOp};
use std::time::Instant;

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mapper.hbc"));

const SRC: &str = r#"
component App
  state id = "a"
  resource feed = feed(id) as shape string
  action go(to: string)
    id = to
  view
    text feed testId="feed"
"#;

/// The runner on stream `id`, opened, and its ticket. `budget`: the turn
/// budget, or none (a functional test's: castle.rs says why).
fn boot(id: &str, budget: Option<f64>) -> (Runner<Module>, u64) {
    let mut m = Module::new(
        HBC.to_vec(),
        "test.mapper",
        "net.websocket wss://mapper.test\n",
    );
    m.set_budget_ms(budget.unwrap_or(f64::INFINITY));
    m.load().expect("the fixture loads");
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let mut r = Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let mut out = r.take_requests();
    if id != "a" {
        r.act("go", vec![Value::str(id)]).unwrap();
        out = r.take_requests();
    }
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].request.url, format!("wss://mapper.test/{id}"));
    let ticket = out[0].ticket;
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(open)).unwrap();
    assert_eq!(feed(&r), "open");
    (r, ticket)
}

fn feed(r: &Runner<Module>) -> String {
    r.resource("feed")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn text(r: &mut Runner<Module>, ticket: u64, data: &str) -> String {
    let m = Message {
        data: data.into(),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
    feed(r)
}

fn binary(r: &mut Runner<Module>, ticket: u64, bytes: Vec<u8>) -> String {
    let m = Message {
        binary: Some(bytes),
        ..Message::default()
    };
    r.fulfill(ticket, Outcome::Message(m)).unwrap();
    feed(r)
}

/// Run every request out as a host does a continuation, until none is: the
/// answer shown after each, and each landing's wall time.
fn turns(r: &mut Runner<Module>) -> (Vec<String>, Vec<f64>) {
    let (mut shown, mut times) = (Vec::new(), Vec::new());
    loop {
        let out = r.take_requests();
        if out.is_empty() {
            return (shown, times);
        }
        for request in out {
            let token = request
                .request
                .continuation
                .expect("a turn is a continuation");
            match r.dispatch_work(token) {
                Dispatch::Again => {
                    let started = Instant::now();
                    r.fulfill(request.ticket, Dispatch::again_outcome())
                        .unwrap();
                    times.push(started.elapsed().as_secs_f64() * 1e3);
                    shown.push(feed(r));
                }
                // A call let go with its stream has no work left.
                Dispatch::Missing => {}
                _ => panic!("a mapper's turn is asked again at once"),
            }
        }
    }
}

/// A later message's mapper call never overtakes an earlier one's: the
/// messages wait behind its turns, then each is mapped and committed in
/// arrival order. While a turn is out the app is not settled.
#[test]
fn a_later_message_waits_for_an_earlier_mappers_promise() {
    let (mut r, ticket) = boot("a", None);
    assert_eq!(text(&mut r, ticket, "yield 2 a"), "open");
    assert_eq!(text(&mut r, ticket, "yield 0 b"), "open");
    assert_eq!(text(&mut r, ticket, "yield 0 c"), "open");
    assert!(r.has_pending(), "a mapper turn is out");
    let (shown, _) = turns(&mut r);
    assert_eq!(
        shown,
        [
            "open",
            "start a end a",
            "start a end a start b end b",
            "start a end a start b end b start c end c",
        ]
    );
    assert!(!r.has_pending());
    // A mapper that settles in its first turn answers then, as before.
    assert_eq!(
        text(&mut r, ticket, "yield 0 d"),
        "start a end a start b end b start c end c start d end d"
    );
    assert!(r.take_requests().is_empty());
}

/// A rejection is a throw: the stream ends with its error, the resource
/// keeps its last value, and what waited behind it is dropped.
#[test]
fn a_rejection_ends_the_stream_with_its_error() {
    let (mut r, ticket) = boot("a", None);
    text(&mut r, ticket, "reject 1 x");
    text(&mut r, ticket, "yield 0 y");
    let (shown, _) = turns(&mut r);
    assert_eq!(shown, ["open"]);
    let failed = r.failed_resources();
    assert!(
        failed
            .iter()
            .any(|(name, why)| *name == "feed" && why.contains("rejected x")),
        "{failed:?}"
    );
    assert!(r.streams().is_empty(), "the stream ended");
    assert!(!r.has_pending());
}

/// What waits behind a mapper's turn is bounded (LLP 1016.000 D4): the
/// 4097th message ends the stream, saying so.
#[test]
fn a_stream_whose_mapper_falls_behind_ends() {
    let (mut r, ticket) = boot("a", None);
    text(&mut r, ticket, "yield 1 a");
    for i in 0..4096 {
        text(&mut r, ticket, &format!("yield 0 {i}"));
    }
    assert!(r.failed_resources().is_empty());
    text(&mut r, ticket, "yield 0 last");
    let failed = r.failed_resources();
    assert!(
        failed
            .iter()
            .any(|(name, why)| *name == "feed" && why.contains("fell behind: 4096 messages")),
        "{failed:?}"
    );
    assert!(r.streams().is_empty());
    turns(&mut r);
    assert_eq!(feed(&r), "open", "its pending call was let go with it");
}

/// The stream's end waits behind a pending call too, and its own mapper
/// call may await: the last value is the end's.
#[test]
fn the_end_waits_behind_a_pending_mapper_call() {
    let (mut r, ticket) = boot("a", None);
    text(&mut r, ticket, "yield 1 a");
    let closed = Outcome::Closed {
        code: 1000,
        reason: String::new(),
        was_clean: true,
    };
    r.fulfill(ticket, closed).unwrap();
    assert_eq!(feed(&r), "open");
    let (shown, _) = turns(&mut r);
    assert_eq!(shown.last().unwrap(), "start a end a close 1000");
    assert!(r.streams().is_empty());
    assert!(!r.has_pending());
}

/// A stream let go with a mapper call pending: the call is let go with it,
/// its turn's landing is dropped, and the new stream maps on its own.
#[test]
fn a_stream_let_go_lets_its_pending_mapper_call_go() {
    let (mut r, ticket) = boot("a", None);
    text(&mut r, ticket, "yield 1 a");
    let turn = r.take_requests();
    assert_eq!(turn.len(), 1);
    r.act("go", vec![Value::str("b")]).unwrap();
    let out = r.take_requests();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].request.url, "wss://mapper.test/b");
    let landed = r
        .fulfill(turn[0].ticket, Dispatch::again_outcome())
        .unwrap();
    assert!(landed.is_none(), "its landing is dropped");
    let next = out[0].ticket;
    let open = Message {
        event: "open".into(),
        ..Message::default()
    };
    r.fulfill(next, Outcome::Message(open)).unwrap();
    let shown = text(&mut r, next, "yield 0 z");
    assert!(
        shown.ends_with("start z end z") && !shown.contains("end a"),
        "{shown}"
    );
}

/// What the mapper sends before its first `await` goes with the frame's
/// first commit, and a later turn sends as a turn of an answer does (the
/// amendment of 2026-10-18), in that order; fetch and timers are refused.
#[test]
fn a_later_turn_sends_and_fetch_and_timers_are_refused() {
    let (mut r, ticket) = boot("a", None);
    r.take_socket_sends();
    text(&mut r, ticket, "send 1 s");
    turns(&mut r);
    let ops: Vec<SocketOp> = (r.take_socket_sends().into_iter())
        .flat_map(|s| s.ops.into_iter().map(|(_, _, op)| op))
        .collect();
    let sent = |s: &str| SocketOp::Text(s.into());
    assert_eq!(ops, [sent("before s"), sent("after s")]);
    // (after a yield: in the frame's own turn a socket's mapper refuses fetch as it always has)
    text(&mut r, ticket, "fetch 1 f");
    let shown = turns(&mut r).0.pop().unwrap();
    assert!(
        shown.contains("fetch() is unavailable in a stream's mapper"),
        "{shown}"
    );
    text(&mut r, ticket, "timer 1 t");
    let shown = turns(&mut r).0.pop().unwrap();
    assert!(
        shown.contains("setTimeout() is unavailable here"),
        "{shown}"
    );
}

/// Discord's `zlib-stream`: one zlib stream across binary frames, each
/// ending in a sync flush, inflated frame by frame by one
/// `DecompressionStream` the mapper keeps.
#[test]
fn a_zlib_stream_inflates_frame_by_frame() {
    use miniz_oxide::deflate::core::{create_comp_flags_from_zip_params, CompressorOxide};
    use miniz_oxide::deflate::stream::deflate;
    use miniz_oxide::MZFlush;
    let (mut r, ticket) = boot("zlib", None);
    let mut c = CompressorOxide::new(create_comp_flags_from_zip_params(6, 15, 0));
    for i in 0..5 {
        let message = format!(
            "{{\"op\":0,\"s\":{i},\"t\":\"MESSAGE_CREATE\",\"d\":{{\"content\":\"{}\"}}}}",
            "hi ".repeat(i * 40)
        );
        let mut packet = vec![0; message.len() + 1024];
        let out = deflate(&mut c, message.as_bytes(), &mut packet, MZFlush::Sync);
        packet.truncate(out.bytes_written);
        assert_eq!(binary(&mut r, ticket, packet), message);
    }
}

/// 8 MB of JSON in one deflate frame (a large account's READY): inflated
/// whole in one turn, and in 64 KiB writes with a yield between, each a
/// turn of its own with its own budget. The default 100 ms budget applies.
#[test]
fn an_8_mb_frame_inflates_in_one_turn_or_many() {
    let mut json = String::from("[");
    for i in 0u64.. {
        if json.len() >= 8 << 20 {
            break;
        }
        json.push_str(&format!(
            "{{\"id\":\"{}\",\"name\":\"guild {i}\",\"members\":{}}},",
            1_000_000_007u64.wrapping_mul(i + 1),
            i % 977
        ));
    }
    let zlib = miniz_oxide::deflate::compress_to_vec_zlib(json.as_bytes(), 6);
    let want = format!("inflated {}", json.len());

    let (mut r, ticket) = boot("whole", None);
    let started = Instant::now();
    assert_eq!(binary(&mut r, ticket, zlib.clone()), want);
    let whole = started.elapsed().as_secs_f64() * 1e3;
    let budgeted = |id| boot(id, Some(100.0));
    let (mut r, ticket) = budgeted("whole");
    assert_eq!(
        binary(&mut r, ticket, zlib.clone()),
        want,
        "whole, within 100 ms"
    );

    let (mut r, ticket) = budgeted("split");
    let started = Instant::now();
    assert_eq!(binary(&mut r, ticket, zlib.clone()), "open");
    let first = started.elapsed().as_secs_f64() * 1e3;
    let (shown, times) = turns(&mut r);
    assert_eq!(shown.last().unwrap(), &want);
    let most = times.iter().copied().fold(first, f64::max);
    eprintln!(
        "8 MB JSON from {} KB of deflate: one turn {whole:.1} ms; in {} turns the longest {most:.1} ms",
        zlib.len() >> 10,
        times.len() + 1
    );
    assert_eq!(times.len() + 1, zlib.len().div_ceil(65536));
}
