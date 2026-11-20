//! CSS Containment 2 §3.2: a box with layout containment treats its
//! contents' overflow as ink overflow. Chrome 156, 2026-11-20: a 300px
//! scroller holding a relatively positioned 500px column whose absolute
//! child reaches 1000px past both its edges has a `scrollHeight` of 1500,
//! and of 500 when the column has `contain: layout`.

use exact_kernel::{Kernel, NodeType, Offer, Op, StyleId, StyleProps, StyleValue};

fn style(rows: &[(StyleId, StyleValue)]) -> StyleProps {
    let mut s = StyleProps::default();
    for (id, value) in rows {
        s.set_dynamic(*id, value).unwrap();
    }
    s
}

/// The scroller (2), its column (3), the column's content (4) and the
/// absolute layer reaching past it (5); returns the scroller's extent.
fn extent(contain: Option<&'static str>) -> f32 {
    let mut kernel = Kernel::with_monospace();
    let n = |v: f64| StyleValue::Number(v);
    let t = |v: &'static str| StyleValue::Text(v.into());
    let mut column = vec![(StyleId::PositionType, t("relative"))];
    if let Some(value) = contain {
        column.push((StyleId::Contain, t(value)));
    }
    let mut ops = Vec::new();
    for id in 1..=5 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.extend([
        Op::SetStyle {
            id: 2,
            patch: Box::new(style(&[
                (StyleId::Height, n(300.0)),
                (StyleId::OverflowY, t("auto")),
            ])),
        },
        Op::SetStyle {
            id: 3,
            patch: Box::new(style(&column)),
        },
        Op::SetStyle {
            id: 4,
            patch: Box::new(style(&[(StyleId::Height, n(500.0))])),
        },
        Op::SetStyle {
            id: 5,
            patch: Box::new(style(&[
                (StyleId::PositionType, t("absolute")),
                (StyleId::Top, n(-1000.0)),
                (StyleId::Bottom, n(-1000.0)),
                (StyleId::Left, n(0.0)),
                (StyleId::Right, n(0.0)),
            ])),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 3,
            children: vec![5, 4],
        },
        Op::AttachRoot { id: 1 },
    ]);
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 800.0))
        .unwrap();
    let layer = kernel.node(5).unwrap().frame;
    assert_eq!(
        (layer.y, layer.height),
        (-1000.0, 2500.0),
        "the layer reaches past the column either way"
    );
    kernel.node(2).unwrap().content.1
}

#[test]
fn layout_containment_keeps_overflow_out_of_the_scrollers_extent() {
    assert_eq!(
        extent(None),
        1500.0,
        "Chrome's scrollHeight without containment"
    );
    assert_eq!(extent(Some("none")), 1500.0, "`none` is no containment");
    assert_eq!(
        extent(Some("layout")),
        500.0,
        "Chrome's scrollHeight with `contain: layout`"
    );
}

/// Chrome 157.0.8103.79, 16px/18px monospace. A static box with layout
/// containment, which the compiler lowers `position: relative`, contains its
/// absolutely positioned child; without containment the child is placed
/// against the root.
#[test]
fn a_contained_box_contains_its_absolute_child() {
    use crate::browser_cases::{css_rows, lay_out_with, mismatches, props};
    let mut wrong = Vec::new();
    for (css, expected) in [
        ("", [20.0, 10.0, 30.0, 40.0]),
        ("contain:layout;position:relative", [20.0, 40.0, 30.0, 40.0]),
    ] {
        let k = lay_out_with(
            props(&css_rows("width:800px")),
            vec![
                (
                    2,
                    1,
                    css_rows(&format!("margin-top:30px;width:100px;height:100px;{css}")),
                ),
                (
                    3,
                    2,
                    css_rows("position:absolute;top:10px;left:20px;width:30px;height:40px"),
                ),
            ],
            &[],
            &[],
        );
        wrong.extend(mismatches(css, &k, &[(3, expected)]));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Chrome 157.0.8103.79, 18px line boxes in a baseline-aligned row: a text
/// with layout containment has no baseline, so the row aligns its border
/// box's bottom edge with its sibling's baseline (Chrome: the sibling at 34).
#[test]
fn a_contained_text_has_no_baseline() {
    use crate::browser_cases::{css_rows, lay_out_with, mismatches, props};
    let mut wrong = Vec::new();
    // The test measurer's monospace ascent is 14.4 points where Chrome's
    // is 14: the contained text sits 0.4 higher here.
    for (css, first) in [("", 30.0), ("contain:layout", 33.6)] {
        let k = lay_out_with(
            props(&css_rows(
                "display:flex;align-items:baseline;width:400px;line-height:18px",
            )),
            vec![
                (2, 1, css_rows("width:100px")),
                (
                    3,
                    1,
                    css_rows(&format!("width:100px;padding-top:30px;{css}")),
                ),
            ],
            &[(2, "X"), (3, "X")],
            &[],
        );
        wrong.extend(mismatches(
            css,
            &k,
            &[
                (2, [0.0, first, 100.0, 18.0]),
                (3, [100.0, 0.0, 100.0, 48.0]),
            ],
        ));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
