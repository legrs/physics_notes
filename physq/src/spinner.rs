//! Loading indicator (CLAUDE.md §11): braille throbber (~80 ms/frame),
//! whimsical gerund rotating every ~2.4 s, elapsed seconds. Used by the TUI
//! status line (render-loop driven) and by a small stderr spinner thread in
//! one-shot mode.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const FRAME_MS: u64 = 80;

/// General whimsical gerunds — deliberately NOT physics-themed (§11).
pub const VERBS: [&str; 20] = [
    "Pondering",
    "Noodling",
    "Percolating",
    "Ruminating",
    "Cogitating",
    "Musing",
    "Simmering",
    "Marinating",
    "Puzzling",
    "Mulling",
    "Conjuring",
    "Wrangling",
    "Untangling",
    "Brewing",
    "Churning",
    "Assembling",
    "Rummaging",
    "Shuffling",
    "Distilling",
    "Finagling",
];
pub const VERB_MS: u64 = 2400;

pub fn frame_at(elapsed: Duration) -> &'static str {
    FRAMES[(elapsed.as_millis() as u64 / FRAME_MS) as usize % FRAMES.len()]
}

pub fn verb_at(elapsed: Duration, salt: u64) -> &'static str {
    VERBS[((elapsed.as_millis() as u64 / VERB_MS) + salt) as usize % VERBS.len()]
}

/// Render one spinner line, e.g. `⠹ Pondering…  (2.3s)`. With a phase label
/// (slow steps: downloads, index builds) the label replaces the whimsy.
pub fn line(elapsed: Duration, label: Option<&str>, salt: u64) -> String {
    let text = match label {
        Some(l) => l.to_string(),
        None => format!("{}…", verb_at(elapsed, salt)),
    };
    format!(
        "{} {}  ({:.1}s)",
        frame_at(elapsed),
        text,
        elapsed.as_secs_f64()
    )
}

/// Background stderr spinner for one-shot mode. Silent when stderr is not a
/// terminal. The heavy work runs on the calling thread; this thread only
/// animates.
pub struct StderrSpinner {
    running: Arc<AtomicBool>,
    label: Arc<Mutex<String>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl StderrSpinner {
    pub fn start(label: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let label_arc = Arc::new(Mutex::new(label.to_string()));
        let handle = if std::io::stderr().is_terminal() {
            let running = running.clone();
            let label = label_arc.clone();
            Some(std::thread::spawn(move || {
                let started = Instant::now();
                while running.load(Ordering::Relaxed) {
                    let text = label.lock().map(|l| l.clone()).unwrap_or_default();
                    let elapsed = started.elapsed();
                    let line = line(elapsed, (!text.is_empty()).then_some(text.as_str()), 0);
                    let mut err = std::io::stderr();
                    let _ = write!(err, "\r\x1b[2K{line}");
                    let _ = err.flush();
                    std::thread::sleep(Duration::from_millis(FRAME_MS));
                }
                let mut err = std::io::stderr();
                let _ = write!(err, "\r\x1b[2K");
                let _ = err.flush();
            }))
        } else {
            None
        };
        Self {
            running,
            label: label_arc,
            handle,
        }
    }

    /// Swap to plain phase text for slow steps (`Downloading model…`), or set
    /// an empty label to fall back to the whimsical verbs.
    pub fn set_label(&self, label: &str) {
        if let Ok(mut l) = self.label.lock() {
            *l = label.to_string();
        }
    }

    pub fn finish(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for StderrSpinner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_advance_every_80ms() {
        assert_eq!(frame_at(Duration::from_millis(0)), "⠋");
        assert_eq!(frame_at(Duration::from_millis(80)), "⠙");
        assert_eq!(frame_at(Duration::from_millis(800)), "⠋"); // wraps
    }

    #[test]
    fn verbs_rotate_every_2400ms() {
        assert_eq!(verb_at(Duration::from_millis(0), 0), "Pondering");
        assert_eq!(verb_at(Duration::from_millis(2400), 0), "Noodling");
    }

    #[test]
    fn line_prefers_phase_label() {
        let l = line(Duration::from_millis(2300), Some("Downloading model…"), 0);
        assert!(l.contains("Downloading model…"));
        assert!(l.contains("(2.3s)"));
        let l = line(Duration::from_millis(0), None, 0);
        assert!(l.contains("Pondering…"));
    }
}
