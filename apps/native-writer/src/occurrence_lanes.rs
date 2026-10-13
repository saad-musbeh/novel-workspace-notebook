//! One authored canonical event may be shown in several occurrence lanes.
use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub(crate) struct LaneRow {
    pub(crate) lane: String,
    pub(crate) event: String,
    pub(crate) title: String,
    pub(crate) time: String,
    pub(crate) scenes: Rc<Vec<String>>,
}
#[derive(Default, Debug)]
pub(crate) struct LaneReport {
    pub(crate) rows: Vec<LaneRow>,
    pub(crate) groups: Vec<String>,
    pub(crate) notes: Vec<String>,
    pub(crate) limited: bool,
}
fn caption(l: &Value) -> String {
    format!(
        "{} · {} [{}]",
        if l["kind"] == "main" {
            "Main"
        } else if l["kind"] == "subplot" {
            "Subplot"
        } else {
            "Unrecognized lane kind"
        },
        excerpt(text(l, "name")),
        excerpt(text(l, "id"))
    )
}
pub(crate) fn project(doc: &Value, live: &HashSet<String>) -> LaneReport {
    let analysis = ChronologyAnalysis::new(doc);
    let lanes = unique(rows(doc, "lanes"));
    let events = unique(rows(doc, "events"));
    let mut out = LaneReport::default();
    let mut budget = Budget::default();
    if rows(doc, "events").len() > ROWS || rows(doc, "lanes").len() > ROWS {
        out.limited = true;
        out.notes.push("Saved event/lane collections exceed this bounded projection; no partial identities are selected. Stored data is unchanged.".into());
        return out;
    }
    for lane in rows(doc, "lanes").iter().take(ROWS) {
        if lanes.contains_key(text(lane, "id")) {
            out.groups.push(caption(lane));
        }
    }
    if rows(doc, "lanes")
        .iter()
        .filter(|l| l["kind"] == "main")
        .count()
        != 1
    {
        out.notes.push("The authored chronology does not have exactly one main lane. Memberships have not been changed.".into());
    }
    for event in rows(doc, "events").iter().take(ROWS) {
        if !budget.visit() {
            break;
        }
        let id = text(event, "id");
        if !events.contains_key(id) {
            out.notes.push(format!(
                "Ambiguous or missing event identity: {}",
                excerpt(id)
            ));
            continue;
        }
        let order = analysis.order(id, id);
        let time = if order == EventOrder::Conflict {
            format!(
                "Conflicting authored chronology · {}",
                analysis.contributors(id).unwrap_or(&[]).join(", ")
            )
        } else if order == EventOrder::Unknown {
            "Incomplete chronology evidence; timing unresolved".into()
        } else {
            analysis
                .range(id)
                .map(|(a, b)| {
                    if a == b {
                        format!("Day {a}")
                    } else {
                        format!("Days {a}–{b}")
                    }
                })
                .unwrap_or("Unknown story time".into())
        };
        let mut membership = BTreeSet::new();
        for lane in rows(event, "laneIds").iter().take(ROWS) {
            if !budget.visit() {
                break;
            }
            if let Some(id) = lane.as_str() {
                if !membership.insert(id) {
                    out.notes.push(format!(
                        "Repeated lane membership for {}: {}",
                        excerpt(text(event, "id")),
                        excerpt(id)
                    ));
                }
            } else {
                out.notes.push(format!(
                    "Malformed lane membership for {}",
                    excerpt(text(event, "id"))
                ));
            }
        }
        if rows(event, "laneIds").len() > ROWS {
            out.limited = true;
        }
        let mut scenes = BTreeSet::new();
        for scene in rows(event, "sceneIds").iter().take(100) {
            if !budget.visit() {
                break;
            }
            if let Some(scene) = scene.as_str() {
                if live.contains(scene) {
                    scenes.insert(scene.to_owned());
                } else {
                    out.notes.push(format!(
                        "Missing linked scene for {}: {}",
                        excerpt(id),
                        excerpt(scene)
                    ));
                }
            }
        }
        if rows(event, "sceneIds").len() > 100 {
            out.limited = true;
        }
        let scenes = Rc::new(scenes.into_iter().collect::<Vec<_>>());
        if membership.is_empty() {
            out.rows.push(LaneRow {
                lane: "Unassigned events".into(),
                event: id.into(),
                title: excerpt(text(event, "title")),
                time: time.clone(),
                scenes: scenes.clone(),
            });
        }
        for lane in membership {
            let caption = lanes
                .get(lane)
                .map(|l| caption(l))
                .unwrap_or_else(|| format!("Missing or ambiguous lane: {}", excerpt(lane)));
            out.rows.push(LaneRow {
                lane: caption,
                event: id.into(),
                title: excerpt(text(event, "title")),
                time: time.clone(),
                scenes: scenes.clone(),
            });
        }
    }
    if out.rows.len() > ROWS {
        out.rows.truncate(ROWS);
        out.limited = true;
    }
    out.limited |= budget.limited
        || analysis.limited()
        || rows(doc, "events").len() > ROWS
        || rows(doc, "lanes").len() > ROWS;
    out
}
pub(crate) fn render(report: &LaneReport) -> String {
    let mut s=String::from("Authored occurrence lanes · Events retain their canonical IDs and unchanged dates. Order below follows stored event rows; it does not schedule the story.\n\n");
    let mut groups = BTreeMap::<&str, Vec<&LaneRow>>::new();
    for group in &report.groups {
        groups.entry(group).or_default();
    }
    for row in &report.rows {
        groups.entry(&row.lane).or_default().push(row);
    }
    for (group, entries) in groups {
        s += &format!("{group}\n\n");
        if entries.is_empty() {
            s += "No authored events in this lane.\n\n";
        }
        for row in entries {
            s += &format!(
                "{} · {}\n{}\nExplicit live scene links: {}\n\n",
                row.event,
                row.title,
                row.time,
                if row.scenes.is_empty() {
                    "None".into()
                } else {
                    row.scenes
                        .iter()
                        .take(10)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                        + if row.scenes.len() > 10 {
                            " (more links available in the selector)"
                        } else {
                            ""
                        }
                }
            );
        }
    }
    for note in &report.notes {
        s += &format!("{note}\n");
    }
    if report.limited {
        s+="Lane coverage limit reached. Some authored memberships or timing evidence remain unresolved.\n";
    }
    s
}
