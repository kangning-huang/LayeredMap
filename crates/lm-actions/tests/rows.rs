//! Snapshot tests for the Selection → suggested actions table: one fixture context per row and
//! per merged pair, rendered as plain text and compared exactly.
//!
//! Views: `dev6` is the developer surface at phase 6 (every action, so the full table order
//! shows); `p3` is phase 3, which must be the same with and without the developer flag (every
//! assistant action is Phase 4) and is what the free app shows.

use lm_actions::{Context, FieldSummary, OpenLayer, Options, Selection, Suggestions, suggest};
use lm_core::model::{Confidence, FieldRole, LayerId, LayerKind, MeasureKind};

const SCHOOLS: LayerId = LayerId(1);
const DISTRICTS: LayerId = LayerId(2);
const HEAT: LayerId = LayerId(3);
const ROADS: LayerId = LayerId(4);
const TABLE: LayerId = LayerId(5);

fn f(name: &str, role: FieldRole) -> FieldSummary {
    FieldSummary::new(name, role, Confidence::Likely)
}

fn schools(seq: u64) -> OpenLayer {
    OpenLayer::new(SCHOOLS, "Schools", LayerKind::Points, 0, seq).with_fields(vec![
        f("name", FieldRole::Name),
        f("type", FieldRole::Category),
        f("enrollment", FieldRole::Measure(MeasureKind::Count)),
    ])
}

fn districts(seq: u64) -> OpenLayer {
    OpenLayer::new(DISTRICTS, "Districts", LayerKind::Areas, 1, seq).with_fields(vec![
        f("GEOID", FieldRole::Identifier),
        f("zone", FieldRole::Category),
        f("median_income", FieldRole::Measure(MeasureKind::Intensive)),
    ])
}

fn heat(seq: u64) -> OpenLayer {
    OpenLayer::new(HEAT, "Heat", LayerKind::Raster, 2, seq)
}

fn roads(seq: u64) -> OpenLayer {
    OpenLayer::new(ROADS, "Roads", LayerKind::Lines, 0, seq)
        .with_fields(vec![f("class", FieldRole::Category), f("lanes", FieldRole::Measure(MeasureKind::Count))])
}

fn table(seq: u64) -> OpenLayer {
    OpenLayer::new(TABLE, "Census table", LayerKind::Table, 0, seq).with_fields(vec![
        f("geoid", FieldRole::Identifier),
        f("lon", FieldRole::Coordinate),
        f("lat", FieldRole::Coordinate),
        f("poverty_rate", FieldRole::Measure(MeasureKind::Intensive)),
    ])
}

fn obj(layer: LayerId) -> Selection {
    Selection::Object { layer }
}

fn lyr(layer: LayerId) -> Selection {
    Selection::Layer { layer }
}

/// Renders suggestions as plain text: the top actions, then More by group.
fn render(s: &Suggestions) -> String {
    let labels = |v: &[lm_actions::Suggested]| {
        for x in v {
            assert!(!x.label.contains('['), "unfilled token in {:?}", x.label);
        }
        v.iter().map(|x| x.label.as_str()).collect::<Vec<_>>().join(" | ")
    };
    let mut out = format!("top: {}", if s.top.is_empty() { "-".to_string() } else { labels(&s.top) });
    for (g, items) in &s.more {
        out.push_str(&format!("\n{}: {}", g.label(), labels(items)));
    }
    out
}

struct Case {
    name: &'static str,
    rows: &'static [u8],
    ctx: Context,
    dev6: &'static str,
    p3: &'static str,
}

fn check(c: &Case) {
    let dev6 = suggest(&c.ctx, Options::developer(6));
    assert_eq!(dev6.rows, c.rows, "{}: matched rows", c.name);
    assert_eq!(render(&dev6), c.dev6, "{}: developer view, phase 6", c.name);
    let dev3 = render(&suggest(&c.ctx, Options::developer(3)));
    let free3 = render(&suggest(&c.ctx, Options::free(3)));
    assert_eq!(dev3, c.p3, "{}: developer view, phase 3", c.name);
    assert_eq!(free3, c.p3, "{}: free app, phase 3", c.name);
}

fn row_cases() -> Vec<Case> {
    vec![
        Case {
            name: "row 1: nothing, no data",
            rows: &[1],
            ctx: Context::new(Selection::Nothing, vec![]),
            dev6: "top: Open file | Try an example | Browse data library",
            p3: "top: Open file | Try an example",
        },
        Case {
            name: "row 2: one point, no other layer",
            rows: &[2],
            ctx: Context::new(obj(SCHOOLS), vec![schools(1)]),
            dev6: concat!(
                "top: Show an area within a distance | Measure from here | Filter by type\n",
                "Compare: Compare with other Schools",
            ),
            p3: concat!("top: Filter by type\n", "Compare: Compare with other Schools",),
        },
        Case {
            name: "row 3: one point, area layer open",
            rows: &[3],
            ctx: Context::new(obj(SCHOOLS), vec![schools(1), districts(2)]),
            dev6: concat!(
                "top: Which District is it in? | Show an area within a distance | Measure from here\n",
                "Compare: Compare with other Schools\n",
                "Change look: Filter by type",
            ),
            p3: concat!("top: -\n", "Compare: Compare with other Schools\n", "Change look: Filter by type",),
        },
        Case {
            name: "row 4: point layer, no other layer",
            rows: &[4],
            ctx: Context::new(lyr(SCHOOLS), vec![schools(1)]),
            dev6: concat!(
                "top: Color or size by a value | Filter Schools | Show an area within a distance of each\n",
                "Understand: View data\n",
                "Compare: Rank Schools by a value\n",
                "Measure: Add the distance to the nearest neighbour in Schools | Add area and coordinates to Schools | Add a field calculated from two values",
            ),
            p3: "top: Color or size by a value | Filter Schools\nUnderstand: View data",
        },
        Case {
            name: "row 5: point layer, area layer open",
            rows: &[5],
            ctx: Context::new(lyr(SCHOOLS), vec![schools(1), districts(2)]),
            dev6: concat!(
                "top: Count Schools in each District | Color or size by a value | Filter Schools\n",
                "Understand: View data\n",
                "Compare: Rank Schools by a value\n",
                "Measure: Show an area within a distance of each | Add the distance to the nearest of Districts to Schools | Add the distance to the nearest neighbour in Schools | Add Districts details to Schools | Add area and coordinates to Schools | Add a field calculated from two values | Keep the part of Schools inside Districts | Remove the part of Schools inside Districts",
            ),
            p3: "top: Color or size by a value | Filter Schools\nUnderstand: View data",
        },
        Case {
            name: "row 6: one area, no other layer",
            rows: &[6],
            ctx: Context::new(obj(DISTRICTS), vec![districts(1)]),
            dev6: "top: Compare with other Districts | Measure its area | Filter by zone",
            p3: "top: Compare with other Districts | Filter by zone",
        },
        Case {
            name: "row 7: one area, point layer open",
            rows: &[7],
            ctx: Context::new(obj(DISTRICTS), vec![districts(1), schools(2)]),
            dev6: concat!("top: What's inside? | Compare with other Districts | Measure its area\n", "Change look: Filter by zone",),
            p3: concat!("top: Compare with other Districts\n", "Change look: Filter by zone",),
        },
        Case {
            name: "row 8: one area, raster open",
            rows: &[8],
            ctx: Context::new(obj(DISTRICTS), vec![districts(1), heat(2)]),
            dev6: concat!("top: Summarize Heat here | Compare with other Districts | Measure its area\n", "Change look: Filter by zone",),
            p3: concat!("top: Compare with other Districts\n", "Change look: Filter by zone",),
        },
        Case {
            name: "row 9: area layer, raster open",
            rows: &[9],
            ctx: Context::new(lyr(DISTRICTS), vec![districts(1), heat(2)]),
            dev6: concat!(
                "top: Summarize Heat in each District | Color by a value | Rank Districts by a value\n",
                "Understand: View data\n",
                "Measure: Add the distance to the nearest neighbour in Districts | Add area and coordinates to Districts | Add a field calculated from two values | Split Districts into separate parts\n",
                "Change look: Filter Districts",
            ),
            p3: "top: Color by a value\nUnderstand: View data\nChange look: Filter Districts",
        },
        Case {
            name: "row 10: area layer, point layer open",
            rows: &[10],
            ctx: Context::new(lyr(DISTRICTS), vec![districts(1), schools(2)]),
            dev6: concat!(
                "top: Count Schools in each District | Color by a value | Rank Districts by a value\n",
                "Understand: View data\n",
                "Measure: Add the distance to the nearest of Schools to Districts | Add the distance to the nearest neighbour in Districts | Add area and coordinates to Districts | Add a field calculated from two values | Split Districts into separate parts\n",
                "Change look: Filter Districts",
            ),
            p3: "top: Color by a value\nUnderstand: View data\nChange look: Filter Districts",
        },
        Case {
            name: "row 11: area layer, no other layer",
            rows: &[11],
            ctx: Context::new(lyr(DISTRICTS), vec![districts(1)]),
            dev6: concat!(
                "top: Color by a value | Rank Districts by a value | Filter Districts\n",
                "Understand: View data\n",
                "Measure: Add the distance to the nearest neighbour in Districts | Add area and coordinates to Districts | Add a field calculated from two values | Split Districts into separate parts",
            ),
            p3: "top: Color by a value | Filter Districts\nUnderstand: View data",
        },
        Case {
            name: "row 12: one line, area layer open",
            rows: &[12],
            ctx: Context::new(obj(ROADS), vec![roads(1), districts(2)]),
            dev6: concat!(
                "top: Measure length | Color by a value | Show an area within a distance\n",
                "Compare: Compare with other Roads\n",
                "Change look: Filter by class",
            ),
            p3: concat!("top: Color by a value\n", "Compare: Compare with other Roads\n", "Change look: Filter by class",),
        },
        Case {
            name: "row 12: line layer alone",
            rows: &[12],
            ctx: Context::new(lyr(ROADS), vec![roads(1)]),
            dev6: concat!(
                "top: Measure length | Color by a value | Show an area within a distance\n",
                "Understand: View data\n",
                "Compare: Rank Roads by a value\n",
                "Measure: Add the distance to the nearest neighbour in Roads | Add area and coordinates to Roads | Add a field calculated from two values | Split Roads into separate parts\n",
                "Change look: Filter Roads",
            ),
            p3: "top: Color by a value\nUnderstand: View data\nChange look: Filter Roads",
        },
        Case {
            name: "row 13: one raster pixel",
            rows: &[13],
            ctx: Context::new(Selection::RasterPixel { layer: HEAT }, vec![heat(1)]),
            dev6: concat!(
                "top: Show where values are like this | Change colors | Summarize in an area I draw\n",
                "Change look: Show only values above…",
            ),
            p3: concat!("top: Show where values are like this | Change colors\n", "Change look: Show only values above…",),
        },
        Case {
            name: "row 14: raster layer, area layer open",
            rows: &[14],
            ctx: Context::new(lyr(HEAT), vec![heat(1), districts(2)]),
            dev6: concat!(
                "top: Summarize Heat in each District | Change colors and range | Show only values above…\n",
                "Measure: Summarize in an area I draw | Find areas where Heat meets conditions",
            ),
            p3: "top: Change colors and range | Show only values above…",
        },
        Case {
            name: "row 15: raster layer, no other layer",
            rows: &[15],
            ctx: Context::new(lyr(HEAT), vec![heat(1)]),
            dev6: "top: Change colors and range | Show only values above… | Summarize in an area I draw\nMeasure: Find areas where Heat meets conditions",
            p3: "top: Change colors and range | Show only values above…",
        },
        Case {
            name: "row 16: table, a layer open",
            rows: &[16],
            ctx: Context::new(lyr(TABLE), vec![table(1), districts(2)]),
            dev6: "top: Match to Districts | Map using coordinate columns | View data\nMeasure: Add a field calculated from two values",
            p3: "top: Match to Districts | View data",
        },
        Case {
            name: "row 17: drawn area, point and area layers open",
            rows: &[17],
            ctx: Context::new(Selection::DrawnArea, vec![schools(1), districts(2)]),
            dev6: "top: Summarize data here | Count Schools here | Measure its area",
            p3: "top: -",
        },
        Case {
            name: "row 18: point layer, raster open",
            rows: &[18],
            ctx: Context::new(lyr(SCHOOLS), vec![schools(1), heat(2)]),
            dev6: concat!(
                "top: Add Heat values to each School | Color or size by a value | Filter Schools\n",
                "Understand: View data\n",
                "Compare: Rank Schools by a value\n",
                "Measure: Show an area within a distance of each | Add the distance to the nearest neighbour in Schools | Add area and coordinates to Schools | Add a field calculated from two values",
            ),
            p3: "top: Color or size by a value | Filter Schools\nUnderstand: View data",
        },
    ]
}

fn merged_cases() -> Vec<Case> {
    vec![
        Case {
            name: "merge 5+18: point layer, area then raster added",
            rows: &[18, 5],
            ctx: Context::new(lyr(SCHOOLS), vec![schools(1), districts(2), heat(3)]),
            dev6: concat!(
                "top: Add Heat values to each School | Count Schools in each District | Color or size by a value\n",
                "Understand: View data\n",
                "Compare: Rank Schools by a value\n",
                "Measure: Show an area within a distance of each | Add the distance to the nearest of Districts to Schools | Add the distance to the nearest neighbour in Schools | Add Districts details to Schools | Add area and coordinates to Schools | Add a field calculated from two values | Keep the part of Schools inside Districts | Remove the part of Schools inside Districts\n",
                "Change look: Filter Schools",
            ),
            p3: "top: Color or size by a value | Filter Schools\nUnderstand: View data",
        },
        Case {
            name: "merge 5+18: point layer, raster then area added",
            rows: &[5, 18],
            ctx: Context::new(lyr(SCHOOLS), vec![schools(1), heat(2), districts(3)]),
            dev6: concat!(
                "top: Count Schools in each District | Add Heat values to each School | Color or size by a value\n",
                "Understand: View data\n",
                "Compare: Rank Schools by a value\n",
                "Measure: Show an area within a distance of each | Add the distance to the nearest of Districts to Schools | Add the distance to the nearest neighbour in Schools | Add Districts details to Schools | Add area and coordinates to Schools | Add a field calculated from two values | Keep the part of Schools inside Districts | Remove the part of Schools inside Districts\n",
                "Change look: Filter Schools",
            ),
            p3: "top: Color or size by a value | Filter Schools\nUnderstand: View data",
        },
        Case {
            name: "merge 9+10: area layer, points then raster added",
            rows: &[9, 10],
            ctx: Context::new(lyr(DISTRICTS), vec![districts(1), schools(2), heat(3)]),
            dev6: concat!(
                "top: Summarize Heat in each District | Count Schools in each District | Color by a value\n",
                "Understand: View data\n",
                "Compare: Rank Districts by a value\n",
                "Measure: Add the distance to the nearest of Schools to Districts | Add the distance to the nearest neighbour in Districts | Add area and coordinates to Districts | Add a field calculated from two values | Split Districts into separate parts\n",
                "Change look: Filter Districts",
            ),
            p3: "top: Color by a value\nUnderstand: View data\nChange look: Filter Districts",
        },
        Case {
            name: "merge 9+10: area layer, raster then points added",
            rows: &[10, 9],
            ctx: Context::new(lyr(DISTRICTS), vec![districts(1), heat(2), schools(3)]),
            dev6: concat!(
                "top: Count Schools in each District | Summarize Heat in each District | Color by a value\n",
                "Understand: View data\n",
                "Compare: Rank Districts by a value\n",
                "Measure: Add the distance to the nearest of Schools to Districts | Add the distance to the nearest neighbour in Districts | Add area and coordinates to Districts | Add a field calculated from two values | Split Districts into separate parts\n",
                "Change look: Filter Districts",
            ),
            p3: "top: Color by a value\nUnderstand: View data\nChange look: Filter Districts",
        },
        Case {
            name: "merge 7+8: one area, points then raster added",
            rows: &[8, 7],
            ctx: Context::new(obj(DISTRICTS), vec![districts(1), schools(2), heat(3)]),
            dev6: concat!(
                "top: Summarize Heat here | What's inside? | Compare with other Districts\n",
                "Measure: Measure its area\n",
                "Change look: Filter by zone",
            ),
            p3: concat!("top: Compare with other Districts\n", "Change look: Filter by zone",),
        },
        Case {
            name: "merge 7+8: one area, raster then points added",
            rows: &[7, 8],
            ctx: Context::new(obj(DISTRICTS), vec![districts(1), heat(2), schools(3)]),
            dev6: concat!(
                "top: What's inside? | Summarize Heat here | Compare with other Districts\n",
                "Measure: Measure its area\n",
                "Change look: Filter by zone",
            ),
            p3: concat!("top: Compare with other Districts\n", "Change look: Filter by zone",),
        },
    ]
}

#[test]
fn table_rows() {
    for c in row_cases() {
        check(&c);
    }
}

#[test]
fn merged_pairs() {
    for c in merged_cases() {
        check(&c);
    }
}

#[test]
#[ignore = "prints the current output, to review when the rules change"]
fn dump() {
    for c in row_cases().into_iter().chain(merged_cases()) {
        let dev6 = suggest(&c.ctx, Options::developer(6));
        println!("== {} rows {:?}\n-- dev6\n{}\n-- p3\n{}\n", c.name, dev6.rows, render(&dev6), render(&suggest(&c.ctx, Options::free(3))));
    }
}
