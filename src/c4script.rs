use crate::c4script_sys;
use std::ffi::{CStr, CString};
use std::os::raw::{c_void, c_char};

use c4script_sys::*;

pub enum DiagnosticSeverity {
    Warning,
    Error,
}

struct DiagnosticsCtx<'a> {
    diagnostic_fn: &'a mut dyn FnMut(DiagnosticSeverity, String),
}

extern "C" fn handle_error(ctx: *mut c_void, msg: *const c_char) {
    let ctx = unsafe { &mut *(ctx as *mut DiagnosticsCtx) };
    let msg = unsafe { CStr::from_ptr(msg) };
    (ctx.diagnostic_fn)(DiagnosticSeverity::Error, msg.to_string_lossy().to_string());
}

extern "C" fn handle_warning(ctx: *mut c_void, msg: *const c_char) {
    let ctx = unsafe { &mut *(ctx as *mut DiagnosticsCtx) };
    let msg = unsafe { CStr::from_ptr(msg) };
    (ctx.diagnostic_fn)(DiagnosticSeverity::Warning, msg.to_string_lossy().to_string());
}

/// Checks a script from a string, returning the number of errors.
/// For each error and warning message, the given functions are called.
pub fn check_string<'a, F: 'a>(script: &str, mut diagnostic_fn: F) -> i32 where F: FnMut(DiagnosticSeverity, String) {
    // TODO: Error handling (NUL bytes)
    let c_script = CString::new(script).expect("CString::new failed");
    let ctx = DiagnosticsCtx { diagnostic_fn: &mut diagnostic_fn };
    let mut handlers = c4s_errorhandlers {
        errors: Some(handle_error),
        warnings: Some(handle_warning),
        ctx: &ctx as *const _ as *mut c_void,
    };
    unsafe {
        c4script_sys::c4s_checkstring(c_script.as_ptr(), &mut handlers as *mut c4s_errorhandlers)
    }
}
