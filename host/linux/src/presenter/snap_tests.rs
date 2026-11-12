//! Scroll snap on the Linux family (LLP 1008 §9): the offsets a scroller's
//! areas rest at, as the reader is told them, and the rule that picks one.
use super::*;

fn boot(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
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

fn rests(snap: &Snap) -> Vec<(f32, f32, bool)> {
    let mut v: Vec<_> = snap.areas.iter().map(|a| (a.from, a.to, a.stop)).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

const SNAPPERS: &str = r#"component App
  view
    column width=400 height=600
      scroll testId="rail" width=400 height=100 overflow-x="scroll" overflow-y="hidden" scroll-snap-type="x mandatory"
        row
          for i in [0, 1, 2, 3] key=i
            view width=300 height=100 flex-shrink=0 scroll-snap-align="center" scroll-snap-stop=(i == 2 ? "always" : "normal")
      scroll testId="pager" width=400 height=200 scroll-snap-type="y mandatory" scroll-padding-top=20
        column
          for i in [0, 1, 2] key=i
            view width=400 height=180 flex-shrink=0 scroll-snap-align="start none" scroll-margin-top=(i == 1 ? 10 : 0)
          view width=400 height=500 flex-shrink=0 scroll-snap-align="start"
      scroll testId="near" width=400 height=100 overflow-x="scroll" overflow-y="hidden" scroll-snap-type="x"
        row
          for i in [0, 1, 2] key=i
            view width=200 height=100 flex-shrink=0 scroll-snap-align="end"
              scroll width=200 height=100 overflow-x="scroll"
                row
                  view width=300 height=100 flex-shrink=0 scroll-snap-align="start"
"#;

#[test]
fn a_scroller_tells_the_offsets_its_areas_rest_at_on_each_axis_it_snaps() {
    let mut p = boot(SNAPPERS);
    let (rail, pager, near) = (id(&p, "rail"), id(&p, "pager"), id(&p, "near"));
    // Each 300-wide card's centre at the 400-wide port's; the first and the
    // last are held by the ends (travel 800).
    let x = p.snap(rail, false).unwrap();
    assert!(x.mandatory);
    assert_eq!(x.port, 400.);
    assert_eq!(
        rests(&x),
        vec![
            (0., 0., false),
            (250., 250., false),
            (550., 550., true),
            (800., 800., false)
        ]
    );
    assert_eq!(p.snap(rail, true), None, "it snaps along x alone");
    // A page's start at the snapport's, 20 below the port's top
    // (`scroll-padding-top`); the second page's area grows 10 above it, past
    // the snapport, so it rests anywhere from 150 to 160; the
    // last, taller than the 180 snapport, rests anywhere it covers it, to
    // the travel's end (840).
    let y = p.snap(pager, true).unwrap();
    assert_eq!(y.port, 180.);
    assert_eq!(
        rests(&y),
        vec![
            (0., 0., false),
            (150., 160., false),
            (340., 340., false),
            (520., 840., false)
        ]
    );
    // `proximity`, and a nested scroller keeps its own areas.
    let near = p.snap(near, false).unwrap();
    assert!(!near.mandatory);
    assert_eq!(
        rests(&near),
        vec![(0., 0., false), (0., 0., false), (200., 200., false)]
    );
}

#[test]
fn the_reader_is_told_each_scrollers_snap_in_its_pixels() {
    let mut p = boot(SNAPPERS);
    let (rail, pager) = (id(&p, "rail"), id(&p, "pager"));
    let words = p.reader_snaps(&[rail, pager, PAGE], 2.);
    let f = |w: i32| f32::from_bits(w as u32);
    // The rail: x, mandatory; a 400 port (800 px) and four areas; no y.
    assert_eq!(words[0], 1 | 4);
    assert_eq!((f(words[1]), words[2]), (800., 4));
    let rail_end = 3 + 3 * 4;
    assert_eq!((f(words[rail_end]), words[rail_end + 1]), (0., 0));
    let pager_at = rail_end + 2;
    assert_eq!(words[pager_at], 2 | 4);
    assert_eq!((f(words[pager_at + 1]), words[pager_at + 2]), (0., 0));
    assert_eq!((f(words[pager_at + 3]), words[pager_at + 4]), (360., 4));
    let page_at = pager_at + 5 + 3 * 4;
    assert_eq!(&words[page_at..], &[0, 0, 0, 0, 0]);
}

#[test]
fn a_rest_is_the_nearest_position_short_of_an_always_one() {
    let at = |from: f32, stop: bool| SnapArea {
        from,
        to: from,
        stop,
        id: 0,
    };
    let cards = [
        at(0., false),
        at(250., false),
        at(550., false),
        at(800., false),
    ];
    assert_eq!(rest(&cards, 600., 250., true, 400.), Some(550.));
    assert_eq!(
        rest(&cards, 400., 250., true, 400.),
        Some(250.),
        "the smaller of two as near"
    );
    let always = cards.map(|a| SnapArea { stop: true, ..a });
    assert_eq!(rest(&always, 780., 250., true, 400.), Some(550.));
    assert_eq!(rest(&always, 10., 550., true, 400.), Some(250.));
    let long = [
        at(0., false),
        SnapArea {
            from: 300.,
            to: 900.,
            stop: true,
            id: 0,
        },
        at(1200., false),
    ];
    assert_eq!(rest(&long, 640., 400., true, 400.), Some(640.));
    assert_eq!(
        rest(&long, 1150., 400., true, 400.),
        Some(900.),
        "left at its end"
    );
    assert_eq!(rest(&cards, 380., 250., false, 400.), Some(250.));
    assert_eq!(
        rest(&cards, 400., 250., false, 400.),
        None,
        "`proximity`: too far"
    );
    assert_eq!(rest(&[], 400., 250., true, 400.), None);
}

#[test]
fn a_wheel_rests_where_its_ticks_add_up_to() {
    let mut p = boot(SNAPPERS);
    let rail = id(&p, "rail");
    let mut seen = Vec::new();
    for _ in 0..4 {
        p.wheel(rail, 40., 0.).unwrap();
        seen.push(p.scroll_of(rail).0);
    }
    // Ticks under half the way to the next position leave it; their sum
    // past the half takes it there.
    assert_eq!(seen, [0., 0., 0., 250.]);
    // A scroll from elsewhere starts the sum over from where it put it.
    p.scroll_to(rail, (500., 0.));
    p.wheel(rail, -40., 0.).unwrap();
    assert_eq!(p.scroll_of(rail).0, 550.);
}

#[test]
fn a_layout_change_keeps_a_scroller_on_the_area_it_rested_on() {
    let mut p = boot(
        r#"component App
  state w = 300
  action wider
    w = 350
  view
    column width=400 height=600
      button testId="wider" press=wider
        text "wider"
      scroll testId="rail" width=400 height=100 overflow-x="scroll" overflow-y="hidden" scroll-snap-type="x mandatory"
        row
          for i in [0, 1, 2, 3] key=i
            view width=w height=100 flex-shrink=0 scroll-snap-align="start"
"#,
    );
    let rail = id(&p, "rail");
    for _ in 0..4 {
        p.wheel(rail, 40., 0.).unwrap();
    }
    assert_eq!(p.scroll_of(rail).0, 300.);
    // The cards widen: the second now rests at 350.
    p.tap(id(&p, "wider")).unwrap();
    for _ in 0..8 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
    assert_eq!(p.scroll_of(rail).0, 350.);
}

#[test]
fn a_virtualized_list_adds_the_rows_it_has_not_mounted() {
    let mut p = boot(
        r#"component App
  view
    list testId="strip" virtualized=true display="flex" width=300 height=100 overflow-x="scroll" overflow-y="hidden" estimated-item-width=150 scroll-snap-type="x mandatory"
      for n in range(1000) key=n
        view width=150 height=100 flex-shrink=0 scroll-snap-align="start"
"#,
    );
    let strip = id(&p, "strip");
    let snap = p.snap(strip, false).unwrap();
    let mut at: Vec<f32> = snap.areas.iter().map(|a| a.from).collect();
    at.sort_by(f32::total_cmp);
    at.dedup();
    // Every card from the first to the bounded window's end (eight ports and
    // one past the offset), mounted or not.
    assert!(at.len() > 16, "{} areas", at.len());
    assert!(
        at.iter().enumerate().all(|(i, &x)| x == i as f32 * 150.),
        "{at:?} {:?}",
        snap.areas
    );
}
