//! Fixed photo-transform authoring: IDREF, four geometry and six release numbers.
use exact_kernel::{Kernel, PropId};
use exact_plan::{EventKind, Plan, Value};
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected query: {name}")
    }
}

/// The handle with `event` bound to `handler`, and its partner (a transform
/// drag needs both, LLP 1057.003 C6) bound to a well-formed `partner`.
fn source(event: &str, params: &str, handler: &str, idref: &str) -> String {
    let (other, other_params) = partner(event);
    format!("component App\n  state value = 0\n  action receive({params})\n    value = 1\n  action partner({other_params})\n    value = 2\n  view\n    column overflow=\"hidden\"\n      column id=\"photo\" width=\"100%\" height=\"100%\" box-sizing=\"border-box\"\n        column testId=\"handle\" transformDragFor={idref} {event}={handler} {other}=partner\n")
}
fn partner(event: &str) -> (&'static str, &'static str) {
    if event == "transformgeometry" {
        ("transformrelease", RELEASE)
    } else {
        ("transformgeometry", GEOMETRY)
    }
}

const GEOMETRY: &str = "bw: number, bh: number, pw: number, ph: number";
const RELEASE: &str = "x: number, y: number, scale: number, vx: number, vy: number, vs: number";

#[test]
fn transform_handlers_compile_bake_roundtrip_and_export_old_and_new_ordinals() {
    for (event, params, ordinal) in [
        ("transformgeometry", GEOMETRY, 15),
        ("transformrelease", RELEASE, 16),
    ] {
        let plan = contract::compile(&source(event, params, "receive", "\"photo\"")).unwrap();
        let plan = contract::bake(plan, NoData).unwrap();
        let plan = Plan::decode(&plan.encode()).unwrap();
        let runner = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let key = runner.kernel().find_by_test_id("handle")[0];
        let handle = runner.kernel().node_by_key(key).unwrap();
        let prop = PropId::from_name("transformDragFor").unwrap();
        assert_eq!(prop as u16, 76);
        assert_eq!(handle.props.str(prop), Some("photo"));
        let event_kind = EventKind::from_name(event).unwrap();
        assert_eq!(event_kind as u8, ordinal);
        assert!(runner.handlers_of(handle.id).contains(&event_kind));
        let tree: serde_json::Value = serde_json::from_str(&agent::tree(&runner)).unwrap();
        assert!(tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["props"]["transformDragFor"] == "photo"
                && n["handlers"]
                    .as_array()
                    .is_some_and(|h| h.contains(&serde_json::json!(event)))));
    }
    assert_eq!(EventKind::Heightrelease as u8, 14);
    assert_eq!(EventKind::Navigate as u8, 13);
    assert_eq!(PropId::HeightDragFor as u16, 75);
}

#[test]
fn exact_numeric_arity_and_string_idref_are_required() {
    for (event, params) in [
        ("transformgeometry", GEOMETRY),
        ("transformrelease", RELEASE),
    ] {
        for bad in [
            params.rsplit_once(',').unwrap().0.to_string(),
            format!("{params}, extra: number"),
        ] {
            let e = contract::compile(&source(event, &bad, "receive", "\"photo\""))
                .unwrap_err()
                .to_string();
            assert!(e.contains("handler-arity"), "{e}");
        }
        for ty in ["string", "bool"] {
            let bad = params.replacen("number", ty, 1);
            let e = contract::compile(&source(event, &bad, "receive", "\"photo\""))
                .unwrap_err()
                .to_string();
            assert!(e.contains("type-handler-payload"), "{e}");
        }
        for bad in ["1", "false"] {
            let e = contract::compile(&source(event, params, "receive", bad))
                .unwrap_err()
                .to_string();
            assert!(e.contains("attr-type"), "{e}");
        }
        contract::compile(&source(
            event,
            &format!("origin: string, {params}"),
            "receive(\"photo\")",
            "\"photo\"",
        ))
        .unwrap();
    }
}

#[test]
fn one_transform_handler_without_the_other_is_refused() {
    for (event, params, missing) in [
        ("transformgeometry", GEOMETRY, "transformrelease"),
        ("transformrelease", RELEASE, "transformgeometry"),
    ] {
        let src = format!("component App\n  state value = 0\n  action receive({params})\n    value = 1\n  view\n    column\n      column id=\"photo\"\n        column transformDragFor=\"photo\" {event}=receive\n");
        let e = contract::compile(&src).unwrap_err().to_string();
        assert!(
            e.contains("lower-transform-drag-handlers") && e.contains(&format!("add `{missing}=`")),
            "{e}"
        );
    }
}

#[test]
fn numeric_types_are_rechecked_after_child_action_inlining() {
    for (event, params) in [
        ("transformgeometry", GEOMETRY),
        ("transformrelease", RELEASE),
    ] {
        for invalid in [false, true] {
            let params = if invalid {
                params.replacen("number", "string", 1)
            } else {
                params.into()
            };
            let (other, other_params) = partner(event);
            let src = format!("component App\n  state n = 0\n  action receive({params})\n    n = 1\n  action partner({other_params})\n    n = 2\n  view\n    column\n      Handle(callback=receive, other=partner)\ncomponent Handle\n  props\n    callback: action\n    other: action\n  view\n    column transformDragFor=\"photo\" {event}=callback {other}=other\n");
            let result = contract::compile(&src);
            if invalid {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("type-handler-payload"));
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn pixel_translate_literals_and_dynamic_template_reach_kernel_rows() {
    let src = "component App\n  state x = 0\n  state y = 0\n  action move(a: number, b: number)\n    x = a\n    y = b\n  view\n    column\n      column testId=\"literal\" translate=\"-12.5px 3px\"\n      column testId=\"dynamic\" translate=`${x}px ${y}px`\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let literal = r.kernel().find_by_test_id("literal")[0];
    assert_eq!(
        r.kernel().node_by_key(literal).unwrap().style.translate,
        exact_kernel::Vec2 { x: -12.5, y: 3.0 }
    );
    let dynamic = r.kernel().find_by_test_id("dynamic")[0];
    r.act("move", vec![Value::Number(27.25), Value::Number(-40.0)])
        .unwrap();
    assert_eq!(
        r.kernel().node_by_key(dynamic).unwrap().style.translate,
        exact_kernel::Vec2 { x: 27.25, y: -40.0 }
    );
    // A value the row refuses is unset, as CSS does an invalid value at
    // computed-value time, and journaled; the runner goes on.
    r.act("move", vec![Value::Number(1e40), Value::Number(0.0)])
        .unwrap();
    assert!(!r.is_poisoned());
    assert_eq!(
        r.kernel().node_by_key(dynamic).unwrap().style.translate,
        exact_kernel::Vec2 { x: 0.0, y: 0.0 }
    );
    assert!(r.journal().any(|l| l.contains("invalid translate value")));
    r.act("move", vec![Value::Number(1.0), Value::Number(2.0)])
        .unwrap();
    assert_eq!(
        r.kernel().node_by_key(dynamic).unwrap().style.translate,
        exact_kernel::Vec2 { x: 1.0, y: 2.0 }
    );
    for bad in [
        "1 2",
        "20%% 0",
        "calc(1px + 2px) 0",
        "1px 2px 3px 4px",
        "NaNpx 0",
        "1e39px 0",
    ] {
        let src = format!("component App\n  view\n    column translate=\"{bad}\"\n");
        assert!(contract::compile(&src).is_err(), "{bad}");
    }
}

const OWNERS: &str = "lower-transform-drag-owners";

fn ownership_source(view: &str) -> String {
    let view: String = view.lines().map(|line| format!("      {line}\n")).collect();
    format!(
        "component App\n  state n = 0\n  state on = true\n  state choice = some(1)\n  state rows = [1, 2]\n  action geometry({GEOMETRY})\n    n = 1\n  action release({RELEASE})\n    n = 2\n  view\n    column\n{view}"
    )
}

fn photo(name: &str) -> String {
    format!(
        "column overflow=\"hidden\"\n  column id={name} width=\"100%\" height=\"100%\" box-sizing=\"border-box\"\n    column transformDragFor={name} transformgeometry=geometry transformrelease=release"
    )
}

fn indented(body: &str) -> String {
    body.lines().map(|line| format!("  {line}\n")).collect()
}

fn nested_transform_targets(body: &str) -> String {
    ownership_source(&format!(
        "column overflow=\"hidden\"\n  column id=\"outer\" width=\"100%\" height=\"100%\" box-sizing=\"border-box\" overflow=\"hidden\"\n    column id=\"inner\" width=\"100%\" height=\"100%\" box-sizing=\"border-box\"\n{}",
        indented(&indented(&indented(body)))
    ))
}

fn transform_choice(condition: &str) -> String {
    format!("if {condition}\n  column transformDragFor=\"outer\" transformgeometry=geometry transformrelease=release\nelse\n  column transformDragFor=\"inner\" transformgeometry=geometry transformrelease=release")
}

fn owner_warnings(src: &str) -> Vec<u32> {
    super::warnings::warned(src)
        .into_iter()
        .map(|(id, line)| {
            assert_eq!(id, OWNERS, "{src}");
            line
        })
        .collect()
}

#[test]
fn simultaneous_transform_targets_name_both_binding_sites() {
    // The id is scoped to strict ancestors: equal spellings can name different targets.
    for other in ["\"other\"", "\"photo\""] {
        let src = ownership_source(&format!("{}\n{}", photo("\"photo\""), photo(other)));
        assert_eq!(owner_warnings(&src), [14, 17]);
        let (_, warnings) = contract::compile_warned(&src);
        for w in warnings {
            assert!(w.message.contains("first valid target"), "{w}");
            assert!(w.message.contains("never stolen"), "{w}");
            assert!(
                w.message
                    .contains("Mount handles only for the active target"),
                "{w}"
            );
        }
    }
}

#[test]
fn constant_templates_and_strings_resolve_to_the_same_transform_target() {
    for (id, binding) in [
        ("\"first\"", "`first`"),
        ("`first`", "\"first\""),
        ("\"first\"", "`fir${\"st\"}`"),
    ] {
        let first = photo(id).replace(
            &format!("transformDragFor={id}"),
            &format!("transformDragFor={binding}"),
        );
        let src = ownership_source(&format!("{first}\n{}", photo("\"second\"")));
        assert_eq!(owner_warnings(&src), [14, 17], "{src}");
        assert!(owner_warnings(&ownership_source(&first)).is_empty());
    }
}

#[test]
fn a_transform_target_repeated_by_a_row_template_is_warned_once() {
    for name in ["\"photo\"", "`photo-${row}`"] {
        for list in ["rows", "[1, 2]"] {
            let src = ownership_source(&format!(
                "list id=\"photos\"\n  for row in {list} key=row\n{}",
                indented(&indented(&photo(name)))
            ));
            assert_eq!(owner_warnings(&src), [16]);
            let (_, warnings) = contract::compile_warned(&src);
            assert!(warnings[0].message.contains("`for` template"));
        }
    }
}

#[test]
fn one_transform_target_can_have_several_handles_even_in_a_loop() {
    for handles in [
        "column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release",
        "column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release\ncolumn transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release",
        "for row in rows key=row\n  column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release",
    ] {
        let src = ownership_source(&format!(
            "column overflow=\"hidden\"\n  column id=\"photo\" width=\"100%\" height=\"100%\" box-sizing=\"border-box\"\n{}",
            indented(&indented(handles))
        ));
        assert!(owner_warnings(&src).is_empty(), "{src}");
    }
    let src = ownership_source(&format!(
        "for row in [1] key=row\n{}",
        indented(&photo("\"photo\""))
    ));
    assert!(owner_warnings(&src).is_empty(), "{src}");
}

#[test]
fn mutually_exclusive_transform_targets_do_not_compete() {
    let first = indented(&photo("\"first\""));
    let second = indented(&photo("\"second\""));
    for view in [
        format!("if on\n{first}else\n{second}"),
        format!(
            "match choice\n  case some(value)\n{}  case none\n{}",
            indented(&first),
            indented(&second)
        ),
    ] {
        assert!(
            owner_warnings(&ownership_source(&view)).is_empty(),
            "{view}"
        );
        // A sibling outside the choice can compete with either arm.
        let src = ownership_source(&format!("{view}{}", photo("\"third\"")));
        assert_eq!(owner_warnings(&src).len(), 3);
    }
}

#[test]
fn hidden_or_unresolved_transform_bindings_are_not_competitors() {
    for other in [
        photo("\"other\"").replacen("column ", "column display=\"none\" ", 1),
        photo("\"other\"").replacen("column ", "column inert=true ", 1),
        photo("\"other\"").replace("transformDragFor=\"other\"", "transformDragFor=\"missing\""),
        photo("\"other\"").replacen("column ", "column id=\"other\" ", 1),
        photo("\"other\"").replace("transformDragFor=\"other\"", "transformDragFor=\"\""),
    ] {
        let src = ownership_source(&format!("{}\n{other}", photo("\"photo\"")));
        assert!(owner_warnings(&src).is_empty(), "{src}");
    }
}

#[test]
fn static_class_visibility_respects_inline_overrides_for_transform_targets() {
    for (attrs, count) in [
        ("class=Hidden", 0),
        ("class=Hidden display=\"block\"", 2),
        ("class=Visible display=\"none\"", 0),
        ("class=Hidden display=(on ? \"block\" : \"none\")", 2),
    ] {
        for suffix in ["overflow", "id"] {
            let other = photo("\"other\"").replacen(
                &format!("column {suffix}"),
                &format!("column {attrs} {suffix}"),
                1,
            );
            let src = format!(
                "{}style Hidden\n  display=\"none\"\nstyle Visible\n  display=\"block\"\n",
                ownership_source(&format!("{}\n{other}", photo("\"photo\"")))
            );
            assert_eq!(owner_warnings(&src).len(), count, "{src}");
        }
    }
}

#[test]
fn shared_conditions_keep_transform_handles_exclusive_across_rows() {
    let handle = |name| {
        format!("column transformDragFor=\"{name}\" transformgeometry=geometry transformrelease=release")
    };
    for choice in [
        transform_choice("on"),
        transform_choice("selected"),
        transform_choice("not on or n > 0"),
        format!(
            "match choice\n  case some(value)\n{}  case none\n{}",
            indented(&indented(&handle("outer"))),
            indented(&indented(&handle("inner")))
        ),
    ] {
        let src =
            nested_transform_targets(&format!("for row in rows key=row\n{}", indented(&choice)))
                .replace("  view\n", "  derive selected = on\n  view\n");
        assert!(owner_warnings(&src).is_empty(), "{src}");
    }
    let src = nested_transform_targets(&format!(
        "for row in rows key=row\n  for item, position in [row] key=item\n{}",
        indented(&indented(&transform_choice("position == 0")))
    ));
    assert!(owner_warnings(&src).is_empty(), "{src}");
}

#[test]
fn opposite_arms_in_different_rows_can_bind_different_transform_targets() {
    for condition in ["row == 1", "position == 0"] {
        let src = nested_transform_targets(&format!(
            "for row, position in rows key=row\n{}",
            indented(&transform_choice(condition))
        ));
        assert_eq!(owner_warnings(&src), [17, 19]);
    }
}

#[test]
fn child_transform_choices_can_vary_with_row_state_and_props() {
    for (declaration, count) in [
        ("state active = true", 2),
        ("derive active = row == 1", 2),
        ("derive active = shared", 0),
    ] {
        let src = format!(
            "{}component Handle\n  props\n    row: number\n    shared: bool\n    geometry: action\n    release: action\n  {declaration}\n  view\n{}",
            nested_transform_targets("for row in rows key=row\n  Handle(row=row, shared=on, geometry=geometry, release=release)"),
            indented(&indented(&transform_choice("active")))
        );
        assert_eq!(owner_warnings(&src).len(), count, "{src}");
    }
}

#[test]
fn a_guard_can_limit_a_transform_target_to_one_literal_row() {
    for (list, condition) in [
        ("[1, 2]", "position == 0"),
        ("[1, 2]", "row == 1"),
        ("rows", "position == 0"),
    ] {
        for guard_handle in [false, true] {
            let photo = photo("\"photo\"");
            let body = if guard_handle {
                photo.replace(
                    "    column transformDragFor",
                    &format!("    if {condition}\n      column transformDragFor"),
                )
            } else {
                format!("if {condition}\n{}", indented(&photo))
            };
            let src = ownership_source(&format!(
                "for row, position in {list} key=row\n{}",
                indented(&body)
            ));
            assert!(owner_warnings(&src).is_empty(), "{src}");
        }
    }
    let src = ownership_source(&format!(
        "for row, position in [1, 2, 3] key=row\n  if position < 2\n{}",
        indented(&indented(&photo("\"photo\"")))
    ));
    assert_eq!(owner_warnings(&src).len(), 1, "{src}");
}

#[test]
fn separate_handles_in_different_rows_can_own_distinct_transform_targets() {
    let src = ownership_source(&format!(
        "for row, position in [1, 2] key=row\n{}",
        indented(&photo("\"photo\"").replace(
            "    column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release",
            "    if position == 0\n      column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release\n    else\n      column transformDragFor=\"photo\" transformgeometry=geometry transformrelease=release"
        ))
    ));
    assert_eq!(owner_warnings(&src).len(), 2, "{src}");
}

#[test]
fn shadowed_row_names_do_not_resolve_to_an_outer_transform_target() {
    let src = ownership_source(
        "for row in [\"a\", \"b\"] key=row\n  column overflow=\"hidden\"\n    column id=row width=\"100%\" height=\"100%\" box-sizing=\"border-box\"\n      for row in [\"c\"] key=row\n        column transformDragFor=row transformgeometry=geometry transformrelease=release",
    );
    assert!(owner_warnings(&src).is_empty(), "{src}");
}

#[test]
fn literal_number_and_boolean_template_parts_resolve_transform_targets() {
    for (id, binding) in [
        ("`photo-${1}`", "`photo-${1}`"),
        ("`photo-${1}`", "\"photo-1\""),
        ("\"photo-true\"", "`photo-${true}`"),
        ("`photo-${false}`", "\"photo-false\""),
        ("`photo-${-0}`", "\"photo-0\""),
        ("\"photo-1e+21\"", "`photo-${1e21}`"),
    ] {
        let first = photo(id).replace(
            &format!("transformDragFor={id}"),
            &format!("transformDragFor={binding}"),
        );
        let src = ownership_source(&format!("{first}\n{}", photo("\"second\"")));
        assert_eq!(owner_warnings(&src), [14, 17], "{src}");
    }
}

#[test]
fn statically_unreachable_match_arms_cannot_own_a_transform_drag() {
    for (subject, reachable, unreachable) in [
        ("some(1)", "\"outer\"", "\"inner\""),
        ("none", "\"inner\"", "\"outer\""),
    ] {
        let body = format!(
            "match {subject}\n  case some(value)\n{}  case none\n{}",
            indented(&indented(&photo("\"outer\""))),
            indented(&indented(&photo("\"inner\"")))
        );
        let src = ownership_source(&format!("{body}{}", photo("\"side\"")));
        let (_, warnings) = contract::compile_warned(&src);
        let lines = owner_warnings(&src);
        assert_eq!(lines.len(), 2, "{src}: {warnings:?}");
        let handle = |id| format!("transformDragFor={id}");
        assert!(lines.iter().any(|line| src
            .lines()
            .nth(*line as usize - 1)
            .unwrap()
            .contains(&handle(reachable))));
        assert!(lines.iter().all(|line| !src
            .lines()
            .nth(*line as usize - 1)
            .unwrap()
            .contains(&handle(unreachable))));
    }
}

#[test]
fn a_template_target_cannot_equal_a_shorter_literal_ancestor_id() {
    let src = ownership_source(&format!(
        "column id=\"photo\" overflow=\"hidden\"\n  for row in [1, 2] key=row\n{}",
        indented(&indented(&photo("`photo-${row}`")))
    ));
    assert_eq!(owner_warnings(&src), [16]);
}

#[test]
fn complementary_sibling_guards_keep_transform_targets_exclusive() {
    let first = indented(&photo("\"first\""));
    let second = indented(&photo("\"second\""));
    for (a, b) in [("on", "not on"), ("not on", "on"), ("n == 1", "n != 1")] {
        let src = ownership_source(&format!("if {a}\n{first}if {b}\n{second}"));
        assert!(owner_warnings(&src).is_empty(), "{src}");
    }
    let src = ownership_source(&format!("if on\n{first}if n > 0\n{second}"));
    assert_eq!(owner_warnings(&src).len(), 2, "{src}");
    // A row-dependent guard can take both arms in different rows.
    let handle = |name| {
        format!("column transformDragFor=\"{name}\" transformgeometry=geometry transformrelease=release")
    };
    let src = nested_transform_targets(&format!(
        "for row in rows key=row\n  if row == 1\n{}  if row != 1\n{}",
        indented(&indented(&handle("outer"))),
        indented(&indented(&handle("inner")))
    ));
    assert_eq!(owner_warnings(&src).len(), 2, "{src}");
}

#[test]
fn a_match_arm_limits_a_transform_target_to_the_rows_that_take_it() {
    let arms = |subject: &str| {
        format!(
            "  match {subject}\n    case some(value)\n{}    case none\n      text \"x\"\n",
            indented(&indented(&indented(&photo("\"photo\""))))
        )
    };
    for (list, subject, count) in [
        ("[some(1), none]", "row", 0),
        ("[1, 2]", "(position == 0 ? some(1) : none)", 0),
        ("[some(1), some(2)]", "row", 1),
    ] {
        let src = ownership_source(&format!(
            "for row, position in {list} key=position\n{}",
            arms(subject)
        ));
        // A position key draws its own advice.
        let owners = super::warnings::warned(&src)
            .into_iter()
            .filter(|(id, _)| *id == OWNERS)
            .count();
        assert_eq!(owners, count, "{src}");
    }
}
