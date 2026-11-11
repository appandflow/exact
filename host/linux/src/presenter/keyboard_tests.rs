//! The keyboard reaches and works what a pointer does, as on the web: Tab
//! stops at every form control HTML lists as focusable.
use super::*;

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

fn id(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The test id of the focus, or "-" for none.
fn focused(p: &Presenter<()>) -> String {
    let k = p.host.kernel();
    p.focus()
        .and_then(|f| k.node(f))
        .and_then(|n| n.props.str(PropId::TestId).map(str::to_owned))
        .unwrap_or_else(|| "-".into())
}

/// The focus after each of `keys`, pressed in turn.
fn walk(p: &mut Presenter<()>, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .map(|k| {
            p.key_down(k, 0.);
            focused(p)
        })
        .collect()
}

const FORM: &str = r#"component App
  state level = 5
  state pick = "a"
  action moved(v: number)
    level = v
  action picked(v: string)
    pick = v
  view
    column width=400 height=600
      input testId="text" height=30
      input type="range" min=0 max=10 value=level input=moved change=moved testId="range" width=200
      input type="checkbox" testId="check"
      select value=pick change=picked aria-label="Pick" testId="select"
        option "A" value="a"
        option "B" value="b"
      input type="date" testId="date"
      input type="time" testId="time"
      input type="file" id="upload" testId="file"
      button "Save" testId="save" height=40
"#;

#[test]
fn tab_stops_at_a_range_a_select_and_a_file_control_as_htmls_focusable_areas() {
    let mut p = boot(FORM);
    // With no picker to open, a date or time field is no stop: its press
    // does nothing either (LLP 1069.001).
    assert_eq!(
        walk(&mut p, &["Tab"; 7]),
        ["text", "range", "check", "select", "file", "save", "text"]
    );
    // Where a press opens the platform's picker, the date and time are stops.
    let mut p = boot(FORM);
    p.agent = true;
    assert_eq!(
        walk(&mut p, &["Tab"; 9]),
        ["text", "range", "check", "select", "date", "time", "file", "save", "text"]
    );
    // A press focuses each, as Chrome's does.
    for name in ["range", "select", "file"] {
        let mut p = boot(FORM);
        let (x, y, w, h) = p.rect_of(id(&p, name)).unwrap();
        p.pointer_down(x + w / 2., y + h / 2., 0.).unwrap();
        p.pointer_up(x + w / 2., y + h / 2., 1.).unwrap();
        assert_eq!(focused(&p), name);
    }
}

#[test]
fn space_and_enter_at_a_file_or_date_control_click_it() {
    let source = r#"component App
  state presses = 0
  action pressed
    presses = presses + 1
  view
    column width=400 height=600
      input type="file" id="upload" press=pressed testId="file"
      input type="date" value="2026-11-10" testId="date"
"#;
    for key in [" ", "Enter"] {
        let mut p = boot(source);
        p.agent = true;
        let (file, date) = (id(&p, "file"), id(&p, "date"));
        let holds = |p: &Presenter<()>| {
            (p.host.runner().device_holds().iter())
                .map(|h| h.node)
                .collect::<Vec<_>>()
        };
        assert_eq!(walk(&mut p, &["Tab", key]), ["file", "file"]);
        assert_eq!(
            p.host.runner().slot("presses"),
            Some(&exact_runner::Value::Number(1.))
        );
        assert_eq!(holds(&p), [Some(file)], "{key:?} opens the file's picker");
        assert_eq!(walk(&mut p, &["Tab", key]), ["date", "date"]);
        assert!(holds(&p).contains(&Some(date)), "{key:?} opens the date's");
    }
}

const TOGGLES: &str = r#"component App
  state on = false
  state sw = false
  state role = false
  state log = ""
  action saw(v: bool)
    log = `${log}input:${v} `
  action flip(v: bool)
    on = v
    log = `${log}change:${v} `
  action flick(v: bool)
    sw = v
  action flock(v: bool)
    role = v
  action pressed
    log = `${log}press `
  view
    column width=400 height=600
      input type="checkbox" checked=on input=saw change=flip press=pressed testId="check"
      input type="checkbox" switch checked=sw change=flick testId="switch"
      input type="checkbox" role="switch" checked=role change=flock testId="role"
      input type="checkbox" testId="loose"
"#;

#[test]
fn space_toggles_a_focused_checkbox_or_switch_as_a_click_and_enter_does_not() {
    use exact_runner::Value;
    let mut p = boot(TOGGLES);
    let slot = |p: &Presenter<()>, name: &str| p.host.runner().slot(name).cloned();
    let on = |v: bool| Some(Value::Bool(v));
    assert_eq!(walk(&mut p, &["Tab", "Enter"]), ["check", "check"]);
    assert_eq!(slot(&p, "on"), on(false), "Enter toggles no checkbox");
    p.key_down(" ", 0.);
    assert_eq!(slot(&p, "on"), on(true));
    assert_eq!(
        slot(&p, "log").as_ref().and_then(Value::as_str),
        Some("press press input:true change:true "),
        "Enter presses; Space presses, then the toggle's input and change"
    );
    for (name, state) in [("switch", "sw"), ("role", "role")] {
        assert_eq!(walk(&mut p, &["Tab", "Enter"]), [name, name]);
        assert_eq!(slot(&p, state), on(false), "Enter toggles no {name}");
        p.key_down(" ", 0.);
        assert_eq!(slot(&p, state), on(true), "Space toggles the {name}");
    }
    // An unbound checkbox keeps its own state.
    let loose = id(&p, "loose");
    assert_eq!(walk(&mut p, &["Tab", " "]), ["loose", "loose"]);
    assert_eq!(p.controls.get(&loose), Some(&true));
}
