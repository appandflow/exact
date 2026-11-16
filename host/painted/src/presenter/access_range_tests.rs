//! A drawn slider or spin button stepped by its author's keys.
use super::*;

/// A drawn slider (horizontal and vertical) and spin button, each stepped
/// by its own `key` handler, which also hears its `keyup`.
const DRAWN_RANGES: &str = r#"component App
  state level = 50
  state floor = 3
  state tilt = 0
  state heard = ""
  action stepLevel(k: string)
    heard = `${heard} ${k}`
    if k == "ArrowRight"
      level = level + 10
    if k == "ArrowLeft"
      level = level - 10
  action stepFloor(k: string)
    heard = `${heard} ${k}`
    if k == "ArrowUp"
      floor = floor + 1
    if k == "ArrowDown"
      floor = floor - 1
  action stepTilt(k: string)
    heard = `${heard} ${k}`
    if k == "ArrowUp"
      tilt = tilt + 1
  action up(k: string)
    heard = `${heard} up:${k}`
  view
    column width=320 height=400 align-items="flex-start"
      box role="slider" tabindex=0 aria-label="Level" aria-valuenow=level key=stepLevel keyup=up width=200 height=30
      box role="spinbutton" tabindex=0 aria-label="Floor" aria-valuenow=floor aria-valuemin=1 key=stepFloor width=100 height=30
      box role="slider" aria-orientation="vertical" tabindex=0 aria-label="Tilt" aria-valuenow=tilt aria-valuemin=-5 aria-valuemax=5 key=stepTilt width=30 height=100
      text heard testId="heard" width=300 height=20
"#;

/// A service steps a drawn slider or spin button as WebKit steps an ARIA
/// one: the author's key handler hears ArrowRight or ArrowLeft for a
/// horizontal slider, ArrowUp or ArrowDown for a vertical one and a spin
/// button, then its keyup; and the step is offered only short of a bound.
/// None is set to a value: only its author's keys move it.
#[test]
fn a_drawn_range_steps_by_its_authors_keys() {
    let mut p = boot(DRAWN_RANGES, (320., 400.), 0);
    let nodes = statement(&mut p).nodes;
    let flags = |nodes: &[Node], name: &str| named(nodes, name).flags;
    let both = flag::RANGE | flag::FORWARD | flag::BACKWARD;
    assert_eq!(flags(&nodes, "Level"), both);
    assert_eq!(
        flags(&nodes, "Floor"),
        both,
        "a spin button with no maximum goes up"
    );
    assert_eq!(flags(&nodes, "Tilt"), both);
    let roles = ["Level", "Floor", "Tilt"].map(|name| named(&nodes, name).role);
    assert_eq!(roles, [role::SLIDER, role::SPIN_BUTTON, role::SLIDER]);
    let of = |name: &str| named(&nodes, name).id;
    let heard = |p: &Presenter<Data>| {
        let kernel = p.host().kernel();
        let text = kernel.node(id(p, "heard")).unwrap().props.str(PropId::Text);
        text.unwrap_or_default().trim().to_string()
    };
    assert!(p.access_act(of("Level"), act::FORWARD, 0.0, 0.0));
    settle(&mut p);
    assert_eq!(heard(&p), "ArrowRight up:ArrowRight");
    assert_eq!(named(&statement(&mut p).nodes, "Level").range[2], 60.0);
    assert!(p.access_act(of("Level"), act::BACKWARD, 0.0, 0.0));
    assert!(p.access_act(of("Floor"), act::BACKWARD, 0.0, 0.0));
    assert!(p.access_act(of("Tilt"), act::FORWARD, 0.0, 0.0));
    settle(&mut p);
    assert_eq!(
        heard(&p),
        "ArrowRight up:ArrowRight ArrowLeft up:ArrowLeft ArrowDown ArrowUp"
    );
    let nodes = statement(&mut p).nodes;
    assert_eq!(named(&nodes, "Level").range[2], 50.0);
    assert_eq!(named(&nodes, "Floor").range[2], 2.0);
    assert_eq!(named(&nodes, "Tilt").range[2], 1.0);
    assert!(!p.access_act(named(&nodes, "Level").id, act::SET, 20.0, 0.0));
    assert!(p.access_act(named(&nodes, "Floor").id, act::BACKWARD, 0.0, 0.0));
    settle(&mut p);
    let nodes = statement(&mut p).nodes;
    assert_eq!(
        flags(&nodes, "Floor"),
        flag::RANGE | flag::FORWARD,
        "at its least"
    );
    assert!(!p.access_act(named(&nodes, "Floor").id, act::BACKWARD, 0.0, 0.0));
}

/// Two horizontal sliders, one of them `direction: ltr` and the other
/// `rtl`, under a document of `direction`, each saying the keys it hears.
fn sliders_in(direction: Direction) -> Presenter<Data> {
    const SLIDERS: &str = r#"component App
  state heard = ""
  action hear(k: string)
    heard = `${heard} ${k}`
  view
    column width=320 height=400 align-items="flex-start"
      box role="slider" tabindex=0 aria-label="Plain" aria-valuenow=50 key=hear width=200 height=30
      box role="slider" tabindex=0 aria-label="Ltr" aria-valuenow=50 key=hear direction="ltr" width=200 height=30
      box role="slider" tabindex=0 aria-label="Rtl" aria-valuenow=50 key=hear direction="rtl" width=200 height=30
      text heard testId="heard" width=300 height=20
"#;
    let launch = exact_runner::time::Launch {
        direction: Some(direction),
        ..exact_runner::time::Launch::NONE
    };
    exact_runner::time::launching_with(launch, || boot(SLIDERS, (320., 400.), 0))
}

/// A service's increment on a horizontal slider is ArrowRight where the
/// slider's own computed `direction` is `ltr` and ArrowLeft where it is
/// `rtl`, whatever the document's: the slider inherits the document's
/// unless its author says otherwise.
#[test]
fn a_drawn_sliders_increment_follows_its_own_direction() {
    for (document, plain) in [
        (Direction::Rtl, "ArrowLeft"),
        (Direction::Ltr, "ArrowRight"),
    ] {
        let mut p = sliders_in(document);
        assert_eq!(p.host().runner().direction(), document.name());
        let nodes = statement(&mut p).nodes;
        for name in ["Plain", "Ltr", "Rtl"] {
            assert!(p.access_act(named(&nodes, name).id, act::FORWARD, 0.0, 0.0));
        }
        settle(&mut p);
        let kernel = p.host().kernel();
        let heard = kernel
            .node(id(&p, "heard"))
            .unwrap()
            .props
            .str(PropId::Text);
        let heard = heard.unwrap_or_default().trim().to_string();
        assert_eq!(
            heard,
            format!("{plain} ArrowRight ArrowLeft"),
            "{document:?}"
        );
    }
}
