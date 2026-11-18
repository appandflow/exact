//! Each production export answers its operation as the dispatcher does.
use super::*;
use exact_plan::Value;
use exact_runner::DataError;

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = r#"component App
  state n = 0
  action add
    n = n + 1
  view
    column
      button press=add testId="add"
        text `${n}`
"#;

fn booted() -> Bridge<NoData> {
    let plan = contract::compile(APP).unwrap().encode();
    let mut bridge = Bridge::new();
    bridge.boot(&plan, NoData, Hooks::none(), 400., 800.);
    bridge
}

type Export = fn(&mut Bridge<NoData>, usize) -> u32;

fn reply(bridge: &mut Bridge<NoData>, request: &str, f: Export) -> String {
    let len = bridge.input_write(request.as_bytes());
    let n = f(bridge, len);
    String::from_utf8(bridge.output_bytes(n as usize).to_vec()).unwrap()
}

/// Each case on two bridges booted alike, so a draining operation is
/// compared with its own first answer.
#[test]
fn each_production_export_answers_as_the_dispatcher() {
    let view = {
        let bridge = booted();
        let kernel = bridge.host.as_ref().unwrap().runner().kernel();
        kernel
            .node_by_key(kernel.find_by_test_id("add")[0])
            .unwrap()
            .id
    };
    let node = format!(r#"{{"op":"node","id":{view}}}"#);
    let planned = format!(r#"{{"op":"node","id":{view},"plan":true}}"#);
    let cases: [(&str, Export); 21] = [
        (r#"{"op":"outstanding"}"#, Bridge::outstanding),
        (r#"{"op":"background"}"#, Bridge::must_finish),
        (r#"{"op":"actions","drain":true}"#, Bridge::ended_actions),
        (r#"{"op":"declared"}"#, Bridge::declared),
        (r#"{"op":"tree"}"#, Bridge::tree),
        (r#"{"op":"state"}"#, Bridge::state),
        (&node, Bridge::node),
        (&planned, Bridge::node),
        (r#"{"op":"perf"}"#, Bridge::perf),
        (r#"{"op":"logs","since":0}"#, Bridge::logs),
        (r#"{"op":"tags"}"#, Bridge::tags),
        (r#"{"op":"picker","id":"none"}"#, Bridge::picker),
        (r#"{"op":"showPicker","id":"none"}"#, Bridge::show_picker),
        (r#"{"op":"pickedPath","name":"a.txt"}"#, Bridge::picked_path),
        (r#"{"op":"appRoots"}"#, Bridge::app_roots),
        (
            r#"{"op":"shareFile","path":"app:/data/none.txt","to":"/nonexistent/none.txt"}"#,
            Bridge::share_file,
        ),
        (r#"{"op":"keptDocuments","owner":1}"#, Bridge::documents),
        (
            r#"{"op":"frameRequest","node":1,"capability":"dialog","args":{}}"#,
            Bridge::frame_request,
        ),
        (r#"{"op":"called","id":1}"#, Bridge::tool),
        (r#"{"op":"call","args":}"#, Bridge::tool),
        (
            r#"{"op":"tap","ticket":7,"choice":"cancel"}"#,
            Bridge::answer_ticket,
        ),
    ];
    // `perf`'s numbers are measured work, which differs from run to run.
    let shape = |reply: String, request: &str| match request.contains("\"perf\"") {
        true => reply.replace(|c: char| c.is_ascii_digit() || c == '.', ""),
        false => reply,
    };
    for (request, export) in cases {
        let general = shape(reply(&mut booted(), request, Bridge::agent), request);
        assert_eq!(
            shape(reply(&mut booted(), request, export), request),
            general,
            "{request}"
        );
    }
}
