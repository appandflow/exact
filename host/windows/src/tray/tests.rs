use super::*;

/// `lParam` and `wParam` as version 4's callback packs them.
fn packed(event: u16, icon: u16, (x, y): (i16, i16)) -> (usize, isize) {
    let w = (x as u16 as usize) | ((y as u16 as usize) << 16);
    let l = (event as usize) | ((icon as usize) << 16);
    (w, l as isize)
}

#[test]
fn a_callback_names_its_event_its_icon_and_where_it_was_asked() {
    let cases = [
        (0x400, Event::Select { keyboard: false }),
        (0x401, Event::Select { keyboard: true }),
        (0x7B, Event::Menu),
        (0x200, Event::Other),
        (0x203, Event::Other),
    ];
    for (raw, event) in cases {
        let (w, l) = packed(raw, 0x1234, (1918, 1060));
        let got = callback(CALLBACK + 0x1234, w, l);
        assert_eq!(
            got,
            Some(Callback {
                event,
                icon: 0x1234,
                at: (1918, 1060)
            })
        );
    }
    // The message names the icon, whatever `lParam`'s high word holds: a
    // GUID's icon ignores its uID.
    let (w, l) = packed(0x7B, 0, (-1200, -40));
    let got = callback(CALLBACK + 7, w, l).unwrap();
    assert_eq!((got.icon, got.at), (7, (-1200, -40)));
    // Messages outside the icons' range are no callback.
    for message in [CALLBACK, CALLBACK + 0x4000, 0x7FFF, 0xC000] {
        assert_eq!(callback(message, w, l), None, "{message:#x}");
    }
}

#[test]
fn a_selection_presses_else_opens_the_menu_and_the_context_menu_never_presses() {
    let click = Event::Select { keyboard: false };
    let key = Event::Select { keyboard: true };
    assert_eq!(does(click, true, true, false), Does::Press);
    assert_eq!(
        does(click, false, true, false),
        Does::Menu { keyboard: false }
    );
    assert_eq!(does(key, false, true, false), Does::Menu { keyboard: true });
    assert_eq!(does(click, false, false, false), Does::Nothing);
    assert_eq!(
        does(Event::Menu, true, true, false),
        Does::Menu { keyboard: false }
    );
    assert_eq!(does(Event::Menu, true, false, false), Does::Nothing);
    for event in [click, key, Event::Menu, Event::Other] {
        assert_eq!(
            does(event, true, true, true),
            Does::Nothing,
            "{event:?}: disabled"
        );
    }
    assert_eq!(does(Event::Other, true, true, false), Does::Nothing);
}

#[test]
fn an_icons_id_follows_its_name_and_no_two_share_one() {
    let a = id("tray", &[]);
    assert_eq!(a, id("tray", &[]), "the same across launches");
    assert!((1..=0x3FFF).contains(&a));
    assert_ne!(id("other", &[]), a);
    // Two names that fold to one id: the second takes the next free one.
    let twin = (0..)
        .map(|i| format!("item-{i}"))
        .find(|n| id(n, &[]) == a)
        .unwrap();
    assert_eq!(id(&twin, &[a]), a % 0x3FFF + 1);
    assert_eq!(id("tray", &[0x3FFF]), a, "only a taken id moves");
    let last = (0..)
        .map(|i| format!("item-{i}"))
        .find(|n| id(n, &[]) == 0x3FFF)
        .unwrap();
    assert_eq!(id(&last, &[0x3FFF]), 1, "past the last id comes the first");
    // Every id is taken but one.
    let taken: Vec<u16> = (1..=0x3FFF).filter(|i| *i != 7).collect();
    assert_eq!(id("tray", &taken), 7);
}

#[test]
fn an_icons_guid_follows_its_app_and_name_in_every_build() {
    let g = guid("com.example.snap", "tray");
    // Pinned: a changed GUID loses the person's choice to show the icon.
    assert_eq!(g, guid("com.example.snap", "tray"));
    assert_eq!(format!("{g:032x}"), "9668442291f582f2a3881f7c3a7f7eeb");
    assert_eq!((g >> 76) & 0xF, 8, "version 8");
    assert_eq!((g >> 62) & 0b11, 0b10, "RFC 9562's variant");
    assert_ne!(guid("com.example.other", "tray"), g);
    assert_ne!(guid("com.example.snap", "plain"), g);
    // The separator keeps the app and the name apart.
    assert_ne!(guid("com.example.snapt", "ray"), g);
}

#[test]
fn only_a_production_build_names_its_icons_by_guid() {
    let compat = |trust: &str, app: &str| {
        format!(r#"{{"inputs":{{"app":"{app}"}},"embedded":{{"trust":"{trust}"}}}}"#)
    };
    assert_eq!(
        guid_owner(&compat("production", "com.example.snap")).as_deref(),
        Some("com.example.snap")
    );
    assert_eq!(guid_owner(&compat("development", "com.example.snap")), None);
    assert_eq!(guid_owner(&compat("production", "")), None);
    assert_eq!(guid_owner(r#"{"inputs":{"app":"com.example.snap"}}"#), None);
}

#[test]
fn a_tooltip_keeps_127_units_and_never_half_a_pair() {
    let units = |t: &[u16; 128]| t.iter().position(|c| *c == 0).unwrap();
    let t = tip("Ember");
    assert_eq!(String::from_utf16(&t[..units(&t)]).unwrap(), "Ember");
    assert_eq!(units(&tip(&"a".repeat(300))), 127);
    let long = format!("{}🪟", "a".repeat(126));
    let t = tip(&long);
    assert_eq!(units(&t), 126, "the pair would end past 127");
    assert_eq!(String::from_utf16(&t[..126]).unwrap(), "a".repeat(126));
}

#[test]
fn an_icon_holds_straight_alpha_in_bgra_and_a_disabled_one_is_dimmed() {
    // Opaque red, half-covered blue, clear.
    let rgba = [255, 0, 0, 255, 0, 0, 128, 128, 0, 0, 0, 0];
    assert_eq!(
        icon_bgra(&rgba, false),
        [0, 0, 255, 255, 255, 0, 0, 128, 0, 0, 0, 0]
    );
    assert_eq!(
        icon_bgra(&rgba, true),
        [0, 0, 255, 92, 255, 0, 0, 46, 0, 0, 0, 0]
    );
}

#[test]
fn the_taskbar_is_dark_unless_the_system_mode_is_light() {
    assert!(taskbar_dark(Some(0)));
    assert!(!taskbar_dark(Some(1)));
    assert!(taskbar_dark(None));
}

#[test]
fn the_shell_is_told_only_what_changed() {
    let placed = [(1, "a"), (2, "b"), (3, "c")];
    let wanted = [(2, "b"), (3, "C"), (4, "d")];
    assert_eq!(
        ops(&placed, &wanted),
        [Op::Delete(1), Op::Modify(3), Op::Add(4)]
    );
    assert_eq!(ops(&wanted, &wanted), []);
    // Explorer restarted: nothing is placed, so everything is added.
    assert_eq!(ops(&[], &wanted), [Op::Add(2), Op::Add(3), Op::Add(4)]);
    // Of two buttons whose ids fold alike, the first goes: the second takes
    // its id but keeps its own GUID, so it is removed and added again.
    let first = Icon {
        id: 5,
        guid: Some(1),
    };
    let second = Icon {
        id: 6,
        guid: Some(2),
    };
    let moved = Icon {
        id: 5,
        guid: Some(2),
    };
    assert_eq!(
        ops(&[(first, "a"), (second, "b")], &[(moved, "b")]),
        [Op::Delete(first), Op::Delete(second), Op::Add(moved)]
    );
}

#[test]
fn the_menus_keys_are_named_as_the_page_names_them() {
    assert_eq!(key(0x0D).as_deref(), Some("Enter"));
    assert_eq!(key(0x1B).as_deref(), Some("Escape"));
    assert_eq!(key(0x20).as_deref(), Some(" "));
    assert_eq!(key(0x28).as_deref(), Some("ArrowDown"));
    assert_eq!(key(0x24).as_deref(), Some("Home"));
    assert_eq!(key(0x51).as_deref(), Some("q"), "a letter for type-ahead");
    assert_eq!(key(0x37).as_deref(), Some("7"));
    assert_eq!(key(0x10), None, "Shift alone");
}

#[test]
fn a_wheel_notch_scrolls_the_persons_lines() {
    assert_eq!(wheel(-120, 3), 48.);
    assert_eq!(wheel(240, 1), -32.);
}
