//! JNI initialization of Android certificate verification.
#![allow(unsafe_code)]

/// The reader initializes certificate verification before any host can fetch.
#[macro_export]
macro_rules! canvas_jni_tls {
    () => {
        /// Initialize TLS from the reader's application context.
        ///
        /// # Safety
        /// The pointers must belong to this call's JNI thread and local frame.
        #[no_mangle]
        pub unsafe extern "system" fn Java_dev_exact_canvas_Native_trust(
            env: *mut $crate::canvas::jni_sys::JNIEnv,
            _class: $crate::canvas::jni_sys::jclass,
            context: $crate::canvas::jni_sys::jobject,
        ) {
            unsafe { $crate::canvas::tls::initialize(env.cast(), context.cast()) };
        }
    };
}

/// Initialize the process's Android trust manager from a JNI call.
///
/// # Safety
/// The pointers must belong to the calling thread's current JNI local frame.
pub unsafe fn initialize(env: *mut std::ffi::c_void, context: *mut std::ffi::c_void) {
    unsafe { ibex2::transport::initialize_android(env, context) };
}
