//! Joining a table to places (WP3.11), in the workspace: when a table without coordinates is open
//! beside map layers, the best shared column is found in the background; a clear answer joins at
//! once (one undo step), an unclear one asks one question, and nothing to offer leaves the table
//! as it is.

use std::collections::HashMap;
use std::sync::Arc;

use lm_core::document::{Classes, ColorBy, Op, Style, Transaction};
use lm_core::model::{FieldRole, JoinRecord, LayerId, LayerKind};
use lm_jobs::import::Dataset;
use lm_jobs::import::{ImportedLayer, LayerData};
use lm_jobs::join::{self, Decision, JoinSpec, Pairing};

use super::{Finish, Workspace, drawable};

/// What a join did, for the card under the layer's name.
#[derive(Clone, Debug)]
pub struct JoinReport {
    pub table_name: String,
    pub layer_field: String,
    pub table_field: String,
    pub level: join::Level,
    /// Layer records that got data, of all.
    pub matched: usize,
    pub layer_rows: usize,
    pub unmatched: Vec<String>,
    pub near_misses: Vec<(String, String)>,
    pub rows_note: Option<String>,
    /// The layer was coloured by the first joined measure (the card offers to undo that).
    pub styled: Option<Style>,
    spec: JoinSpec,
    table: Arc<Dataset>,
    table_source: lm_core::model::SourceRef,
    base_rev: u32,
}

/// One answer the join question offers.
#[derive(Clone, Debug)]
pub struct JoinOffer {
    /// "district ↔ name".
    pub label: String,
    /// "15 of 17 match".
    pub detail: String,
    pub target: LayerId,
    spec: JoinSpec,
}

/// "Which places does population.csv describe?"
#[derive(Clone, Debug)]
pub struct JoinQuestion {
    pub table_name: String,
    pub offers: Vec<JoinOffer>,
}

#[derive(Default)]
pub struct JoinState {
    pub reports: HashMap<LayerId, JoinReport>,
    pub questions: HashMap<LayerId, JoinQuestion>,
    /// What each table was last tried against (the map layers and their data), so a table is
    /// tried again only when that changes.
    tried: HashMap<LayerId, String>,
    /// A table whose join is being worked out.
    working: HashMap<LayerId, String>,
}

fn table_name(w: &Workspace, id: LayerId) -> String {
    w.doc.layer(id).map(|l| l.profile.source.file_name().to_string()).unwrap_or_default()
}

fn vector(w: &Workspace, id: LayerId) -> Option<Arc<Dataset>> {
    let l = w.doc.layer(id)?;
    match &w.data.get(&(id, l.profile.data_rev))?.data {
        LayerData::Vector(ds) => Some(ds.clone()),
        LayerData::Raster(_) => None,
    }
}

impl Workspace {
    /// Tries to join each open table to the map layers, in the background, when the table or the
    /// layers changed since it was last tried.
    pub(super) fn try_joins(&mut self) {
        let targets: Vec<(LayerId, u32)> = self
            .doc
            .layers
            .iter()
            .rev()
            .filter(|l| drawable(&l.profile) && matches!(l.profile.kind, LayerKind::Areas | LayerKind::Points | LayerKind::Lines))
            .map(|l| (l.id, l.profile.data_rev))
            .collect();
        let tables: Vec<(LayerId, u32)> =
            self.doc.layers.iter().filter(|l| l.profile.kind == LayerKind::Table).map(|l| (l.id, l.profile.data_rev)).collect();
        // `tried` is never pruned: after Undo brings a joined table back, it isn't joined again
        // on its own (the layers are as they were, so the key matches).
        self.joins.questions.retain(|t, _| tables.iter().any(|(id, _)| id == t));
        if targets.is_empty() {
            return;
        }
        for (table, rev) in tables {
            let key = format!("{rev}|{targets:?}");
            if self.joins.tried.get(&table) == Some(&key) || self.joins.working.get(&table) == Some(&key) {
                continue;
            }
            let Some(tds) = vector(self, table) else { continue };
            let sets: Vec<(LayerId, Arc<Dataset>)> = targets.iter().filter_map(|(id, _)| vector(self, *id).map(|d| (*id, d))).collect();
            if sets.len() != targets.len() {
                continue;
            }
            let name = table_name(self, table);
            self.joins.working.insert(table, key.clone());
            let fail_key = key.clone();
            self.spawn_job(
                None,
                Box::new(move |w: &mut Workspace| {
                    w.joins.working.remove(&table);
                    w.joins.tried.insert(table, fail_key);
                }),
                move || {
                    // The best decision over every map layer: a join beats a question; within
                    // each, the higher match rate, then the layer higher on the shelf.
                    let mut best: Option<(LayerId, Decision, f64)> = None;
                    for (id, lds) in &sets {
                        let pairings = join::score(&tds, lds);
                        let d = join::decide(&pairings);
                        let rate = pairings.first().map_or(0.0, Pairing::rate);
                        let rank = |d: &Decision| match d {
                            Decision::Join(_) => 2,
                            Decision::Ask(_) => 1,
                            Decision::Nothing => 0,
                        };
                        let better = best.as_ref().is_none_or(|(_, bd, br)| (rank(&d), rate) > (rank(bd), *br));
                        if better {
                            best = Some((*id, d, rate));
                        }
                    }
                    let offers: Option<(LayerId, Decision, Vec<(String, String, JoinSpec)>)> = best.and_then(|(id, d, _)| {
                        let lds = &sets.iter().find(|(i, _)| *i == id)?.1;
                        let specs = match &d {
                            Decision::Join(p) => vec![(String::new(), String::new(), join::spec_for(&tds, lds, p, &name))],
                            Decision::Ask(ps) => ps
                                .iter()
                                .map(|p| {
                                    (
                                        format!("{} ↔ {}", tds.fields[p.table_field].name, lds.fields[p.layer_field].name),
                                        format!("{} of {} match", p.matched_keys, p.table_keys),
                                        join::spec_for(&tds, lds, p, &name),
                                    )
                                })
                                .collect(),
                            Decision::Nothing => Vec::new(),
                        };
                        Some((id, d, specs))
                    });
                    Box::new(move |w: &mut Workspace| {
                        if w.joins.working.get(&table) != Some(&key) {
                            return;
                        }
                        w.joins.working.remove(&table);
                        w.joins.tried.insert(table, key);
                        match offers {
                            Some((target, Decision::Join(_), mut specs)) => {
                                if let Some((_, _, spec)) = specs.pop() {
                                    w.perform_join(table, target, spec, true);
                                }
                            }
                            Some((target, Decision::Ask(_), specs)) => {
                                let name = table_name(w, table);
                                let offers =
                                    specs.into_iter().map(|(label, detail, spec)| JoinOffer { label, detail, target, spec }).collect();
                                w.joins.questions.insert(table, JoinQuestion { table_name: name, offers });
                                w.select_layer(table);
                            }
                            _ => {}
                        }
                    }) as Finish
                },
            );
        }
    }

    /// Joins `table` to `target` as `spec` says: the target gains the table's fields in a new
    /// data revision, the table leaves the shelf, and (when `style` and the layer has one
    /// colour) the layer is coloured by the first joined measure. One undo step.
    pub(super) fn perform_join(&mut self, table: LayerId, target: LayerId, spec: JoinSpec, style: bool) -> bool {
        let (Some(tds), Some(lds)) = (vector(self, table), vector(self, target)) else { return false };
        let Some(tl) = self.doc.layer(table).cloned() else { return false };
        let Some(ll) = self.doc.layer(target).cloned() else { return false };
        let Some(joined) = join::apply(&tds, &lds, &spec) else { return false };
        let base_rev = ll.profile.data_rev;
        self.next_rev = self.next_rev.max(base_rev) + 1;
        let rev = self.next_rev;
        let mut profile = ll.profile.clone();
        profile.data_rev = rev;
        let table_fields = added_profiles(&tl, &spec, &joined.new_fields);
        profile.fields.extend(table_fields.iter().cloned());
        profile.joins.push(JoinRecord {
            table: spec.table_name.clone(),
            source: tl.profile.source.clone(),
            fields: joined.new_fields.clone(),
            spec: spec.to_json(),
        });
        let data = ImportedLayer {
            name: ll.display_name.clone(),
            profile: profile.clone(),
            data: LayerData::Vector(Arc::new(joined.data.clone())),
            status: lm_core::model::ImportStatus::Complete,
            raw: None,
        };
        self.data.insert((target, rev), Arc::new(data));
        let mut ops: Vec<Op> = Vec::new();
        if let Some(tx) = self.doc.set_profile_tx(target, profile) {
            ops.extend(tx.ops);
        }
        // Colour by the first joined measure, when the layer has one colour.
        let mut styled = None;
        if style
            && ll.style.color_by.is_none()
            && let Some(f) = table_fields.iter().find(|f| matches!(f.role.value, FieldRole::Measure(_)))
        {
            let mut s = ll.style.clone();
            let pal = if lm_core::palette::is_temperature(f.unit.value.as_deref()) { "heat" } else { "viridis" };
            s.color_by = Some(ColorBy {
                field: f.name.clone(),
                categories: false,
                classes: Classes::Groups { count: 5, equal_steps: false },
                palette: pal.into(),
                per_km2: false,
                fit_to_view: false,
            });
            if let Some(tx) = self.doc.set_style_tx(target, s) {
                ops.extend(tx.ops);
                styled = Some(ll.style.clone());
            }
        }
        if let Some(tx) = self.doc.remove_layer_tx(table) {
            ops.extend(tx.ops);
        }
        if !self.commit(Some(Transaction::new(format!("Join {}", spec.table_name), ops))) {
            return false;
        }
        self.joins.questions.remove(&table);
        self.joins.reports.insert(
            target,
            JoinReport {
                table_name: spec.table_name.clone(),
                layer_field: spec.layer_field.clone(),
                table_field: spec.table_field.clone(),
                level: spec.level,
                matched: joined.layer_matched,
                layer_rows: lds.features.len(),
                unmatched: joined.unmatched.clone(),
                near_misses: joined.near_misses.clone(),
                rows_note: joined.rows_note.clone(),
                styled,
                spec,
                table: tds,
                table_source: tl.profile.source.clone(),
                base_rev,
            },
        );
        self.select_layer(target);
        true
    }

    /// The join question's answer: an offer, or None to keep the table as a table.
    pub fn answer_join(&mut self, table: LayerId, offer: Option<usize>) {
        let Some(q) = self.joins.questions.remove(&table) else { return };
        if let Some(o) = offer.and_then(|i| q.offers.get(i).cloned()) {
            self.perform_join(table, o.target, o.spec, true);
        }
    }

    /// Accepts the near-miss suggestions on a joined layer: the join is made again from the
    /// layer's data before it, with those keys matched. One undo step.
    pub fn accept_near_misses(&mut self, target: LayerId) {
        let Some(r) = self.joins.reports.get(&target).cloned() else { return };
        let Some(base) = self.data.get(&(target, r.base_rev)).cloned() else { return };
        let LayerData::Vector(lds) = &base.data else { return };
        let mut spec = r.spec.clone();
        spec.aliases.extend(r.near_misses.iter().cloned());
        let Some(joined) = join::apply(&r.table, lds, &spec) else { return };
        let Some(ll) = self.doc.layer(target).cloned() else { return };
        self.next_rev = self.next_rev.max(ll.profile.data_rev) + 1;
        let rev = self.next_rev;
        let mut profile = ll.profile.clone();
        profile.data_rev = rev;
        if let Some(j) = profile.joins.last_mut() {
            j.spec = spec.to_json();
        }
        self.data.insert(
            (target, rev),
            Arc::new(ImportedLayer {
                name: ll.display_name.clone(),
                profile: profile.clone(),
                data: LayerData::Vector(Arc::new(joined.data)),
                status: lm_core::model::ImportStatus::Complete,
                raw: None,
            }),
        );
        let tx = self.doc.set_profile_tx(target, profile).map(|mut t| {
            t.label = "Use the suggested matches".into();
            t
        });
        if self.commit(tx)
            && let Some(rep) = self.joins.reports.get_mut(&target)
        {
            rep.matched = joined.layer_matched;
            rep.unmatched = joined.unmatched;
            rep.near_misses = Vec::new();
            rep.spec = spec;
        }
    }

    /// Puts the layer's colours back as they were before the join.
    pub fn keep_original_colors(&mut self, target: LayerId) {
        let Some(s) = self.joins.reports.get_mut(&target).and_then(|r| r.styled.take()) else { return };
        let tx = self.doc.set_style_tx(target, s);
        self.commit(tx);
    }

    /// The card for a joined layer, while the join is still the layer's latest.
    pub fn join_report(&self, target: LayerId) -> Option<&JoinReport> {
        let r = self.joins.reports.get(&target)?;
        let l = self.doc.layer(target)?;
        l.profile.joins.last().is_some_and(|j| j.table == r.table_name).then_some(r)
    }

    pub fn dismiss_join_report(&mut self, target: LayerId) {
        self.joins.reports.remove(&target);
    }

    pub fn join_question(&self, table: LayerId) -> Option<&JoinQuestion> {
        self.joins.questions.get(&table)
    }

    /// Whether a table is being matched to the map layers right now.
    pub fn join_working(&self, table: LayerId) -> bool {
        self.joins.working.contains_key(&table)
    }
}

/// The table's field profiles for the fields a join added, under their joined names.
fn added_profiles(table: &lm_core::document::DocLayer, spec: &JoinSpec, names: &[String]) -> Vec<lm_core::model::FieldProfile> {
    let src: Vec<&lm_core::model::FieldProfile> = table.profile.fields.iter().filter(|f| f.name != spec.table_field).collect();
    src.iter()
        .zip(names)
        .map(|(f, n)| {
            let mut p = (*f).clone();
            if let Some(r) = table.roles.get(&f.name) {
                p.role = lm_core::model::Known::certain(*r, "Set by you.");
            }
            p.name = n.clone();
            p
        })
        .collect()
}

impl JoinReport {
    /// "Joined population.csv to districts: 16 of 17 districts matched."
    pub fn line(&self, layer_name: &str, nouns: (&str, &str)) -> String {
        let noun = if self.layer_rows == 1 { nouns.0 } else { nouns.1 };
        format!(
            "Joined {} to {layer_name}: {} of {} {noun} matched.",
            self.table_name,
            lm_core::model::group_thousands(self.matched as u64),
            lm_core::model::group_thousands(self.layer_rows as u64)
        )
    }

    /// "Country Name ↔ name, through standard country names and codes."
    pub fn how(&self) -> String {
        format!("{} ↔ {}, {}.", self.table_field, self.layer_field, self.level.describe())
    }

    pub fn table_source(&self) -> &lm_core::model::SourceRef {
        &self.table_source
    }
}
