//! CSS Containment 2 §3.2: a box with `contain: layout` is the containing
//! block of its absolutely positioned descendants, so the compiler lowers it
//! `position: relative` as it does every box that is one on every host
//! (LLP 1074 T1); a literal `none` contains nothing.

use crate::scroll_timelines::NoData;
use exact_kernel::{Kernel, PositionType};
use exact_runner::Runner;

#[test]
fn a_box_with_layout_containment_is_lowered_relative() {
    let plan = contract::compile(
        r#"component App
  view
    column
      box testId="contains" contain="layout"
        box position="absolute"
      box testId="contains-none" contain="none"
        box position="absolute"
"#,
    )
    .unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    for (id, position) in [
        ("contains", PositionType::Relative),
        ("contains-none", PositionType::Static),
    ] {
        let key = k.find_by_test_id(id)[0];
        assert_eq!(
            k.node_by_key(key).unwrap().style.position_type,
            position,
            "{id}"
        );
    }
}
