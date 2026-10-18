//! Joining a table to places (WP3.11), in the workspace: when a table without coordinates is open
//! beside map layers, the best shared column is found in the background; a clear answer joins at
//! once (one undo step), an unclear one asks one question, and nothing to offer leaves the table
//! as it is. A table that clearly matches several layers joins each of them, in one step. A
//! joined table stays on the shelf as a row under each layer it joined, and Unjoin puts it back.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use lm_core::document::{Classes, ColorBy, DocLayer, Op, Style, Transaction};
use lm_core::model::{FieldRole, JoinRecord, LayerId, LayerKind};
use lm_jobs::import::Dataset;
use lm_jobs::import::{ImportedLayer, LayerData};
use lm_jobs::join::{self, JoinSpec, Plan};

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
    /// Table rows that found no place (blank keys included), and those with a blank key.
    pub unmatched_rows: usize,
    pub blank_keys: usize,
    pub near_misses: Vec<(String, String)>,
    pub rows_note: Option<String>,
    /// The cardinality and row counts, worked out before joining.
    pub expected: join::Expected,
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
    /// What joining this way would give, worked out before joining.
    pub expected: Option<join::Expected>,
    pub target: LayerId,
    spec: JoinSpec,
}

/// "Which places does population.csv describe?"
#[derive(Clone, Debug)]
pub struct JoinQuestion {
    pub table_name: String,
    pub offers: Vec<JoinOffer>,
}

/// A table joined to a layer, as the session remembers it: the table as it stood on the shelf
/// (Unjoin puts it back) and how many of the layer's records got data.
#[derive(Clone)]
struct JoinedTable {
    layer: DocLayer,
    data: Arc<ImportedLayer>,
    matched: usize,
    layer_rows: usize,
}

/// A joined table's row under its layer on the shelf.
#[derive(Clone, Debug, PartialEq)]
pub struct Sublayer {
    /// The table's name ("population.csv"), as the layer's join record has it.
    pub table: String,
    /// Layer records that got data, of all, when known this session.
    pub matched: Option<(usize, usize)>,
    /// Whether Unjoin can put the table back (the session still has it).
    pub can_unjoin: bool,
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
    /// Joined tables by layer and table name. Kept after Unjoin, so Undo and Redo find them.
    joined: HashMap<(LayerId, String), JoinedTable>,
    /// Layers whose joined tables are folded away on the shelf (in memory; open by default).
    collapsed: HashSet<LayerId>,
    /// Tables the user unjoined: never joined again on their own.
    unjoined: HashSet<LayerId>,
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
            if self.joins.unjoined.contains(&table) {
                continue;
            }
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
                    // Every layer the table clearly matches joins (best first); with none, one
                    // question about the layer that matches best.
                    let layers: Vec<&Dataset> = sets.iter().map(|(_, d)| d.as_ref()).collect();
                    let outcome = match join::plan(&tds, &layers) {
                        Plan::Join(v) => {
                            Outcome::Join(v.iter().map(|(i, p)| (sets[*i].0, join::spec_for(&tds, &sets[*i].1, p, &name))).collect())
                        }
                        Plan::Ask(i, ps) => {
                            let (id, lds) = &sets[i];
                            Outcome::Ask(
                                *id,
                                ps.iter()
                                    .map(|p| {
                                        let spec = join::spec_for(&tds, lds, p, &name);
                                        (
                                            format!("{} ↔ {}", tds.fields[p.table_field].name, lds.fields[p.layer_field].name),
                                            format!("{} of {} match", p.matched_keys, p.table_keys),
                                            join::expect(&tds, lds, &spec),
                                            spec,
                                        )
                                    })
                                    .collect(),
                            )
                        }
                        Plan::Nothing => Outcome::Nothing,
                    };
                    Box::new(move |w: &mut Workspace| {
                        if w.joins.working.get(&table) != Some(&key) {
                            return;
                        }
                        w.joins.working.remove(&table);
                        w.joins.tried.insert(table, key);
                        match outcome {
                            Outcome::Join(targets) => {
                                w.perform_joins(table, targets, true);
                            }
                            Outcome::Ask(target, specs) => {
                                let name = table_name(w, table);
                                let offers = specs
                                    .into_iter()
                                    .map(|(label, detail, expected, spec)| JoinOffer { label, detail, expected, target, spec })
                                    .collect();
                                w.joins.questions.insert(table, JoinQuestion { table_name: name, offers });
                                w.select_layer(table);
                            }
                            Outcome::Nothing => {}
                        }
                    }) as Finish
                },
            );
        }
    }

    /// Joins `table` to `target` as `spec` says (the question's answer).
    pub(super) fn perform_join(&mut self, table: LayerId, target: LayerId, spec: JoinSpec, style: bool) -> bool {
        self.perform_joins(table, vec![(target, spec)], style)
    }

    /// Joins `table` to each target as its spec says: each target gains the table's fields in a
    /// new data revision, the table leaves the shelf (it shows under each target instead), and
    /// (when `style` and the first target has one colour) the first target is coloured by the
    /// first joined measure. One undo step.
    pub(super) fn perform_joins(&mut self, table: LayerId, targets: Vec<(LayerId, JoinSpec)>, style: bool) -> bool {
        let Some(tds) = vector(self, table) else { return false };
        let Some(tl) = self.doc.layer(table).cloned() else { return false };
        let Some(tdata) = self.data(table).cloned() else { return false };
        let mut ops: Vec<Op> = Vec::new();
        let mut done: Vec<(LayerId, JoinReport)> = Vec::new();
        for (target, spec) in targets {
            let (Some(lds), Some(ll)) = (vector(self, target), self.doc.layer(target).cloned()) else { continue };
            let Some(joined) = join::apply(&tds, &lds, &spec) else { continue };
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
                style: None,
            };
            self.data.insert((target, rev), Arc::new(data));
            if let Some(tx) = self.doc.set_profile_tx(target, profile) {
                ops.extend(tx.ops);
            }
            // Colour by the first joined measure, when the layer has one colour. Only the best
            // match is recoloured: the others gain the fields and keep their look.
            let mut styled = None;
            if style
                && done.is_empty()
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
            done.push((
                target,
                JoinReport {
                    table_name: spec.table_name.clone(),
                    layer_field: spec.layer_field.clone(),
                    table_field: spec.table_field.clone(),
                    level: spec.level,
                    matched: joined.layer_matched,
                    layer_rows: lds.features.len(),
                    unmatched: joined.unmatched.clone(),
                    unmatched_rows: joined.unmatched_rows.len(),
                    blank_keys: joined.blank_keys,
                    near_misses: joined.near_misses.clone(),
                    rows_note: joined.rows_note.clone(),
                    expected: joined.expected.clone(),
                    styled,
                    spec,
                    table: tds.clone(),
                    table_source: tl.profile.source.clone(),
                    base_rev,
                },
            ));
        }
        let Some(first) = done.first().map(|(id, _)| *id) else { return false };
        if let Some(tx) = self.doc.remove_layer_tx(table) {
            ops.extend(tx.ops);
        }
        let name = done[0].1.table_name.clone();
        if !self.commit(Some(Transaction::new(format!("Join {name}"), ops))) {
            return false;
        }
        self.joins.questions.remove(&table);
        for (target, r) in done {
            // Open under the layer, so the user sees where the table went.
            self.joins.collapsed.remove(&target);
            self.joins.joined.insert(
                (target, r.table_name.clone()),
                JoinedTable { layer: tl.clone(), data: tdata.clone(), matched: r.matched, layer_rows: r.layer_rows },
            );
            self.joins.reports.insert(target, r);
        }
        self.select_layer(first);
        true
    }

    /// Unjoins `table` from `target`: the layer loses the table's fields in a new data revision
    /// (and colours, filters, roles and stars that used them), and the table returns to the
    /// shelf just below the layer, unless it is still joined to another layer. One undo step.
    pub fn unjoin(&mut self, target: LayerId, table: &str) -> bool {
        let Some(ll) = self.doc.layer(target).cloned() else { return false };
        let Some(i) = ll.profile.joins.iter().position(|j| j.table == table) else { return false };
        let Some(lds) = vector(self, target) else { return false };
        let Some(saved) = self.joins.joined.get(&(target, table.to_string())).cloned() else { return false };
        let record = ll.profile.joins[i].clone();
        let gone = |f: &str| record.fields.iter().any(|x| x == f);
        self.next_rev = self.next_rev.max(ll.profile.data_rev) + 1;
        let rev = self.next_rev;
        let mut profile = ll.profile.clone();
        profile.data_rev = rev;
        profile.joins.remove(i);
        profile.fields.retain(|f| !gone(&f.name));
        self.data.insert(
            (target, rev),
            Arc::new(ImportedLayer {
                name: ll.display_name.clone(),
                profile: profile.clone(),
                data: LayerData::Vector(Arc::new(join::separate(&lds, &record.fields))),
                status: lm_core::model::ImportStatus::Complete,
                raw: None,
                style: None,
            }),
        );
        let mut ops: Vec<Op> = Vec::new();
        // Roles and stars first: their changes need the fields still there.
        for f in ll.roles.keys().filter(|f| gone(f)) {
            ops.extend(self.doc.set_role_tx(target, f, None).map(|t| t.ops).unwrap_or_default());
        }
        for f in ll.starred.iter().filter(|f| gone(f)) {
            ops.extend(self.doc.set_starred_tx(target, f, false).map(|t| t.ops).unwrap_or_default());
        }
        ops.extend(self.doc.set_profile_tx(target, profile).map(|t| t.ops).unwrap_or_default());
        if ll.style.color_by.as_ref().is_some_and(|c| gone(&c.field)) {
            // The colours from before the join, when this join set them; otherwise one colour.
            let before = self.joins.reports.get(&target).filter(|r| r.table_name == table).and_then(|r| r.styled.clone());
            let s = before.unwrap_or_else(|| Style { color_by: None, ..ll.style.clone() });
            ops.extend(self.doc.set_style_tx(target, s).map(|t| t.ops).unwrap_or_default());
        }
        if let Some(f) = &ll.filter
            && f.conditions.iter().any(|c| gone(&c.field))
        {
            let mut f = f.clone();
            f.conditions.retain(|c| !gone(&c.field));
            let to = (!f.conditions.is_empty()).then_some(f);
            ops.extend(self.doc.set_filter_tx(target, to).map(|t| t.ops).unwrap_or_default());
        }
        let elsewhere = self
            .doc
            .layers
            .iter()
            .any(|l| l.id != target && l.profile.joins.iter().any(|j| j.table == record.table && j.source == record.source));
        let back = !elsewhere && self.doc.layer(saved.layer.id).is_none();
        if back && let Some(index) = self.doc.index_of(target) {
            self.data.entry((saved.layer.id, saved.layer.profile.data_rev)).or_insert_with(|| saved.data.clone());
            ops.push(Op::AddLayer { index, layer: Box::new(saved.layer.clone()) });
        }
        if !self.commit(Some(Transaction::new(format!("Unjoin {table}"), ops))) {
            return false;
        }
        // Not joined again on its own; Undo brings the join back.
        self.joins.unjoined.insert(saved.layer.id);
        if self.joins.reports.get(&target).is_some_and(|r| r.table_name == table) {
            self.joins.reports.remove(&target);
        }
        self.select_layer(if back { saved.layer.id } else { target });
        true
    }

    /// The joined tables shown under a layer on the shelf, in the order joined.
    pub fn sublayers(&self, id: LayerId) -> Vec<Sublayer> {
        let Some(l) = self.doc.layer(id) else { return Vec::new() };
        l.profile
            .joins
            .iter()
            .map(|j| {
                let saved = self.joins.joined.get(&(id, j.table.clone()));
                Sublayer { table: j.table.clone(), matched: saved.map(|s| (s.matched, s.layer_rows)), can_unjoin: saved.is_some() }
            })
            .collect()
    }

    /// Whether a layer's joined tables are shown under it on the shelf.
    pub fn sublayers_open(&self, id: LayerId) -> bool {
        !self.joins.collapsed.contains(&id)
    }

    pub fn set_sublayers_open(&mut self, id: LayerId, open: bool) {
        if open {
            self.joins.collapsed.remove(&id);
        } else {
            self.joins.collapsed.insert(id);
        }
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
                style: None,
            }),
        );
        let tx = self.doc.set_profile_tx(target, profile).map(|mut t| {
            t.label = "Use the suggested matches".into();
            t
        });
        if self.commit(tx)
            && let Some(rep) = self.joins.reports.get_mut(&target)
        {
            if let Some(saved) = self.joins.joined.get_mut(&(target, rep.table_name.clone())) {
                saved.matched = joined.layer_matched;
            }
            rep.matched = joined.layer_matched;
            rep.unmatched = joined.unmatched;
            rep.unmatched_rows = joined.unmatched_rows.len();
            rep.blank_keys = joined.blank_keys;
            rep.expected = joined.expected;
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

/// What working out a table's join found, for the finish on the main thread.
enum Outcome {
    Join(Vec<(LayerId, JoinSpec)>),
    Ask(LayerId, Vec<(String, String, Option<join::Expected>, JoinSpec)>),
    Nothing,
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

    /// "Show the 3 not matched", with the rows when a key repeats or is blank ("Show the 2 not
    /// matched (5 rows)"); None when every row found a place.
    pub fn unmatched_label(&self) -> Option<String> {
        let (keys, rows) = (self.unmatched.len(), self.unmatched_rows);
        let plural = |n: usize| if n == 1 { "row" } else { "rows" };
        match (keys, rows) {
            (_, 0) => None,
            (0, r) => Some(format!("Show the {r} {} not matched", plural(r))),
            (k, r) if k == r => Some(format!("Show the {k} not matched")),
            (k, r) => Some(format!("Show the {k} not matched ({r} {})", plural(r))),
        }
    }

    pub fn table_source(&self) -> &lm_core::model::SourceRef {
        &self.table_source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(w: &mut Workspace, rel: &str, batch: u64) {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
        let layers = lm_jobs::import::import_path(&path, &lm_jobs::import::Ctx::detached(Default::default())).unwrap();
        assert!(w.add_imported(layers, batch), "{rel}");
    }

    fn join(w: &mut Workspace) {
        w.try_joins();
        w.wait_pending();
    }

    fn has_gdp(w: &Workspace, id: LayerId) -> bool {
        w.doc.layer(id).unwrap().profile.fields.iter().any(|f| f.name == "gdp_growth_2023")
            && w.data(id).is_some_and(|d| matches!(&d.data, LayerData::Vector(ds) if ds.fields.iter().any(|f| f.name == "gdp_growth_2023")))
    }

    /// Two countries layers and a table of ISO3 codes: the table joins both in one step, shows
    /// under each, and the best match alone is recoloured; Undo puts everything back.
    #[test]
    fn a_table_joins_several_layers_in_one_undo_step() {
        let mut w = Workspace::default();
        open(&mut w, "assets/examples/countries.geojson", 1);
        open(&mut w, "assets/examples/countries.geojson", 2);
        open(&mut w, "fixtures/J1/gdp_growth_iso3.csv", 3);
        let before = w.doc.clone();
        let (a, b) = (w.doc.layers[0].id, w.doc.layers[1].id);
        join(&mut w);
        assert_eq!(w.doc.layers.len(), 2, "the table left the shelf");
        for id in [a, b] {
            assert!(has_gdp(&w, id));
            let subs = w.sublayers(id);
            assert_eq!(subs.len(), 1);
            assert_eq!(subs[0].table, "gdp_growth_iso3.csv");
            assert!(subs[0].can_unjoin && subs[0].matched.is_some_and(|(m, n)| m > 150 && n == 177), "{subs:?}");
            assert!(w.sublayers_open(id), "open right after the join");
            assert!(w.join_report(id).is_some());
        }
        // The higher layer on the shelf (b) is the first target: it alone is coloured.
        assert!(w.doc.layer(b).unwrap().style.color_by.is_some());
        assert!(w.doc.layer(a).unwrap().style.color_by.is_none());
        assert_eq!(w.selected(), Some(b));
        assert_eq!(w.undo.undo_label(), Some("Join gdp_growth_iso3.csv"));
        w.undo();
        assert_eq!(w.doc.layers, before.layers, "one undo restores both layers and the table");
        assert!(!has_gdp(&w, a) && !has_gdp(&w, b));
        join(&mut w);
        assert_eq!(w.doc.layers.len(), 3, "not joined again on its own after Undo");
        w.redo();
        assert!(has_gdp(&w, a) && has_gdp(&w, b));
    }

    /// Unjoin: the layer loses the table's fields and colours, the table comes back below it as
    /// a standalone layer and isn't joined again; one undo step.
    #[test]
    fn unjoin_puts_the_table_back_in_one_undo_step() {
        let mut w = Workspace::default();
        open(&mut w, "assets/examples/countries.geojson", 1);
        open(&mut w, "fixtures/J1/gdp_growth_iso3.csv", 2);
        let c = w.doc.layers[0].id;
        let table = w.doc.layers.iter().find(|l| l.profile.kind == LayerKind::Table).unwrap().clone();
        let original = w.doc.layer(c).unwrap().clone();
        join(&mut w);
        let joined = w.doc.clone();
        assert!(has_gdp(&w, c) && w.doc.layer(c).unwrap().style.color_by.is_some());
        assert!(w.unjoin(c, "gdp_growth_iso3.csv"));
        assert_eq!(w.undo.undo_label(), Some("Unjoin gdp_growth_iso3.csv"));
        assert!(!has_gdp(&w, c));
        let l = w.doc.layer(c).unwrap();
        assert!(l.profile.joins.is_empty() && l.style == original.style && l.profile.fields == original.profile.fields);
        assert_eq!(w.doc.layer(table.id), Some(&table), "the table is back as it was");
        assert_eq!(w.doc.index_of(table.id), Some(0), "just below the layer on the shelf");
        assert!(w.sublayers(c).is_empty());
        join(&mut w);
        assert_eq!(w.doc.layers.len(), 2, "an unjoined table isn't joined again on its own");
        w.undo();
        assert_eq!(w.doc.layers, joined.layers, "Undo joins it again");
        assert!(has_gdp(&w, c));
        w.redo();
        assert!(!has_gdp(&w, c) && w.doc.layer(table.id).is_some());
    }

    /// A table under two layers comes back to the shelf only when the last join goes.
    #[test]
    fn unjoining_from_one_of_two_layers_keeps_it_under_the_other() {
        let mut w = Workspace::default();
        open(&mut w, "assets/examples/countries.geojson", 1);
        open(&mut w, "assets/examples/countries.geojson", 2);
        open(&mut w, "fixtures/J1/gdp_growth_iso3.csv", 3);
        let (a, b) = (w.doc.layers[0].id, w.doc.layers[1].id);
        join(&mut w);
        assert!(w.unjoin(a, "gdp_growth_iso3.csv"));
        assert_eq!(w.doc.layers.len(), 2, "still under b");
        assert!(!has_gdp(&w, a) && has_gdp(&w, b));
        assert!(w.unjoin(b, "gdp_growth_iso3.csv"));
        assert_eq!(w.doc.layers.len(), 3, "the last unjoin brings it back");
        assert!(w.doc.layer(b).unwrap().style.color_by.is_none());
    }
}
