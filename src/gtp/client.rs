use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::player::Player;
use crate::r#move::Move;

use super::error::{GenmoveResult, GtpError, GtpProcessReport};
use super::protocol::{format_command, parse_response};
use super::vertex::{move_to_gtp, player_to_gtp};

const MAX_CAPTURED_STDERR_BYTES: usize = 16 * 1024;
const STDERR_READ_CHUNK_BYTES: usize = 4096;

struct StderrCapture {
    buffer: Arc<Mutex<VecDeque<u8>>>,
    handle: Option<JoinHandle<()>>,
}

impl StderrCapture {
    fn new(mut stderr: ChildStderr) -> Self {
        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(
            MAX_CAPTURED_STDERR_BYTES,
        )));
        let thread_buffer = Arc::clone(&buffer);
        let handle = thread::spawn(move || {
            let mut chunk = [0u8; STDERR_READ_CHUNK_BYTES];
            loop {
                let bytes_read = match stderr.read(&mut chunk) {
                    Ok(0) => return,
                    Ok(bytes_read) => bytes_read,
                    Err(_) => return,
                };

                let Ok(mut buffer) = thread_buffer.lock() else {
                    return;
                };
                for byte in &chunk[..bytes_read] {
                    if buffer.len() == MAX_CAPTURED_STDERR_BYTES {
                        buffer.pop_front();
                    }
                    buffer.push_back(*byte);
                }
            }
        });

        Self {
            buffer,
            handle: Some(handle),
        }
    }

    fn captured_text(&self) -> String {
        let Ok(buffer) = self.buffer.lock() else {
            return "<stderr capture unavailable>".to_string();
        };
        let bytes: Vec<u8> = buffer.iter().copied().collect();
        String::from_utf8_lossy(&bytes).trim_end().to_string()
    }

    fn join(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        let _ = handle.join();
    }
}

/// A raw GTP client that communicates with an engine subprocess.
pub struct GtpClient {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: StderrCapture,
    program: String,
    args: Vec<String>,
    next_id: u32,
}

impl GtpClient {
    /// Spawn a new GTP engine process.
    pub fn new(program: &str, args: &[&str]) -> Result<Self, GtpError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| GtpError::Protocol("failed to open stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GtpError::Protocol("failed to open stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| GtpError::Protocol("failed to open stderr".to_string()))?;

        Ok(GtpClient {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            stderr: StderrCapture::new(stderr),
            program: program.to_string(),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            next_id: 1,
        })
    }

    fn process_report(&mut self, cmd: &str, args: &[&str]) -> GtpProcessReport {
        let exit_status = match self.child.try_wait() {
            Ok(Some(status)) => {
                self.stderr.join();
                Some(status.to_string())
            }
            Ok(None) => None,
            Err(err) => Some(format!("failed to read process status: {err}")),
        };

        GtpProcessReport {
            command: format_engine_command(&self.program, &self.args),
            gtp_command: format_gtp_command_for_report(cmd, args),
            exit_status,
            stderr: self.stderr.captured_text(),
        }
    }

    fn io_error_with_context(&mut self, err: std::io::Error, cmd: &str, args: &[&str]) -> GtpError {
        match err.kind() {
            std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::UnexpectedEof => {
                GtpError::ProcessNotRunning(self.process_report(cmd, args))
            }
            _ => GtpError::Io(err),
        }
    }

    /// Send a raw GTP command and return the response content.
    pub fn send_command(&mut self, cmd: &str, args: &[&str]) -> Result<String, GtpError> {
        let id = self.next_id;
        self.next_id += 1;

        let formatted = format_command(id, cmd, args);
        if let Err(err) = self.stdin.write_all(formatted.as_bytes()) {
            return Err(self.io_error_with_context(err, cmd, args));
        }
        if let Err(err) = self.stdin.flush() {
            return Err(self.io_error_with_context(err, cmd, args));
        }

        // Read response lines until we get an empty line
        let mut response_text = String::new();
        loop {
            let mut line = String::new();
            let bytes = match self.stdout.read_line(&mut line) {
                Ok(bytes) => bytes,
                Err(err) => return Err(self.io_error_with_context(err, cmd, args)),
            };
            if bytes == 0 {
                return Err(GtpError::ProcessNotRunning(self.process_report(cmd, args)));
            }
            if line.trim().is_empty() {
                if !response_text.is_empty() {
                    break;
                }
                // Skip leading empty lines
                continue;
            }
            if !response_text.is_empty() {
                response_text.push('\n');
            }
            response_text.push_str(line.trim_end());
        }

        let resp = parse_response(&response_text)?;
        if resp.success {
            Ok(resp.content)
        } else {
            Err(GtpError::EngineError(resp.content))
        }
    }

    // -------------------------------------------------------------------------
    // Typed GTP command wrappers
    // -------------------------------------------------------------------------

    pub fn protocol_version(&mut self) -> Result<String, GtpError> {
        self.send_command("protocol_version", &[])
    }

    pub fn name(&mut self) -> Result<String, GtpError> {
        self.send_command("name", &[])
    }

    pub fn version(&mut self) -> Result<String, GtpError> {
        self.send_command("version", &[])
    }

    pub fn known_command(&mut self, cmd: &str) -> Result<bool, GtpError> {
        let resp = self.send_command("known_command", &[cmd])?;
        Ok(resp.trim().eq_ignore_ascii_case("true"))
    }

    pub fn list_commands(&mut self) -> Result<Vec<String>, GtpError> {
        let resp = self.send_command("list_commands", &[])?;
        Ok(resp.lines().map(|l| l.trim().to_string()).collect())
    }

    pub fn boardsize(&mut self, size: u8) -> Result<(), GtpError> {
        let s = size.to_string();
        self.send_command("boardsize", &[&s])?;
        Ok(())
    }

    pub fn clear_board(&mut self) -> Result<(), GtpError> {
        self.send_command("clear_board", &[])?;
        Ok(())
    }

    pub fn komi(&mut self, komi: f32) -> Result<(), GtpError> {
        let s = format!("{}", komi);
        self.send_command("komi", &[&s])?;
        Ok(())
    }

    pub fn play(&mut self, player: Player, m: &Move, board_height: u8) -> Result<(), GtpError> {
        let color = player_to_gtp(player);
        let vertex = move_to_gtp(m, board_height);
        self.send_command("play", &[color, &vertex])?;
        Ok(())
    }

    pub fn genmove(&mut self, player: Player, board_height: u8) -> Result<GenmoveResult, GtpError> {
        let color = player_to_gtp(player);
        let resp = self.send_command("genmove", &[color])?;
        let lower = resp.trim().to_lowercase();
        if lower == "resign" {
            Ok(GenmoveResult::Resign)
        } else if lower == "pass" {
            Ok(GenmoveResult::Move(Move::pass()))
        } else {
            let pos = super::vertex::vertex_to_position(&resp, board_height)?;
            Ok(GenmoveResult::Move(Move::place(pos.col, pos.row)))
        }
    }

    pub fn undo(&mut self) -> Result<(), GtpError> {
        self.send_command("undo", &[])?;
        Ok(())
    }

    pub fn showboard(&mut self) -> Result<String, GtpError> {
        self.send_command("showboard", &[])
    }

    pub fn final_score(&mut self) -> Result<String, GtpError> {
        self.send_command("final_score", &[])
    }

    pub fn quit(&mut self) -> Result<(), GtpError> {
        // Ignore errors from quit — the engine might already be gone
        let _ = self.send_command("quit", &[]);
        Ok(())
    }
}

impl Drop for GtpClient {
    fn drop(&mut self) {
        let _ = self.send_command("quit", &[]);
        let _ = self.child.wait();
        self.stderr.join();
    }
}

fn quote_command_part(part: &str) -> String {
    if part.is_empty() {
        return "''".to_string();
    }

    if part.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | '=' | ':' | ',')
    }) {
        return part.to_string();
    }

    format!("'{}'", part.replace('\'', "'\\''"))
}

fn format_engine_command(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_command_part(program));
    for arg in args {
        parts.push(quote_command_part(arg));
    }
    parts.join(" ")
}

fn format_gtp_command_for_report(cmd: &str, args: &[&str]) -> String {
    if args.is_empty() {
        return cmd.to_string();
    }

    format!("{} {}", cmd, args.join(" "))
}
