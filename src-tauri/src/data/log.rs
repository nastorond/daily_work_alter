//! A tiny append-only diagnostic log at `%APPDATA%\DailyWorkAlter\log.txt`.
//!
//! This app has one job and it happens once a day while nobody is watching, so
//! "it didn't pop and I don't know why" is the failure that matters. stderr is
//! useless here — release builds run with `windows_subsystem = "windows"` and no
//! console — so the few decisions worth reconstructing go to a file instead.
//!
//! Deliberately not a general logging framework: no levels, no per-tick spam.
//! Only the notify decision, window create/close outcomes, and config reloads.

use chrono::Local;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};

/// Trim the file once it passes this, keeping the newer half.
const MAX_BYTES: u64 = 64 * 1024;

pub fn line(msg: &str) {
    let path = super::config::app_data_dir().join("log.txt");
    let _ = super::config::ensure_dirs();

    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_BYTES {
            trim(&path);
        }
    }

    let stamped = format!("{} {}\n", Local::now().format("%Y-%m-%d %H:%M:%S"), msg);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(stamped.as_bytes());
    }
}

/// Keeps the second half of the file, starting at the next line break so the
/// first surviving entry isn't a fragment.
fn trim(path: &std::path::Path) {
    let Ok(mut f) = OpenOptions::new().read(true).write(true).open(path) else {
        return;
    };
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return;
    }
    let cut = buf.len() / 2;
    let start = buf[cut..]
        .iter()
        .position(|b| *b == b'\n')
        .map(|i| cut + i + 1)
        .unwrap_or(cut);
    let tail = buf[start..].to_vec();
    if f.set_len(0).is_err() {
        return;
    }
    let _ = f.seek(SeekFrom::Start(0));
    let _ = f.write_all(&tail);
}
