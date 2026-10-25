// A context slot keeps a `Weak<Context>` in the context's annex, with a
// guaranteed finalizer that frees the annex. Dropping the isolate drains that
// finalizer after the isolate handle is nulled, so the `Weak` is dropped while
// V8 still holds its weak global. With marking in progress, V8's teardown GC
// then runs the weak callback on the `WeakData`, which must still be alive.
#[test]
fn context_slot_weak_survives_isolate_teardown() {
  v8::V8::set_flags_from_string("--stress-compaction --stress-marking=1");
  v8::V8::initialize_platform(v8::new_default_platform(0, false).make_shared());
  v8::V8::initialize();

  for i in 0..10 {
    let mut isolate = v8::Isolate::new(Default::default());
    let late_weak;
    let cancelled = std::rc::Rc::new(std::cell::Cell::new(0));
    {
      v8::scope!(let scope, &mut isolate);
      let context = v8::Context::new(scope, Default::default());
      context.set_slot(std::rc::Rc::new(i));
      let value = v8::String::new(scope, "weak target").unwrap();
      late_weak = v8::Weak::new(scope, value);
      // A plain weak has no second-pass callback. It must still be reclaimed.
      let plain = v8::Weak::new(scope, value);
      context.set_slot(std::rc::Rc::new(plain));
      let counter = cancelled.clone();
      let cancelled_weak = v8::Weak::with_guaranteed_finalizer(
        scope,
        value,
        Box::new(move || counter.set(counter.get() + 1)),
      );
      // Isolate slots are dropped after invalidation but before the finaliser
      // map is drained. Dropping this weak must cancel its finaliser.
      scope.set_slot(cancelled_weak);
    }
    drop(isolate);
    assert_eq!(cancelled.get(), 0);
    drop(late_weak);
  }
  for i in 0..10 {
    let mut creator = v8::Isolate::snapshot_creator(None, None);
    {
      v8::scope!(let scope, &mut creator);
      let context = v8::Context::new(scope, Default::default());
      scope.set_default_context(context);
      // This unsnapshotted context's annex is finalised before CreateBlob;
      // its weak callback can still run during snapshot creation/disposal.
      let discarded = v8::Context::new(scope, Default::default());
      discarded.set_slot(std::rc::Rc::new(i));
    }
    assert!(
      creator
        .create_blob(v8::FunctionCodeHandling::Clear)
        .is_some()
    );
  }
}
