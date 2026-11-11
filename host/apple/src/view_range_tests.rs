//! A view timeline's named `animation-range` on Apple: a cover one scrollport
//! tall stretches from its bottom edge over `exit -100% exit 0%` as its
//! scroller is pulled past its start, the rubber band's negative offset,
//! while a sibling consumer stays on `cover`; and a photo shorter than the
//! scrollport, its own timeline's consumer, does the same by its own height.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_motion::Property;
use exact_plan::Value;
use exact_runner::DataError;

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = r##"keyframes stretch
  from scale=2
  to scale=1
keyframes pin
  from translate="0 -100%"
  to translate="0 0"
keyframes fade
  from opacity=0
  to opacity=1
component App
  view
    scroll testId="page" width=400 height=800
      column
        box testId="cover" height=1000 view-timeline="--cover y"
          box testId="photo" position="absolute" inset=0 transform-origin="50% 100%" animation="stretch 1s linear both" animation-timeline="--cover" animation-range="exit -100% exit 0%"
          box testId="words" position="absolute" inset=0 animation="fade 1s linear both" animation-timeline="--cover"
          box testId="copy" position="absolute" inset=0 pointer-events="none" animation="pin 1s linear both" animation-timeline="--cover" animation-range="exit -100% exit 0%"
        box testId="shot" width=400 height=600 view-timeline="--shot" transform-origin="50% 100%" animation="stretch 1s linear both" animation-timeline="--shot" animation-range="exit -100% exit 0%"
        box height=2000
"##;

fn id(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn node(host: &Host<NoData>, test_id: &str) -> u64 {
    let k = host.runner.kernel();
    exact_kernel::motion::motion_node(k.find_by_test_id(test_id)[0])
}

fn shown(host: &Host<NoData>, test_id: &str, property: Property) -> f64 {
    let n = node(host, test_id);
    host.engine
        .sampled_value(n, property)
        .unwrap_or_else(|| panic!("`{test_id}` shows no {property:?}"))
        .x
}

#[test]
fn a_named_range_stretches_a_cover_by_its_own_height_past_the_scrollers_start() {
    let plan = contract::compile(APP).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.0,
        800.0,
    )
    .unwrap();
    let page = id(&host, "page");
    let scale = |h: &Host<NoData>| shown(h, "photo", Property::Scale);
    assert!((scale(&host) - 1.0).abs() < 1e-6, "{}", scale(&host));
    // The words run over `cover`, 1800 long: at rest 800 / 1800 on.
    let words = shown(&host, "words", Property::Opacity);
    assert!((words - 800.0 / 1800.0).abs() < 1e-6, "{words}");

    let _ = host.view_scrolled(page, 0.0, -100.0);
    assert!((scale(&host) - 1.1).abs() < 1e-6, "{}", scale(&host));
    // A second consumer over the same range: moved back up by the pull, a
    // tenth of its own height.
    let copy = host
        .engine
        .sampled_value(node(&host, "copy"), Property::Translate)
        .unwrap();
    assert!((copy.w + 10.0).abs() < 1e-6, "{copy:?}");
    let words = shown(&host, "words", Property::Opacity);
    assert!((words - 700.0 / 1800.0).abs() < 1e-6, "{words}");

    let _ = host.view_scrolled(page, 0.0, -250.0);
    assert!((scale(&host) - 1.25).abs() < 1e-6, "{}", scale(&host));

    // Scrolled on, the range is behind it: held at 1.
    let _ = host.view_scrolled(page, 0.0, 300.0);
    assert!((scale(&host) - 1.0).abs() < 1e-6, "{}", scale(&host));

    // The photo, 600 tall at 1000, is 60 short of the scrollport's start at
    // 940: a tenth of its own height before its exit.
    let _ = host.view_scrolled(page, 0.0, 940.0);
    let shot = shown(&host, "shot", Property::Scale);
    assert!((shot - 1.1).abs() < 1e-6, "{shot}");
}
