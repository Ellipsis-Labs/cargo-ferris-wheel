use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use console::{Term, style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::constants::progress::{SPINNER_FRAMES, TICK_INTERVAL};

// Progress bar style templates as constants
const PROGRESS_BAR_TEMPLATE: &str =
    "{msg} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {per_sec}";
const SPINNER_TEMPLATE: &str = "{spinner:.cyan} {msg}";

pub struct ProgressReporter {
    term: Term,
    spinner_position: AtomicUsize,
    multi_progress: MultiProgress,
    current_bar: Option<ProgressBar>,
}

impl Default for ProgressReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressReporter {
    pub fn new() -> Self {
        let term = Term::stderr();
        Self {
            term,
            spinner_position: AtomicUsize::new(0),
            multi_progress: MultiProgress::new(),
            current_bar: None,
        }
    }

    pub fn create_progress_bar(&mut self, len: u64, message: &str) -> ProgressBar {
        let pb = self.multi_progress.add(ProgressBar::new(len));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(PROGRESS_BAR_TEMPLATE)
                .expect("Progress bar template should be valid")
                .progress_chars("█▉▊▋▌▍▎▏ "),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(TICK_INTERVAL);
        pb
    }

    pub fn create_spinner(&mut self, message: &str) -> ProgressBar {
        let pb = self.multi_progress.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template(SPINNER_TEMPLATE)
                .expect("Spinner template should be valid")
                .tick_strings(&["🎡 ", "🎡⊙", "🎡◐", "🎡◓", "🎡◑", "🎡◒", "🎡○", "🎡●", "✓"]),
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(TICK_INTERVAL);
        pb
    }

    fn get_ferris_wheel_frame(&self) -> &'static str {
        let pos = self.spinner_position.fetch_add(1, Ordering::Relaxed) % SPINNER_FRAMES.len();
        SPINNER_FRAMES[pos]
    }

    pub fn start_discovery(&mut self) {
        let _ = self.term.clear_line();
        eprintln!("{} Discovering Rust workspaces...", style("🔍").cyan());
        let spinner = self.create_spinner("Scanning for Cargo.lock files...");
        self.current_bar = Some(spinner);
    }

    pub fn checking_manifest(&self, path: &Path) {
        if let Some(ref pb) = self.current_bar {
            pb.set_message(format!("Checking: {}...", path.display()));
        } else {
            let _ = self.term.clear_line();
            eprint!(
                "\r{} Checking: {}... ",
                style(self.get_ferris_wheel_frame()).cyan(),
                style(path.display()).dim()
            );
        }
    }

    pub fn analyzing_workspace(&self, name: &str) {
        let _ = self.term.clear_line();
        eprint!(
            "\r{} Analyzing workspace: {}... ",
            style(self.get_ferris_wheel_frame()).yellow(),
            style(name).green()
        );
    }

    pub fn finish_discovery(&mut self, count: usize) {
        if let Some(pb) = self.current_bar.take() {
            pb.finish_and_clear();
        }
        let _ = self.term.clear_line();
        if count == 0 {
            eprintln!("\r{} No workspaces found", style("✗").red());
        } else {
            eprintln!(
                "\r{} Discovery complete: found {} workspace{}",
                style("✓").green(),
                style(count).yellow().bold(),
                if count == 1 { "" } else { "s" }
            );
        }
    }

    pub fn start_cycle_detection(&mut self) {
        eprintln!("\n{} Detecting dependency cycles...", style("🔄").yellow());
    }

    pub fn start_graph_building(&mut self, total_workspaces: usize) -> ProgressBar {
        let pb = self.create_progress_bar(total_workspaces as u64, "Building dependency graph");
        self.current_bar = Some(pb.clone());
        pb
    }

    pub fn update_graph_progress(&self, workspace_name: &str) {
        if let Some(ref pb) = self.current_bar {
            pb.set_message(format!("Processing workspace: {workspace_name}"));
            pb.inc(1);
        }
    }

    pub fn finish_graph_building(&mut self) {
        if let Some(pb) = self.current_bar.take() {
            pb.finish_with_message("Graph building complete");
        }
    }

    pub fn finish_cycle_detection(&self, cycles_found: usize) {
        if cycles_found == 0 {
            eprintln!(
                "{} No cycles detected! {}",
                style("✓").green().bold(),
                style("🎉").dim()
            );
        } else {
            eprintln!(
                "{} Found {} cycle{}",
                style("⚠").yellow().bold(),
                style(cycles_found).red().bold(),
                if cycles_found == 1 { "" } else { "s" }
            );
        }
    }
}
