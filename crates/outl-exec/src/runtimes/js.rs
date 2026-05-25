//! `js` runtime — JavaScript via [Boa](https://boajs.dev).
//!
//! Boa is a JS engine in pure Rust, ES2015+ with ongoing work toward
//! full ECMAScript conformance. Good enough for snippets in notes
//! ("compute the slug of this title", "format this date"), nowhere
//! near production V8.
//!
//! We expose a single native `__outl_log` and prepend a tiny shim that
//! wires it into `console.log` / `.warn` / `.error`, so user code can
//! call `console.log(...)` naturally and the output lands in our
//! buffer.
//!
//! Gated behind the `lang-js` feature.

// Boa's only way to register a *capturing* native function is
// `NativeFunction::from_closure`, which is `unsafe` because the
// closure must not capture data that's `!Send` in a way that escapes
// `Context`'s lifetime. Our closure captures an `Rc<RefCell<String>>`
// we own throughout `execute`, so the invariant holds trivially.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use boa_engine::{js_string, Context, JsValue, NativeFunction, Source};

use crate::runtime::{ExecContext, ExecError, ExecOutput, ExitStatus, Runtime};

/// Boa-backed JavaScript runtime.
pub struct JsRuntime;

const CONSOLE_SHIM: &str = r#"
globalThis.console = {
    log:   (...a) => __outl_log(a.map(x => String(x)).join(' ') + '\n'),
    warn:  (...a) => __outl_log(a.map(x => String(x)).join(' ') + '\n'),
    error: (...a) => __outl_log(a.map(x => String(x)).join(' ') + '\n'),
    info:  (...a) => __outl_log(a.map(x => String(x)).join(' ') + '\n'),
};
"#;

impl Runtime for JsRuntime {
    fn language(&self) -> &'static str {
        "js"
    }

    fn execute(&self, source: &str, _ctx: &ExecContext) -> Result<ExecOutput, ExecError> {
        let start = Instant::now();
        let mut context = Context::default();
        let sink: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

        // Register `__outl_log(string)` as a native fn that pushes
        // into the shared buffer. The shim above turns `console.log`
        // calls into invocations of this.
        let log_sink = sink.clone();
        let log_fn = unsafe {
            NativeFunction::from_closure(move |_, args, ctx| {
                if let Some(arg) = args.first() {
                    let s = arg.to_string(ctx)?;
                    log_sink.borrow_mut().push_str(&s.to_std_string_escaped());
                }
                Ok(JsValue::undefined())
            })
        };
        context
            .register_global_callable(js_string!("__outl_log"), 1, log_fn)
            .map_err(|e| ExecError::Sandbox(format!("register __outl_log: {e}")))?;
        // Run the shim that wires console.log → __outl_log. Errors
        // here would mean a broken Boa install, so just panic-via-?.
        let _ = context
            .eval(Source::from_bytes(CONSOLE_SHIM))
            .map_err(|e| ExecError::Sandbox(format!("console shim: {e}")))?;

        // Don't carry the shim's `undefined` over as the auto-print
        // value — only the user script's last expression matters.
        let value = match context.eval(Source::from_bytes(source)) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ExecOutput {
                    stdout: sink.borrow().clone(),
                    stderr: e.to_string(),
                    duration: start.elapsed(),
                    exit: ExitStatus::Trap("js-error".into()),
                });
            }
        };

        let mut stdout = sink.borrow().clone();
        if stdout.is_empty() && !value.is_undefined() {
            let s = value
                .to_string(&mut context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|_| format!("{value:?}"));
            stdout.push_str(&s);
        }
        Ok(ExecOutput {
            stdout,
            stderr: String::new(),
            duration: start.elapsed(),
            exit: ExitStatus::Ok,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> String {
        JsRuntime
            .execute(src, &ExecContext::default())
            .unwrap()
            .stdout
    }

    #[test]
    fn arithmetic_last_value_auto_printed() {
        assert_eq!(run("1 + 2"), "3");
    }

    #[test]
    fn console_log_writes_stdout() {
        assert_eq!(run("console.log('hello')"), "hello\n");
    }

    #[test]
    fn template_literals() {
        assert_eq!(run("`x=${2+3}`"), "x=5");
    }

    #[test]
    fn arrow_fn_and_map() {
        assert_eq!(run("[1,2,3].map(n => n * n).join(',')"), "1,4,9");
    }

    #[test]
    fn parse_error_returns_trap() {
        let out = JsRuntime
            .execute("function (", &ExecContext::default())
            .unwrap();
        assert!(matches!(out.exit, ExitStatus::Trap(_)));
        assert!(!out.stderr.is_empty());
    }
}
