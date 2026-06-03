// SPDX-License-Identifier: Apache-2.0
// Copyright 2024 [Your Name]

//! Shared control channel for a single system-tray icon across the Synapt apps.
//!
//! The first app to start binds the loopback control port and becomes the tray
//! host; later apps connect to it as clients. The host owns the one tray icon and
//! routes menu actions to whichever app should handle them, forwarding to a
//! connected client over this channel. Transport is a newline-delimited JSON
//! protocol over loopback TCP so the module stays dependency-light (no async
//! runtime) and identical on every platform.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

use serde::{Deserialize, Serialize};

/// Loopback port used to coordinate the shared tray icon.
pub const TRAY_CONTROL_PORT: u16 = 48710;

/// Which Synapt application a tray action targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    Synapt,
    SynaptClip,
}

/// A command exchanged over the tray control channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TrayCommand {
    /// Sent by a client immediately after connecting to announce its app.
    Register { app: AppKind },
    /// Show (and focus) the target app's window.
    Show { app: AppKind },
    /// Toggle the visibility of the target app's window.
    Toggle { app: AppKind },
    /// Quit all Synapt apps.
    Quit,
}

/// Try to become the tray host by binding the loopback control port. Returns the
/// listener on success, or None if a host already owns the port.
pub fn claim_host() -> Option<TcpListener> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, TRAY_CONTROL_PORT))).ok()
}

/// Connect to an existing tray host, if one is running.
pub fn connect_host() -> Option<TcpStream> {
    TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, TRAY_CONTROL_PORT))).ok()
}

/// Write a single command as a JSON line.
pub fn write_command(stream: &mut TcpStream, cmd: &TrayCommand) -> std::io::Result<()> {
    let mut line = serde_json::to_string(cmd)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()
}

/// Read JSON-line commands from a stream until it closes, invoking `handler` for
/// each decoded command. Returns when the peer disconnects.
pub fn read_commands<F: FnMut(TrayCommand)>(stream: &TcpStream, mut handler: F) {
    let clone = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    for line in BufReader::new(clone).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(cmd) = serde_json::from_str::<TrayCommand>(&line) {
            handler(cmd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_roundtrip_register() {
        let cmd = TrayCommand::Register { app: AppKind::SynaptClip };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"cmd\":\"register\""));
        assert!(json.contains("\"app\":\"synapt_clip\""));
        let back: TrayCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, TrayCommand::Register { app: AppKind::SynaptClip }));
    }

    #[test]
    fn command_roundtrip_show() {
        let cmd = TrayCommand::Show { app: AppKind::Synapt };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: TrayCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, TrayCommand::Show { app: AppKind::Synapt }));
    }

    #[test]
    fn app_kind_is_hashable_key() {
        use std::collections::HashMap;
        let mut m: HashMap<AppKind, i32> = HashMap::new();
        m.insert(AppKind::Synapt, 1);
        m.insert(AppKind::SynaptClip, 2);
        assert_eq!(m.get(&AppKind::SynaptClip), Some(&2));
    }
}
