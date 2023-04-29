use std::ffi::{c_char, c_int, c_uint, CString};

pub fn set_main_loop(func: EmCallbackFunc) -> ! {
    unsafe { emscripten_set_main_loop(func, 0, 1) };
    // emscripten_set_main_loop with simulate_infinite_loop set to true will
    // throw an exception to stop execution of the caller, i.e. we never end up
    // here. The loop {} here just reflects the actual "return value" of
    // emscripten_set_main_loop, which would in this case be "!".
    loop {}
}

pub fn run_javascript(script: &str) {
    let mut script = Vec::from(script.as_bytes());
    script.push(0);
    let script = CString::from_vec_with_nul(script).unwrap();
    unsafe { emscripten_run_script(script.as_c_str().as_ptr()) };
}

pub type EmCallbackFunc = extern "C" fn();
extern "C" {
    /// https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_set_main_loop
    pub fn emscripten_set_main_loop(
        func: EmCallbackFunc,
        fps: c_int,
        simulate_infinite_loop: c_int,
    );

    /// https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_run_script
    pub fn emscripten_run_script(script: *const c_char);

    /// https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_sleep
    pub fn emscripten_sleep(ms: c_uint);
}
