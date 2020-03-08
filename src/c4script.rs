use crate::c4script_sys;
use std::ffi::{CStr, CString};
use std::os::raw::{c_void, c_char};

use lsp_types::{Range, Position};
use c4script_sys::*;

pub enum DiagnosticSeverity {
    Warning,
    Error,
}

/// Position in script where an error or warning occured.
pub struct DiagnosticPosition {
    pub file: String,
    pub function: String, /// empty string if outside function
    pub line: u64, /// starting at line 1
    pub column: u64, /// starting at column 1
    pub length: u64,
}

impl DiagnosticPosition {
    /// Adapts a DiagnosticPosition from FFI, returning None for invalid positions.
    fn from_c4s(pos: &c4s_diagnostic_position) -> Option<DiagnosticPosition> {
        if pos.valid > 0 {
            Some(DiagnosticPosition {
                file: unsafe { CStr::from_ptr(pos.file) }.to_string_lossy().to_string(),
                function: unsafe { CStr::from_ptr(pos.function) }.to_string_lossy().to_string(),
                line: pos.line,
                column: pos.column,
                length: pos.length,
            })
        } else {
            None
        }
    }

    /// Converts to an LSP range.
    pub fn to_range(&self) -> Range {
        let line = self.line - 1;
        let character = self.column - 1;
        Range {
            start: Position { line, character },
            end: Position { line, character: character + self.length } ,
        }
    }
}

struct DiagnosticsCtx<'a> {
    diagnostic_fn: &'a mut dyn FnMut(DiagnosticSeverity, String, Option<DiagnosticPosition>),
}

extern "C" fn handle_error(ctx: *mut c_void, msg: *const c_char, pos: c4s_diagnostic_position) {
    let ctx = unsafe { &mut *(ctx as *mut DiagnosticsCtx) };
    let msg = unsafe { CStr::from_ptr(msg) };
    (ctx.diagnostic_fn)(DiagnosticSeverity::Error,
        msg.to_string_lossy().to_string(),
        DiagnosticPosition::from_c4s(&pos));
}

extern "C" fn handle_warning(ctx: *mut c_void, msg: *const c_char, pos: c4s_diagnostic_position) {
    let ctx = unsafe { &mut *(ctx as *mut DiagnosticsCtx) };
    let msg = unsafe { CStr::from_ptr(msg) };
    (ctx.diagnostic_fn)(DiagnosticSeverity::Warning,
        msg.to_string_lossy().to_string(),
        DiagnosticPosition::from_c4s(&pos));
}

/// Checks a script from a string, returning the number of errors.
/// For each error and warning message, the given functions are called.
pub fn check_string<'a, F: 'a>(script: &str, mut diagnostic_fn: F) -> i32
where F: FnMut(DiagnosticSeverity, String, Option<DiagnosticPosition>) {
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
