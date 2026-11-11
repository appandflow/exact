//! A range under the pointer and the keys (LLP 1069.001 D7), as HTML's
//! `input type="range"` behaves in a browser.
use super::*;
use std::path::PathBuf;

const LEVEL: &str = r#"component App
  state level = 50
  state log = ""
  action slide(v: number)
    level = v
    log = `${log}i${v} `
  action settle(v: number)
    log = `${log}c${v} `
  view
    column width=400
      box DIRECTION width=400
        input type="range" min=0 max=100 step="any" value=level input=slide change=settle testId="r" width=200
"#;

fn boot(source: &str) -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn named(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn level(p: &Presenter<()>) -> f64 {
    p.host.runner().slot("level").unwrap().as_number().unwrap()
}

/// The range's value after a mouse press and release `fraction` of the way
/// across its box, from the left.
fn pressed_at(direction: &str, fraction: f32) -> f64 {
    let mut p = boot(&LEVEL.replace("DIRECTION", direction));
    let (x, y, w, h) = p.box_of(named(&p, "r")).unwrap().rect;
    let at = (x + w * fraction, y + h / 2.0);
    let now = p.host.now();
    p.pointer_down(at.0, at.1, now).unwrap();
    p.pointer_up(at.0, at.1, now).unwrap();
    level(&p)
}

/// Runs the calling test again in a child process that shows the neutral
/// look, which is the whole process's: true in that child.
fn in_neutral_child() -> bool {
    const CHILD: &str = "EXACT_RANGE_NEUTRAL_TEST";
    if std::env::var_os(CHILD).is_some() {
        crate::app::show_neutral_look();
        return true;
    }
    let name = std::thread::current().name().unwrap().to_owned();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

/// A right-to-left range has its minimum at the right, where it is painted,
/// so a press four fifths of the way across sets about a fifth of it (a
/// browser's `<div dir="rtl"><input type="range">`).
fn mirrored_press() {
    let ltr = pressed_at(r#"direction="ltr""#, 0.8);
    let rtl = pressed_at(r#"direction="rtl""#, 0.8);
    assert!(ltr > 75.0 && ltr < 85.0, "{ltr}");
    assert!((ltr + rtl - 100.0).abs() < 1e-6, "ltr {ltr}, rtl {rtl}");
}

#[test]
fn a_press_on_a_right_to_left_range_sets_the_value_under_it() {
    mirrored_press();
}

#[test]
fn a_press_on_a_right_to_left_range_sets_the_value_under_it_in_the_neutral_look() {
    if in_neutral_child() {
        mirrored_press();
    }
}
