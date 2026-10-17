//! Global shortcut sessions owned by the desktop portal, independent of Tauri.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ashpd::desktop::{
    global_shortcuts::{GlobalShortcuts, NewShortcut},
    Session,
};
use futures_util::StreamExt;
use serde::Serialize;
use tokio::{sync::Mutex as AsyncMutex, task::JoinHandle};

const ACTION: &str = "show-panel";

/// The binding actually granted by the desktop, rather than the requested keys.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    /// Whether the portal has granted a live binding.
    pub registered: bool,
    /// The desktop's human-readable shortcut description.
    pub trigger: Option<String>,
    /// The latest registration or session failure.
    pub error: Option<String>,
}

/// Information needed to focus a window after a Wayland shortcut activation.
pub struct Activation {
    /// A compositor-issued token permitting activation of the window.
    pub token: Option<String>,
    /// The compositor event timestamp, in milliseconds.
    pub timestamp: u32,
}

struct Binding {
    portal: GlobalShortcuts,
    session: Arc<Session<GlobalShortcuts>>,
    worker: JoinHandle<()>,
}

/// Owns one shortcut session and serializes registration and configuration.
#[derive(Default)]
pub struct PortalHotkey {
    binding: AsyncMutex<Option<Binding>>,
    status: Arc<Mutex<Status>>,
}

fn read_status(status: &Mutex<Status>) -> Status {
    status.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

fn write_status(status: &Mutex<Status>, value: Status) {
    *status.lock().unwrap_or_else(|p| p.into_inner()) = value;
}

fn desktop_exec(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or("The application path is not valid UTF-8")?;
    if path.contains(['\n', '\r']) {
        return Err("The application path contains a newline".into());
    }
    // Desktop Exec is not a shell command: quote its reserved characters and
    // escape percent field codes, then escape backslashes for the key file.
    let mut quoted = String::from("\"");
    for ch in path.chars() {
        match ch {
            '\\' | '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    Ok(quoted.replace('\\', "\\\\"))
}

fn ensure_desktop_entry(app_id: &str, description: &str) -> Result<(), String> {
    // RPM/DEB provide this file. Portable AppImages and development builds need
    // an app-owned identity entry too; never replace an existing desktop entry.
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
        .ok_or("Could not locate the desktop applications directory")?;
    let data_dirs =
        std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let filename = format!("{app_id}.desktop");
    if std::iter::once(data_home.clone())
        .chain(std::env::split_paths(&data_dirs))
        .any(|dir| dir.join("applications").join(&filename).is_file())
    {
        return Ok(());
    }
    let executable = match std::env::var_os("APPIMAGE") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        _ => std::env::current_exe().map_err(|e| e.to_string())?,
    };
    let exec = desktop_exec(&executable)?;
    let directory = data_home.join("applications");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join(filename);
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let description = description.replace(['\n', '\r'], " ");
    let entry = format!("[Desktop Entry]\nType=Application\nName={description}\nExec={exec}\nTerminal=false\nNoDisplay=true\n");
    if let Err(error) = file.write_all(entry.as_bytes()) {
        let _ = std::fs::remove_file(path);
        return Err(error.to_string());
    }
    Ok(())
}

/// Detect native Wayland sessions even when XWayland also provides DISPLAY.
pub fn is_wayland_session(session_type: Option<&str>, wayland_display: Option<&str>) -> bool {
    session_type == Some("wayland") || wayland_display.is_some_and(|value| !value.is_empty())
}

/// Convert saved Tauri accelerators to the XDG shortcut syntax.
pub fn preferred_trigger(shortcut: &str) -> Result<String, String> {
    let parts: Vec<_> = shortcut.split('+').map(str::trim).collect();
    let (key, modifiers) = parts.split_last().ok_or("Empty shortcut")?;
    let mut result = Vec::new();
    for modifier in modifiers {
        let name = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => "CTRL",
            "alt" | "option" => "ALT",
            "shift" => "SHIFT",
            "super" | "meta" | "cmd" | "command" => "LOGO",
            _ => return Err(format!("Unsupported shortcut modifier: {modifier}")),
        };
        if !result.iter().any(|part| part == name) {
            result.push(name.to_string());
        }
    }
    let lower = key.to_ascii_lowercase();
    let keysym = match lower.as_str() {
        "space" => "space",
        "enter" | "return" => "Return",
        "escape" | "esc" => "Escape",
        "tab" => "Tab",
        "backspace" => "BackSpace",
        "delete" => "Delete",
        "insert" => "Insert",
        "up" | "arrowup" => "Up",
        "down" | "arrowdown" => "Down",
        "left" | "arrowleft" => "Left",
        "right" | "arrowright" => "Right",
        "home" => "Home",
        "end" => "End",
        "pageup" => "Prior",
        "pagedown" => "Next",
        "bracketleft" | "[" => "bracketleft",
        "bracketright" | "]" => "bracketright",
        "backslash" | "\\" => "backslash",
        "slash" | "/" => "slash",
        "semicolon" | ";" => "semicolon",
        "quote" | "'" => "apostrophe",
        "backquote" | "`" => "grave",
        "comma" | "," => "comma",
        "period" | "." => "period",
        "minus" | "-" => "minus",
        "equal" | "=" => "equal",
        "numpadadd" => "KP_Add",
        "numpadsubtract" => "KP_Subtract",
        "numpadmultiply" => "KP_Multiply",
        "numpaddivide" => "KP_Divide",
        "numpaddecimal" => "KP_Decimal",
        "numpadenter" => "KP_Enter",
        _ if lower.len() == 1 && lower.as_bytes()[0].is_ascii_alphanumeric() => &lower,
        _ if lower
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n)) =>
        {
            result.push(lower.to_ascii_uppercase());
            return Ok(result.join("+"));
        }
        _ if lower
            .strip_prefix("numpad")
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit()) =>
        {
            result.push(format!("KP_{}", &lower[6..]));
            return Ok(result.join("+"));
        }
        _ => return Err(format!("Unsupported shortcut key: {key}")),
    };
    result.push(keysym.to_string());
    Ok(result.join("+"))
}

impl PortalHotkey {
    /// Return the current registration state without waiting for a consent dialog.
    pub fn status(&self) -> Status {
        read_status(&self.status)
    }

    /// Bind a shortcut and retain its session and activation subscription.
    pub async fn register(
        &self,
        app_id: &str,
        description: &str,
        shortcut: &str,
        activate: impl Fn(Activation) + Send + Sync + 'static,
    ) -> Result<(), String> {
        let mut binding = self.binding.lock().await;
        if binding.as_ref().is_some_and(|b| !b.worker.is_finished()) {
            return Ok(());
        }
        if let Some(old) = binding.take() {
            old.worker.abort();
            let _ = old.session.close().await;
        }
        write_status(&self.status, Status::default());
        let result = self.bind(app_id, description, shortcut, activate).await;
        match result {
            Ok(value) => {
                *binding = Some(value);
                Ok(())
            }
            Err(error) => {
                let message = format!("Could not enable the Wayland shortcut: {error}. Use the tray icon to open the app.");
                write_status(
                    &self.status,
                    Status {
                        error: Some(message.clone()),
                        ..Status::default()
                    },
                );
                Err(message)
            }
        }
    }

    async fn bind(
        &self,
        app_id: &str,
        description: &str,
        shortcut: &str,
        activate: impl Fn(Activation) + Send + Sync + 'static,
    ) -> Result<Binding, String> {
        let trigger = preferred_trigger(shortcut)?;
        let app_id: ashpd::AppID = app_id.try_into().map_err(|e: ashpd::Error| e.to_string())?;
        ensure_desktop_entry(app_id.as_ref(), description)?;
        // A dedicated connection identifies this host app and drops any pending
        // requests when registration fails or the session is replaced.
        let connection = ashpd::zbus::Connection::session()
            .await
            .map_err(|e| e.to_string())?;
        ashpd::register_host_app_with_connection(connection.clone(), app_id)
            .await
            .map_err(|e| format!("desktop application registration failed: {e}"))?;
        let portal = GlobalShortcuts::with_connection(connection)
            .await
            .map_err(|e| e.to_string())?;
        let session = Arc::new(
            portal
                .create_session(Default::default())
                .await
                .map_err(|e| e.to_string())?,
        );
        let result = async {
            let mut activated = portal.receive_activated().await.map_err(|e| e.to_string())?;
            let mut changed = portal.receive_shortcuts_changed().await.map_err(|e| e.to_string())?;
            let worker_session = Arc::clone(&session);
            let response = portal.bind_shortcuts(&session, &[NewShortcut::new(ACTION, description).preferred_trigger(trigger.as_str())], None, Default::default())
                .await.map_err(|e| e.to_string())?.response().map_err(|e| e.to_string())?;
            let granted = response.shortcuts().iter().find(|s| s.id() == ACTION)
                .ok_or("The desktop did not grant the shortcut")?;
            write_status(&self.status, Status { registered: true, trigger: Some(granted.trigger_description().to_string()), error: None });
            let status = Arc::clone(&self.status);
            // Streams are scoped to this dedicated connection's single session.
            let worker = tokio::spawn(async move {
                let mut closed = match worker_session.receive_closed().await {
                    Ok(stream) => stream,
                    Err(error) => {
                        write_status(&status, Status { error: Some(error.to_string()), ..Status::default() });
                        let _ = worker_session.close().await;
                        return;
                    }
                };
                loop {
                    tokio::select! {
                        event = activated.next() => match event {
                            Some(event) if event.shortcut_id() == ACTION => {
                                let token = event.options().get("activation_token")
                                    .and_then(|value| <&str>::try_from(value).ok()).map(str::to_owned);
                                activate(Activation { token, timestamp: event.timestamp().as_millis() as u32 });
                            }
                            Some(_) => {}
                            None => break,
                        },
                        event = changed.next() => match event {
                            Some(event) => {
                                let shortcut = event.shortcuts().iter().find(|s| s.id() == ACTION);
                                write_status(&status, Status {
                                    registered: shortcut.is_some(),
                                    trigger: shortcut.map(|s| s.trigger_description().to_string()),
                                    error: None,
                                });
                            }
                            None => break,
                        },
                        _ = closed.next() => break,
                    }
                }
                write_status(&status, Status { error: Some("The desktop shortcut session closed. Use the tray icon to open the app.".into()), ..Status::default() });
                let _ = worker_session.close().await;
            });
            Ok(worker)
        }.await;
        match result {
            Ok(worker) => Ok(Binding {
                portal,
                session,
                worker,
            }),
            Err(error) => {
                let _ = session.close().await;
                Err(error)
            }
        }
    }

    /// Open the desktop's shortcut editor without replacing its saved binding.
    pub async fn configure(&self) -> Result<(), String> {
        let binding = self.binding.lock().await;
        let binding = binding
            .as_ref()
            .filter(|b| !b.worker.is_finished())
            .ok_or("The desktop shortcut session is not active")?;
        binding
            .portal
            .configure_shortcuts(&binding.session, None, Default::default())
            .await
            .map_err(|e| format!("Could not open desktop shortcut settings: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_command_quotes_paths_without_expanding_field_codes() {
        assert_eq!(
            desktop_exec(Path::new("/tmp/Synapt App.AppImage")).unwrap(),
            "\"/tmp/Synapt App.AppImage\""
        );
        assert_eq!(
            desktop_exec(Path::new("/tmp/100%Synapt")).unwrap(),
            "\"/tmp/100%%Synapt\""
        );
        assert_eq!(
            desktop_exec(Path::new("/tmp/$Synapt")).unwrap(),
            "\"/tmp/\\\\$Synapt\""
        );
        assert!(desktop_exec(Path::new("/tmp/Synapt\nExec=other")).is_err());
    }

    #[tokio::test]
    async fn failed_registration_never_reports_a_live_binding() {
        let hotkey = PortalHotkey::default();
        let error = hotkey
            .register("io.aatish.synapt", "Synapt", "Unknown+V", |_| {
                panic!("must not activate")
            })
            .await;
        assert!(error.is_err());
        assert!(!hotkey.status().registered);
        assert!(hotkey.status().trigger.is_none());
        assert!(hotkey.status().error.is_some());
        assert!(hotkey.configure().await.is_err());
        // A failed attempt must release the registration lock for a retry.
        assert!(hotkey
            .register("io.aatish.synapt", "Synapt", "", |_| {})
            .await
            .is_err());
    }

    #[test]
    fn wayland_wins_when_xwayland_is_also_available() {
        assert!(is_wayland_session(Some("wayland"), Some("wayland-0")));
        assert!(is_wayland_session(None, Some("wayland-0")));
        assert!(!is_wayland_session(Some("x11"), None));
        assert!(!is_wayland_session(None, Some("")));
    }

    #[test]
    fn saved_accelerators_use_xdg_modifiers_and_keysyms() {
        assert_eq!(preferred_trigger("ctrl+space").unwrap(), "CTRL+space");
        assert_eq!(preferred_trigger("Super+Shift+V").unwrap(), "LOGO+SHIFT+v");
        assert_eq!(preferred_trigger("Ctrl+ArrowUp").unwrap(), "CTRL+Up");
        assert_eq!(preferred_trigger("ctrl+[").unwrap(), "CTRL+bracketleft");
        assert_eq!(preferred_trigger("ctrl+;").unwrap(), "CTRL+semicolon");
        assert_eq!(preferred_trigger("Alt+F12").unwrap(), "ALT+F12");
        assert_eq!(preferred_trigger("Ctrl+Numpad0").unwrap(), "CTRL+KP_0");
        assert_eq!(
            preferred_trigger("Super+BracketLeft").unwrap(),
            "LOGO+bracketleft"
        );
        assert!(preferred_trigger("").is_err());
        assert!(preferred_trigger("Unknown+V").is_err());
        assert!(preferred_trigger("Ctrl+F25").is_err());
    }
}
