//! A password field's text in `state` (LLP 1156.001 D6): the slot it writes,
//! and a derive that copies it, read `•••` as the tree shows the field; a
//! string that only contains it, and every other slot, read as they are;
//! `expect state` asks `reveal` and reads the text.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
}

const SRC: &str = r#"
component App
  state pw = "hunter2"
  state who = "alice"
  derive copy = pw
  derive said = "pw is " + pw
  action typed(v: string)
    pw = v
  view
    column
      input testId="who" value=who
      input type="password" testId="pw" value=pw input=typed
"#;

#[test]
fn state_masks_a_password_fields_text_unless_a_test_asks_to_compare() {
    let plan = contract::compile(SRC).expect("the fixture compiles");
    let r = Runner::boot(plan, NoData, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let state: serde_json::Value = serde_json::from_str(&agent::state(&r)).unwrap();
    assert_eq!(state["slots"]["pw"], "•••");
    assert_eq!(state["derives"]["copy"], "•••");
    assert_eq!(state["derives"]["said"], "pw is hunter2");
    assert_eq!(state["slots"]["who"], "alice");
    let revealed: serde_json::Value =
        serde_json::from_str(&agent::handle(&r, r#"{"op":"state","reveal":true}"#)).unwrap();
    assert_eq!(revealed["slots"]["pw"], "hunter2");
}
