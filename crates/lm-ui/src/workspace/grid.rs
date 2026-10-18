//! Grids with variables and time (WP6.12): the time step a layer shows, the variable and level,
//! and keeping each grid layer's cells in step with them. Slices are read off the interface
//! thread (`lm_jobs::import::netcdf::slice`); one already read is taken from the cache at once.

use std::sync::Arc;

use lm_core::document::{DocLayer, Op};
use lm_core::grid::GridProfile;
use lm_core::model::{LayerId, LayerProfile};
use lm_jobs::import::{ImportedLayer, LayerData, netcdf};

use super::Workspace;

impl Workspace {
    /// The time step a layer's cells show now (they may lag the document's step while a slice
    /// is being read).
    pub fn shown_time(&self, id: LayerId) -> usize {
        self.doc.layer(id).and_then(|l| self.grid_time.get(&(id, l.profile.data_rev)).copied()).unwrap_or(0)
    }

    /// Whether a layer's cells show the document's time step (Play waits for this).
    pub fn time_ready(&self, id: LayerId) -> bool {
        let Some(l) = self.doc.layer(id) else { return false };
        let steps = l.profile.grid.as_ref().map_or(1, GridProfile::steps);
        self.data.contains_key(&(id, l.profile.data_rev)) && self.shown_time(id) == l.time.min(steps.saturating_sub(1))
    }

    /// A document layer as its cells show it: its time step is the one its data holds, so an
    /// action and its recipe name the step that was computed on.
    pub fn as_shown(&self, l: &DocLayer) -> DocLayer {
        let mut l = l.clone();
        if l.profile.grid.is_some() {
            l.time = self.shown_time(l.id);
        }
        l
    }

    /// Shows time step `to` of a grid layer. Steps taken one after another (the slider, Play,
    /// ← →) fold into one undo step.
    pub fn set_time(&mut self, id: LayerId, to: usize) -> bool {
        let tx = self.doc.set_time_tx(id, to).map(|mut t| {
            if let Some(l) = self.doc.layer(id) {
                t.label = format!("Change time of {}", l.display_name);
            }
            t
        });
        self.commit_gesture(tx, &format!("time:{}", id.0))
    }

    /// Steps a grid layer's time by `delta`, wrapping past the ends when `wrap` (Play).
    pub fn step_time(&mut self, id: LayerId, delta: i64, wrap: bool) -> bool {
        let Some(l) = self.doc.layer(id) else { return false };
        let Some(steps) = l.profile.grid.as_ref().filter(|g| g.has_time()).map(GridProfile::steps) else { return false };
        let n = steps as i64;
        let next = l.time as i64 + delta;
        let next = if wrap { next.rem_euclid(n) } else { next.clamp(0, n - 1) };
        self.set_time(id, next as usize)
    }

    /// Reads time step `t` of a grid layer into the cache ahead of need (while playing).
    pub fn prefetch(&mut self, id: LayerId, t: usize) {
        let Some(l) = self.doc.layer(id) else { return };
        let Some(g) = l.profile.grid.clone() else { return };
        let (source, rev) = (l.profile.source.clone(), l.profile.data_rev);
        if t >= g.steps() || self.grid_reading.contains(&(id, rev, t)) || netcdf::cached_slice(&source, &g, t).is_some() {
            return;
        }
        self.grid_reading.push((id, rev, t));
        self.spawn_job(None, Box::new(move |w: &mut Workspace| w.grid_reading.retain(|r| *r != (id, rev, t))), move || {
            let _ = netcdf::slice(&source, &g, t);
            Box::new(move |w: &mut Workspace| w.grid_reading.retain(|r| *r != (id, rev, t)))
        });
    }

    /// Shows another variable or level of a grid layer: read off the interface thread, then one
    /// undo step changes the layer's variable, colour range and (when the new variable has fewer
    /// steps) its time step.
    pub fn choose_variable(&mut self, id: LayerId, variable: &str, level: usize) -> Result<(), String> {
        let Some(l) = self.doc.layer(id).cloned() else { return Err("That layer is gone.".into()) };
        let Some(g) = &l.profile.grid else { return Err("This layer has no variables to choose from.".into()) };
        if g.variable == variable && g.level == level {
            return Ok(());
        }
        let Some(v) = g.variables.iter().find(|v| v.name == variable) else {
            return Err(format!("{} has no variable {variable}.", l.display_name));
        };
        self.not_busy(id, &l.display_name)?;
        let data = self.data(id).cloned().ok_or("This layer's data isn't loaded.")?;
        let label = if g.variable == variable {
            format!("Show {} of {}", v.level_label(level), l.display_name)
        } else {
            format!("Show {} in {}", v.long_name.clone().unwrap_or_else(|| v.name.clone()), l.display_name)
        };
        let (variable, time) = (variable.to_string(), l.time);
        let wake = self.wake.clone();
        self.spawn_edit(id, label, false, wake, move || netcdf::choose_variable(&data, &variable, level, time).map_err(|e| e.to_string()));
        if let Some(p) = self.pending.last_mut() {
            p.restyle = true;
        }
        Ok(())
    }

    /// The ops that go with another variable's profile: its colour range, and the time step
    /// clamped to its steps (the new data holds that step).
    pub(super) fn grid_restyle(&mut self, id: LayerId, rev: u32, to: &LayerProfile) -> Vec<Op> {
        let Some(l) = self.doc.layer(id) else { return Vec::new() };
        let mut ops = Vec::new();
        let mut style = l.style.clone();
        style.stretch = netcdf::default_stretch(to);
        style.band = 0;
        style.above = None;
        style.like = None;
        if style != l.style {
            ops.push(Op::SetStyle { id, from: Box::new(l.style.clone()), to: Box::new(style) });
        }
        let steps = to.grid.as_ref().map_or(1, GridProfile::steps);
        let time = l.time.min(steps.saturating_sub(1));
        if time != l.time {
            ops.push(Op::SetTime { id, from: l.time, to: time });
        }
        self.grid_time.insert((id, rev), time);
        ops
    }

    /// Brings each grid layer's cells to its time step: from the cache at once, or read in the
    /// background (the old step stays on the map meanwhile).
    pub(super) fn sync_grids(&mut self) {
        let wanted: Vec<(LayerId, u32, usize, GridProfile, lm_core::model::SourceRef)> = self
            .doc
            .layers
            .iter()
            .filter_map(|l| {
                let g = l.profile.grid.as_ref()?;
                let rev = l.profile.data_rev;
                let want = l.time.min(g.steps().saturating_sub(1));
                let held = self.grid_time.get(&(l.id, rev)).copied().unwrap_or(0);
                (held != want && self.data.contains_key(&(l.id, rev))).then(|| (l.id, rev, want, g.clone(), l.profile.source.clone()))
            })
            .collect();
        for (id, rev, want, g, source) in wanted {
            if let Some(r) = netcdf::cached_slice(&source, &g, want) {
                self.put_slice(id, rev, want, r);
                continue;
            }
            if self.grid_reading.contains(&(id, rev, want)) {
                continue;
            }
            self.grid_reading.push((id, rev, want));
            self.spawn_job(None, Box::new(move |w: &mut Workspace| w.grid_reading.retain(|r| *r != (id, rev, want))), move || {
                let read = netcdf::slice(&source, &g, want);
                Box::new(move |w: &mut Workspace| {
                    w.grid_reading.retain(|r| *r != (id, rev, want));
                    match read {
                        // Put in only while the layer still wants that step (it is cached either way).
                        Ok(r) => {
                            let still = w.doc.layer(id).is_some_and(|l| l.profile.data_rev == rev && l.time == want);
                            if still {
                                w.put_slice(id, rev, want, r);
                            }
                        }
                        Err(e) => w.notice = Some(e.to_string()),
                    }
                })
            });
        }
    }

    /// Replaces a grid layer's cells with another time step's.
    fn put_slice(&mut self, id: LayerId, rev: u32, step: usize, r: Arc<lm_jobs::Raster>) {
        let Some(old) = self.data.get(&(id, rev)) else { return };
        let new = ImportedLayer { data: LayerData::Raster(r), ..(**old).clone() };
        self.data.insert((id, rev), Arc::new(new));
        self.grid_time.insert((id, rev), step);
        self.order_key.clear();
    }
}
