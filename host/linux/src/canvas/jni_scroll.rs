//! The scroller's JNI methods, expanded by `canvas_jni!` inside its module,
//! where `with` reaches the thread's host.

#[doc(hidden)]
#[macro_export]
macro_rules! canvas_jni_scroll {
    () => {
        /// The reader moved scroller `id` by (`dx`, `dy`) pixels,
        /// knowing revision `rev` of it.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_scrolled(
            _env: *mut $crate::canvas::jni_sys::JNIEnv,
            _class: $crate::canvas::jni_sys::jclass,
            id: $crate::canvas::jni_sys::jint,
            dx: $crate::canvas::jni_sys::jfloat,
            dy: $crate::canvas::jni_sys::jfloat,
            rev: $crate::canvas::jni_sys::jint,
        ) {
            with(|h| h.scrolled(id as u32, dx, dy, rev as u32));
        }

        /// A finger began dragging scroller `id`, or (`ended`) let go of it
        /// (`CanvasHost::drag_began`, `drag_ended`; px/s, px).
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_scrollDrag(
            _env: *mut $crate::canvas::jni_sys::JNIEnv,
            _class: $crate::canvas::jni_sys::jclass,
            id: $crate::canvas::jni_sys::jint,
            ended: $crate::canvas::jni_sys::jboolean,
            vx: $crate::canvas::jni_sys::jfloat,
            vy: $crate::canvas::jni_sys::jfloat,
            rest: $crate::canvas::jni_sys::jboolean,
            rest_x: $crate::canvas::jni_sys::jfloat,
            rest_y: $crate::canvas::jni_sys::jfloat,
        ) {
            let rest = (rest != 0).then_some((rest_x, rest_y));
            match ended != 0 {
                false => with(|h| h.drag_began(id as u32)),
                true => with(|h| h.drag_ended(id as u32, vx, vy, rest)),
            };
        }
    };
}
