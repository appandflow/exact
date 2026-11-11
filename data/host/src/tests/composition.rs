use super::*;

#[test]
fn native_storage_changes_reach_both_mixed_children_through_storage() {
    use std::{cell::Cell, rc::Rc};
    struct Changed(Rc<Cell<bool>>);
    impl DataSource for Changed {
        fn app_id(&self) -> &str {
            "test.storage.changed"
        }
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::Unit)
        }
        fn note_storage_changed(&mut self) {
            self.0.set(true);
        }
        fn take_storage_changed(&mut self) -> bool {
            self.0.replace(false)
        }
    }
    let javascript = Rc::new(Cell::new(false));
    let rust = Rc::new(Cell::new(false));
    let mut source = Storage::new(
        exact_data::Mixed::new(Changed(javascript.clone()), Changed(rust.clone()), &[], &[])
            .unwrap(),
    );
    assert!(!source.take_storage_changed());
    for _ in 0..2 {
        source.note_storage_changed();
        assert!(javascript.get(), "the JavaScript child is notified");
        assert!(rust.get(), "the Rust child is notified");
        assert!(source.take_storage_changed());
        assert!(!javascript.get() && !rust.get(), "both flags are consumed");
        assert!(!source.take_storage_changed());
    }
}

#[test]
fn mixed_storage_waits_for_both_images_without_activating_sources() {
    use std::{cell::Cell, rc::Rc};
    struct Preparing {
        ready: Rc<Cell<bool>>,
        polls: Rc<Cell<usize>>,
    }
    impl DataSource for Preparing {
        fn app_id(&self) -> &str {
            "test.preload"
        }
        fn preload(&self) -> Result<bool, DataError> {
            self.polls.set(self.polls.get() + 1);
            Ok(self.ready.get())
        }
        fn query(
            &mut self,
            _: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, DataError> {
            panic!("image preparation must not run app queries")
        }
        fn activate(&mut self) -> Result<(), DataError> {
            panic!("image preparation must not activate app instances")
        }
    }
    let first = Rc::new(Cell::new(false));
    let second = Rc::new(Cell::new(false));
    let polls = Rc::new(Cell::new(0));
    let source = Storage::new(
        exact_data::Mixed::new(
            Preparing {
                ready: first.clone(),
                polls: polls.clone(),
            },
            Preparing {
                ready: second.clone(),
                polls: polls.clone(),
            },
            &["first"],
            &["second"],
        )
        .unwrap(),
    );
    assert!(!source.preload().unwrap());
    assert_eq!(
        polls.get(),
        2,
        "both images start even while the first is pending"
    );
    first.set(true);
    assert!(!source.preload().unwrap());
    second.set(true);
    assert!(source.preload().unwrap());
    assert!(!source.ready(), "preloading does not activate storage");
}
