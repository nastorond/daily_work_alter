//! The OS-facing surface: the window, the tray icon, and the Windows
//! integrations (global shortcut, login autostart, toast notifications).

pub mod autostart;
pub mod notify;
pub mod shortcut;
pub mod tray;
pub mod window;
