use std::fmt;

use crate::game::SetupError;
use crate::r#move::Move;

#[derive(Debug, Clone)]
pub struct GtpProcessReport {
    pub command: String,
    pub gtp_command: String,
    pub exit_status: Option<String>,
    pub stderr: String,
}

/// Errors that can occur during GTP communication.
#[derive(Debug)]
pub enum GtpError {
    Io(std::io::Error),
    Protocol(String),
    EngineError(String),
    InvalidVertex(String),
    InvalidColor(String),
    InvalidMove(String),
    InvalidSetup(SetupError),
    ProcessNotRunning(GtpProcessReport),
    UnsupportedBoardSize(u8),
}

/// Result of a `genmove` command — the engine can play a move, pass, or resign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenmoveResult {
    Move(Move),
    Resign,
}

impl From<std::io::Error> for GtpError {
    fn from(e: std::io::Error) -> Self {
        GtpError::Io(e)
    }
}

impl fmt::Display for GtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GtpError::Io(e) => write!(f, "GTP I/O error: {}", e),
            GtpError::Protocol(msg) => write!(f, "GTP protocol error: {}", msg),
            GtpError::EngineError(msg) => write!(f, "GTP engine error: {}", msg),
            GtpError::InvalidVertex(v) => write!(f, "invalid GTP vertex: {}", v),
            GtpError::InvalidColor(c) => write!(f, "invalid GTP color: {}", c),
            GtpError::InvalidMove(m) => write!(f, "invalid GTP move: {}", m),
            GtpError::InvalidSetup(err) => write!(f, "invalid GTP setup position: {}", err),
            GtpError::ProcessNotRunning(report) => {
                let exit_status = report.exit_status.as_deref().unwrap_or("unknown");
                if report.stderr.is_empty() {
                    write!(
                        f,
                        "GTP engine process is not running: command `{}`, GTP `{}`, status {}, stderr <empty>",
                        report.command, report.gtp_command, exit_status
                    )
                } else {
                    write!(
                        f,
                        "GTP engine process is not running: command `{}`, GTP `{}`, status {}, stderr:\n{}",
                        report.command, report.gtp_command, exit_status, report.stderr
                    )
                }
            }
            GtpError::UnsupportedBoardSize(s) => write!(f, "unsupported board size: {}", s),
        }
    }
}

impl std::error::Error for GtpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GtpError::Io(e) => Some(e),
            GtpError::InvalidSetup(err) => Some(err),
            _ => None,
        }
    }
}
