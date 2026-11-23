//! Scan progress reporting: an indicatif bar and a verbose line printer.

use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::Mutex;
use vmate_core::country::CountryCode;
use vmate_core::scan::ScanProgress;

/// Progress bar shown for a normal (non-verbose) scan.
pub struct ProgressReporter {
    bar: ProgressBar,
    state: Mutex<(usize, usize, usize)>, // (tested, ok, matched)
}

impl ProgressReporter {
    pub fn new(filter: String) -> Self {
        let bar = ProgressBar::new(0);
        let style = ProgressStyle::with_template("{prefix:>14} [{bar:40}] {percent:>3}% {msg}")
            .expect("valid indicatif template")
            .progress_chars("##-");
        bar.set_style(style);
        bar.set_prefix(if filter.is_empty() {
            "Scanning".to_string()
        } else {
            format!("Filter: {filter}")
        });
        Self {
            bar,
            state: Mutex::new((0, 0, 0)),
        }
    }

    fn render(&self, state: &(usize, usize, usize)) {
        self.bar.set_position(state.0 as u64);
        self.bar.set_message(format!(
            "tested {} | ok {} | matched {}",
            state.0, state.1, state.2
        ));
    }
}

impl ScanProgress for ProgressReporter {
    fn total(&self, total: usize) {
        self.bar.set_length(total as u64);
    }

    fn tested(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(e) => e.into_inner(),
        };
        state.0 += 1;
        self.render(&state);
    }

    fn ok(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(e) => e.into_inner(),
        };
        state.1 += 1;
        self.render(&state);
    }

    fn matched(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(e) => e.into_inner(),
        };
        state.2 += 1;
        self.render(&state);
    }

    fn success(&self, _path: &Path, _country: &CountryCode) {}

    fn failed(&self, _path: &Path) {}

    fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

/// Line-based reporter for verbose scans; mirrors the Go tool's output.
pub struct VerboseReporter;

impl ScanProgress for VerboseReporter {
    fn total(&self, total: usize) {
        println!("Testing {total} configs");
    }

    fn tested(&self) {}

    fn ok(&self) {}

    fn matched(&self) {}

    fn success(&self, path: &Path, country: &CountryCode) {
        println!("[SUCCESS] {} --- {}", path.display(), country);
    }

    fn failed(&self, path: &Path) {
        println!("[FAILED] {}", path.display());
    }
}
