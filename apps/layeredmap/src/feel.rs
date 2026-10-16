//! The feel harness (WP1.6): `layeredmap --feel OUT_DIR [--runs N] [--fixture DIR]` runs the
//! camera scripts over Shanghai and writes every frame's interval, then p50/p95/p99 and frames over
//! budget per script, plus counters that don't depend on timing (renders, frames). A warm-up pass
//! fetches the tiles first. Frames are measured with vsync off: the real cost, not the display's
//! cadence. `cargo xtask feel` builds and runs it.
//!
//! `layeredmap --launch-probe` prints the time from process start to the first map frame and quits
//! (Gate 1: cold launch to a usable map ≤ 2 s, 95th percentile over 10 launches).

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use lm_ui::Shell;
use lm_ui::mapview::script::{ALL, Script};

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Settle,
    Run,
    After,
}

struct Rec {
    script: Script,
    run: usize,
    after: bool,
    dt_ms: f64,
}

pub struct Feel {
    plan: Vec<(Script, usize)>,
    idx: usize,
    phase: Phase,
    phase_start: f64,
    stable_since: Option<f64>,
    last: Option<Instant>,
    home: lm_ui::mapview::map::Camera,
    records: Vec<Rec>,
    renders_at_start: u64,
    counters: Vec<(Script, usize, u64, usize)>,
    out: PathBuf,
}

impl Feel {
    pub fn new(out: PathBuf, runs: usize, home: lm_ui::mapview::map::Camera) -> Feel {
        let mut plan: Vec<(Script, usize)> = ALL.iter().map(|s| (*s, 0)).collect();
        for run in 1..=runs.max(1) {
            plan.extend(ALL.iter().map(|s| (*s, run)));
        }
        Feel {
            plan,
            idx: 0,
            phase: Phase::Settle,
            phase_start: 0.0,
            stable_since: None,
            last: None,
            home,
            records: Vec::new(),
            renders_at_start: 0,
            counters: Vec::new(),
            out,
        }
    }

    /// Drives the camera for this frame. Returns true when the harness has finished.
    pub fn step(&mut self, shell: &mut Shell, t: f64) -> bool {
        let now = Instant::now();
        let dt_ms = self.last.map_or(0.0, |l| (now - l).as_secs_f64() * 1000.0);
        self.last = Some(now);
        let Some(&(script, run)) = self.plan.get(self.idx) else { return true };
        let elapsed = t - self.phase_start;
        match self.phase {
            Phase::Settle => {
                shell.map_view_mut().set_camera(script.start(self.home));
                let quiet = !shell.map_view().tiles_pending();
                if quiet {
                    let since = *self.stable_since.get_or_insert(t);
                    if t - since > 0.6 || elapsed > 20.0 {
                        self.begin(Phase::Run, t, shell);
                    }
                } else {
                    self.stable_since = None;
                    if elapsed > 20.0 {
                        self.begin(Phase::Run, t, shell);
                    }
                }
            }
            Phase::Run => {
                shell.map_view_mut().set_camera(script.camera(self.home, elapsed));
                // The first frame's interval spans the last settle frame: not counted.
                if run > 0 && elapsed > 0.0 {
                    self.records.push(Rec { script, run, after: false, dt_ms });
                }
                if elapsed >= script.duration() {
                    self.begin(Phase::After, t, shell);
                }
            }
            Phase::After => {
                if run > 0 {
                    self.records.push(Rec { script, run, after: true, dt_ms });
                }
                if elapsed >= 0.6 {
                    if run > 0 {
                        let frames = self.records.iter().filter(|r| r.script == script && r.run == run).count();
                        self.counters.push((script, run, shell.map_view().renders - self.renders_at_start, frames));
                    }
                    self.idx += 1;
                    self.begin(Phase::Settle, t, shell);
                }
            }
        }
        self.idx >= self.plan.len()
    }

    fn begin(&mut self, phase: Phase, t: f64, shell: &Shell) {
        self.phase = phase;
        self.phase_start = t;
        self.stable_since = None;
        if phase == Phase::Run {
            self.renders_at_start = shell.map_view().renders;
        }
    }

    pub fn write(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.out)?;
        let mut f = std::io::BufWriter::new(std::fs::File::create(self.out.join("frames.csv"))?);
        writeln!(f, "script,run,phase,dt_ms")?;
        for r in &self.records {
            writeln!(f, "{},{},{},{:.3}", r.script.code(), r.run, if r.after { "after" } else { "run" }, r.dt_ms)?;
        }
        let mut s = std::io::BufWriter::new(std::fs::File::create(self.out.join("summary.csv"))?);
        writeln!(s, "script,run,frames,p50_ms,p95_ms,p99_ms,max_ms,over_16_7,over_33,renders")?;
        for &(script, run, renders, _) in &self.counters {
            let mut d: Vec<f64> = self.records.iter().filter(|r| r.script == script && r.run == run).map(|r| r.dt_ms).collect();
            d.sort_by(f64::total_cmp);
            let q = |p: f64| d.get(((d.len().saturating_sub(1)) as f64 * p).round() as usize).copied().unwrap_or(0.0);
            writeln!(
                s,
                "{},{},{},{:.2},{:.2},{:.2},{:.2},{},{},{}",
                script.code(),
                run,
                d.len(),
                q(0.5),
                q(0.95),
                q(0.99),
                d.last().copied().unwrap_or(0.0),
                d.iter().filter(|x| **x > 16.7).count(),
                d.iter().filter(|x| **x > 33.0).count(),
                renders
            )?;
        }
        Ok(())
    }
}
