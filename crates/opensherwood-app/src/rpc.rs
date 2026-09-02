//! JSON-RPC transport over stdio: a blocking loop for headless mode and a reader thread for
//! window mode.

use std::io::{BufRead, Write};
use std::sync::mpsc;

use opensherwood_protocol::{Request, Response, RpcError};
use serde_json::Value;

use crate::engine::Session;

/// Handle one request line and return the response (and whether it was a shutdown).
pub fn handle_line(session: &mut Session, line: &str) -> (Response, bool) {
    match serde_json::from_str::<Request>(line) {
        Err(e) => (
            Response::err(Value::Null, RpcError::new(RpcError::PARSE, e.to_string())),
            false,
        ),
        Ok(req) => {
            let id = req.id.clone().unwrap_or(Value::Null);
            let is_shutdown = req.method == "shutdown";
            let resp = match session.dispatch(&req.method, req.params) {
                Ok(v) => Response::ok(id, v),
                Err(e) => Response::err(id, e),
            };
            (resp, is_shutdown)
        }
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

/// Serve requests from stdin until EOF or `shutdown` (headless mode).
pub fn serve_stdio(mut session: Session) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let (resp, shutdown) = handle_line(&mut session, &line);
        write_response(&resp)?;
        if shutdown {
            return Ok(());
        }
    }
    Ok(())
}

/// Spawn a thread that forwards stdin lines to a channel (window mode).
pub fn spawn_stdin_reader() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) if !l.trim().is_empty() => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
    rx
}
