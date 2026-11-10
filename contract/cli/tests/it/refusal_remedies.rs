//! Refusals that name the input that would succeed (LLP 1156.002 D8, D9):
//! a handler chosen by a condition, and a helper written above the root.

fn refused(src: &str) -> contract::CompileError {
    contract::compile(src).expect_err("refused")
}

#[test]
fn a_conditional_handler_is_told_to_be_one_action_that_chooses() {
    let e = refused(
        "component App\n  state searching = false\n  action close\n    searching = false\n  action closeSearch\n    searching = false\n  view\n    button \"Done\" press=(searching ? closeSearch : close)\n",
    );
    assert_eq!(e.id, "analyze-handler-shape");
    assert!(e.message.contains("not a condition: write one action that chooses"), "{}", e.message);
    assert!(e.message.contains("`press=pick`"), "{}", e.message);
}

#[test]
fn a_helper_above_the_root_is_told_to_move_below_it() {
    let e = refused(
        "component Card\n  props\n    title: string\n  view\n    text \"card\"\n\ncomponent App\n  view\n    Card title=\"a\"\n",
    );
    assert_eq!(e.id, "analyze-root-props", "{}", e.message);
    assert!(e.message.ends_with("if `Card` is a helper, move it below the component the app starts from"), "{}", e.message);
    // A file of one component has nothing to move.
    let alone = refused("component App\n  props\n    title: string\n  view\n    text \"card\"\n");
    assert_eq!(alone.message, "the root component (the first in the file) takes no props");
}
