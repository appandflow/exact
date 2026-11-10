//! A reader's scroll drag: one `scrolldragstart`, then one `scrolldragend`
//! with UIKit's units and sign, after the drag's own `scroll`s.
use super::*;

fn boot(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    for _ in 0..8 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn log(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
    node.props.str(PropId::Text).unwrap_or_default().to_string()
}

/// A scroller 300 tall over 1,000 of content that logs its drag: `s` a
/// start, `e(vx,vy,left,top)` an end, `-` for a target it has none of, and
/// `.` for each `scroll`.
const LIST: &str = r#"fn at(n: option<number>): string = match n { case some(v) => `${v}`, case none => "-" }

component App
  state log = ""
  action start
    log = log + "s"
  action moved(x: number, y: number)
    log = log + "."
  action ended(e: ScrollDragEvent)
    log = log + `e(${e.velocityX},${e.velocityY},${at(e.targetLeft)},${at(e.targetTop)})`
  view
    column width=400 height=400
      scroll testId="list" width=400 height=300 scrolldragstart=start scrolldragend=ended scroll=moved
        box width=400 height=1000
      text log testId="log"
"#;

/// The reader is told of the list, as at a touch's down.
fn told(p: &mut Presenter<()>) -> ViewId {
    let list = id(p, "list");
    p.reader_under(200., 100., 1.);
    assert!(p.reader_knows(list));
    list
}

#[test]
fn a_drag_starts_once_and_ends_with_uikits_velocity_and_target() {
    let mut p = boot(LIST);
    let list = told(&mut p);
    assert_eq!(p.reader_drag_began(list), None);
    assert_eq!(log(&p), "s");
    assert_eq!(p.reader_scrolled(list, (0., 40.), 0), Some((0., 40.)));
    assert_eq!(p.reader_scrolled(list, (0., 60.), 0), Some((0., 60.)));
    // The finger moved up at 1,500 points a second: the offset grows, so
    // the velocity is positive, in points a millisecond.
    assert_eq!(
        p.reader_drag_ended(list, (0., 1500.), Some((0., 450.))),
        None
    );
    assert_eq!(log(&p), "s..e(0,1.5,0,450)");
}

#[test]
fn a_target_is_clamped_to_the_range_and_none_without_one() {
    let mut p = boot(LIST);
    let list = told(&mut p);
    assert_eq!(
        p.reader_drag_ended(list, (0., -800.), Some((0., -90.))),
        None
    );
    assert_eq!(
        p.reader_drag_ended(list, (0., 3000.), Some((0., 5000.))),
        None
    );
    assert_eq!(p.reader_drag_ended(list, (20., 0.), None), None);
    assert_eq!(
        log(&p),
        "e(0,-0.8,0,0)e(0,3,0,700)e(0.02,0,-,-)",
        "the range is 1,000 less the 300 port"
    );
}

#[test]
fn only_a_known_shown_scroller_with_the_handler_hears_it() {
    let mut p = boot(LIST);
    let list = id(&p, "list");
    // Never told of to the reader: nothing.
    assert_eq!(p.reader_drag_began(list), None);
    assert_eq!(p.reader_drag_began(PAGE), None);
    assert_eq!(log(&p), "");
    let mut quiet = boot(&LIST.replace(" scrolldragstart=start scrolldragend=ended", ""));
    let list = told(&mut quiet);
    assert_eq!(quiet.reader_drag_began(list), None);
    assert_eq!(quiet.reader_drag_ended(list, (0., 100.), None), None);
    assert_eq!(log(&quiet), "");
    assert!(p
        .reader_drag_ended(id(&p, "list"), (f64::NAN, 0.), None)
        .is_some());
}

#[test]
fn a_wheel_sends_no_drag() {
    let mut p = boot(LIST);
    let list = told(&mut p);
    p.wheel(list, 0., 120.).unwrap();
    assert!(
        !log(&p).contains('s') && !log(&p).contains('e'),
        "{}",
        log(&p)
    );
}
