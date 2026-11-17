//! GPU-Test: two fixed spots solved by postflop-solver on all cores and by the GPU prototype,
//! with times and how far the results differ. Locked while the PokerStars client runs, like
//! the solver; a running test stops as soon as the client starts.

use std::sync::atomic::Ordering;

use ringside_gpu::{Report, benchmark};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{CmdResult, blocking};

#[derive(Serialize)]
pub struct GpuBenchmark {
    #[serde(flatten)]
    report: Report,
    /// The summary for the clipboard.
    text: String,
}

/// Progress arrives as `gpu-benchmark-progress` events with a short German status line.
#[tauri::command]
pub async fn gpu_benchmark(app: AppHandle) -> CmdResult<GpuBenchmark> {
    let events = app.clone();
    blocking(app, move |state| {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let mut progress = |line: &str| {
            let _ = events.emit("gpu-benchmark-progress", line);
        };
        let stop = || state.client_running.load(Ordering::Relaxed);
        match benchmark(threads, &mut progress, &stop) {
            Ok(report) => Ok(GpuBenchmark { text: report.text(), report }),
            Err(_) if stop() => Err("gesperrt".into()),
            Err(e) => Err(e),
        }
    })
    .await
}
