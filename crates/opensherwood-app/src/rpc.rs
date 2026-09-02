//! JSON-RPC 2.0 transport over stdio: a blocking loop for headless mode and a reader thread for
//! window mode. Requests are validated per the specification: `jsonrpc` must be "2.0", `method` a
//! string, `id` absent (notification), a number, a string or null; notifications get no response.

use std::io::{BufRead, Read, Write};
use std::sync::mpsc;

use opensherwood_protocol::{Response, RpcError};
use serde_json::Value;

use crate::engine::Session;

/// Largest accepted request line in bytes.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

/// Outcome of handling one line.
#[derive(Debug)]
pub struct Handled {
    /// Response to write (none for notifications).
    pub response: Option<Response>,
    /// The request was `shutdown`.
    pub shutdown: bool,
}

/// Handle one request line.
pub fn handle_line(session: &mut Session, line: &str) -> Handled {
    if line.len() > MAX_LINE_BYTES {
        return Handled {
            response: Some(Response::err(
                Value::Null,
                RpcError::new(RpcError::INVALID_REQUEST, "request too large"),
            )),
            shutdown: false,
        };
    }
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Handled {
                response: Some(Response::err(
                    Value::Null,
                    RpcError::new(RpcError::PARSE, e.to_string()),
                )),
                shutdown: false,
            };
        }
    };
    let Value::Object(mut obj) = value else {
        return invalid(Value::Null, "request must be an object");
    };
    let id = match obj.remove("id") {
        None => None,
        Some(v @ (Value::Null | Value::Number(_) | Value::String(_))) => Some(v),
        Some(_) => return invalid(Value::Null, "id must be a number, a string or null"),
    };
    let id_for_errors = id.clone().unwrap_or(Value::Null);
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return invalid(id_for_errors, "jsonrpc must be \"2.0\"");
    }
    let Some(method) = obj.get("method").and_then(Value::as_str).map(str::to_owned) else {
        return invalid(id_for_errors, "method must be a string");
    };
    let params = match obj.remove("params") {
        None => None,
        Some(v @ (Value::Object(_) | Value::Array(_))) => Some(v),
        Some(_) => return invalid(id_for_errors, "params must be an object or an array"),
    };
    let shutdown = method == "shutdown";
    let result = session.dispatch(&method, params);
    let response = id.map(|id| match result {
        Ok(v) => Response::ok(id, v),
        Err(e) => Response::err(id, e),
    });
    Handled { response, shutdown }
}

fn invalid(id: Value, message: &str) -> Handled {
    Handled {
        response: Some(Response::err(
            id,
            RpcError::new(RpcError::INVALID_REQUEST, message),
        )),
        shutdown: false,
    }
}

/// Write one response line to stdout.
pub fn write_response(resp: &Response) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    serde_json::to_writer(&mut out, resp)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

/// Read one line with a size cap; `Ok(None)` at EOF, `Err` when the line is too long.
fn read_line_capped(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut buf = Vec::new();
    let n = reader
        .by_ref()
        .take(MAX_LINE_BYTES as u64 + 1)
        .read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > MAX_LINE_BYTES {
        // Drain the rest of the oversized line so the stream stays in sync.
        let mut sink = Vec::new();
        reader.read_until(b'\n', &mut sink)?;
        return Err(std::io::Error::other("line exceeds MAX_LINE_BYTES"));
    }
    Ok(Some(String::from_utf8_lossy(&buf).trim_end().to_string()))
}

/// Serve requests from stdin until EOF or `shutdown` (headless mode).
pub fn serve_stdio(mut session: Session) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    loop {
        let line = match read_line_capped(&mut reader) {
            Ok(Some(l)) => l,
            Ok(None) => return Ok(()),
            Err(_) => {
                write_response(&Response::err(
                    Value::Null,
                    RpcError::new(RpcError::INVALID_REQUEST, "request too large"),
                ))?;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let handled = handle_line(&mut session, &line);
        if let Some(resp) = &handled.response {
            write_response(resp)?;
        }
        if handled.shutdown {
            return Ok(());
        }
    }
}

/// Spawn a thread that forwards stdin lines to a bounded channel (window mode).
pub fn spawn_stdin_reader() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::sync_channel(64);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        loop {
            match read_line_capped(&mut reader) {
                Ok(Some(l)) if !l.trim().is_empty() => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
    });
    rx
}
