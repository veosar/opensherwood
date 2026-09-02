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
        drop(buf);
        discard_rest_of_line(reader)?;
        return Err(std::io::Error::other("line exceeds MAX_LINE_BYTES"));
    }
    Ok(Some(String::from_utf8_lossy(&buf).trim_end().to_string()))
}

/// Skip input up to and including the next `\n` (or EOF) without allocating: the discarded bytes
/// only ever pass through the reader's own buffer, however long the line is.
fn discard_rest_of_line(reader: &mut impl BufRead) -> std::io::Result<()> {
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(());
        }
        if let Some(i) = available.iter().position(|&b| b == b'\n') {
            reader.consume(i + 1);
            return Ok(());
        }
        let n = available.len();
        reader.consume(n);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn oversized_line_is_discarded_without_allocating_it() {
        // A 48 MiB line (three times the cap) is produced lazily by `repeat`, so the only place it
        // could ever be materialised is inside the reader; then a normal line must still arrive.
        let long = std::io::repeat(b'a').take(3 * MAX_LINE_BYTES as u64);
        let tail = Cursor::new(b"\n{\"ok\":1}\nlast".to_vec());
        let mut reader = BufReader::with_capacity(8 * 1024, long.chain(tail));
        let err = read_line_capped(&mut reader).unwrap_err();
        assert!(err.to_string().contains("MAX_LINE_BYTES"));
        assert_eq!(
            read_line_capped(&mut reader).unwrap().as_deref(),
            Some("{\"ok\":1}")
        );
        assert_eq!(
            read_line_capped(&mut reader).unwrap().as_deref(),
            Some("last")
        );
        assert_eq!(read_line_capped(&mut reader).unwrap(), None);
    }

    #[test]
    fn oversized_line_at_eof_is_discarded() {
        let long = std::io::repeat(b'x').take(MAX_LINE_BYTES as u64 + 1);
        let mut reader = BufReader::new(long);
        assert!(read_line_capped(&mut reader).is_err());
        assert_eq!(read_line_capped(&mut reader).unwrap(), None);
    }
}
