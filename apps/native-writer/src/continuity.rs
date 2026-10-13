//! Saved authored continuity questions. No prose inference or canonical writes.
use super::*;
use crate::planning::{ChronologyAnalysis, EventOrder};
use std::collections::{BTreeSet, HashSet};
#[path = "occurrence_lanes.rs"]
pub(crate) mod lanes;

pub(crate) const RULES: &[(&str, &str)] = &[
    ("chronology-cycle", "Confirmed chronology contradictions"),
    ("missing-prerequisite", "Missing prerequisite"),
    ("knowledge-before-learning", "Knowledge used too early"),
    ("exclusive-presence", "Incompatible exclusive presence"),
    ("confirmed-state-conflict", "Conflicting confirmed state"),
    ("unresolved-setup", "Required payoff reminders"),
];
const WORK: usize = 20_000;
const ROWS: usize = 2_000;
fn rows<'a>(d: &'a Value, k: &str) -> &'a [Value] {
    d[k].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn text<'a>(d: &'a Value, k: &str) -> &'a str {
    d[k].as_str().unwrap_or("")
}
fn excerpt(s: &str) -> String {
    let mut c = s.chars();
    let mut s = c.by_ref().take(2_000).collect::<String>();
    if c.next().is_some() {
        s.push('…');
    }
    s
}
#[derive(Default)]
pub(crate) struct Budget {
    work: usize,
    pub(crate) limited: bool,
}
impl Budget {
    pub(crate) fn visit(&mut self) -> bool {
        self.work += 1;
        if self.work > WORK {
            self.limited = true;
            false
        } else {
            true
        }
    }
}
fn unique<'a>(rows: &'a [Value]) -> HashMap<&'a str, &'a Value> {
    let mut values = HashMap::new();
    let mut duplicates = HashSet::new();
    for row in rows.iter().take(ROWS) {
        let id = text(row, "id");
        if id.is_empty() || id.len() > 200 {
            continue;
        }
        if values.insert(id, row).is_some() {
            duplicates.insert(id);
        }
    }
    for id in duplicates {
        values.remove(id);
    }
    values
}
#[derive(Clone, Debug)]
pub(crate) struct Finding {
    pub(crate) rule: String,
    pub(crate) title: String,
    pub(crate) explanation: String,
    pub(crate) ids: Vec<String>,
    pub(crate) links: BTreeSet<(String, String)>,
    pub(crate) annotation: Vec<String>,
}
#[derive(Default, Debug)]
pub(crate) struct Report {
    pub(crate) findings: Vec<Finding>,
    pub(crate) unresolved: usize,
    pub(crate) limited: bool,
}
fn finding(
    rule: &str,
    title: &str,
    why: &str,
    ids: Vec<String>,
    links: impl IntoIterator<Item = (String, String)>,
) -> Finding {
    Finding {
        rule: rule.into(),
        title: title.into(),
        explanation: why.into(),
        ids,
        links: links.into_iter().collect(),
        annotation: Vec::new(),
    }
}
fn link(kind: &str, id: &str) -> (String, String) {
    (kind.into(), id.into())
}
/// Approximate bounds are windows of uncertainty, not occupied intervals. A
/// relative event is definite only when its complete anchor chain is definite.
fn definite_range(
    analysis: &ChronologyAnalysis<'_>,
    events: &HashMap<&str, &Value>,
    id: &str,
    budget: &mut Budget,
) -> Option<(f64, f64)> {
    let mut current = id;
    let mut seen = HashSet::new();
    loop {
        if !budget.visit() || !seen.insert(current) {
            return None;
        }
        let event = events.get(current)?;
        match text(&event["time"], "kind") {
            "fixed" | "interval" => break,
            "relative" => current = event["time"]["eventId"].as_str()?,
            _ => return None,
        }
    }
    if analysis.range_uncertainty(id) != Some(0.0) {
        return None;
    }
    if matches!(
        analysis.order(id, id),
        EventOrder::Unknown | EventOrder::Conflict
    ) {
        None
    } else {
        analysis.range(id)
    }
}
fn overlap(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0.max(b.0) <= a.1.min(b.1)
}
fn exact_annotations(doc: &Value, f: &Finding, budget: &mut Budget) -> Vec<String> {
    let mut out = Vec::new();
    if f.links.is_empty() {
        return out;
    }
    for issue in rows(doc, "issues").iter().take(ROWS) {
        if !budget.visit() {
            break;
        }
        if text(issue, "category") != f.rule
            || !matches!(text(issue, "status"), "intentional" | "dismissed")
        {
            continue;
        }
        let source = rows(issue, "links");
        if source.len() > ROWS {
            budget.limited = true;
            continue;
        }
        let mut links = BTreeSet::new();
        let mut complete = true;
        for row in source {
            if !budget.visit() {
                complete = false;
                break;
            }
            let (Some(kind), Some(id)) = (row["kind"].as_str(), row["id"].as_str()) else {
                complete = false;
                break;
            };
            if !matches!(kind, "record" | "event" | "scene" | "plot-point") || id.is_empty() {
                complete = false;
                break;
            }
            links.insert((kind.into(), id.into()));
        }
        if complete && links == f.links {
            out.push(format!(
                "Author annotation {} · {} · {}",
                excerpt(text(issue, "id")),
                text(issue, "status"),
                excerpt(text(issue, "resolution"))
            ));
            if out.len() == 20 {
                budget.limited = true;
                break;
            }
        }
    }
    out
}

pub(crate) fn check(doc: &Value, enabled: &BTreeSet<String>) -> Report {
    let analysis = ChronologyAnalysis::new(doc);
    let mut budget = Budget::default();
    let mut out = Report::default();
    let events = unique(rows(doc, "events"));
    let records = unique(rows(doc, "records"));
    let points = unique(rows(doc, "plotPoints"));
    let facts = unique(rows(doc, "facts"));
    let approved = rows(doc, "facts")
        .iter()
        .take(ROWS)
        .filter(|f| {
            f["confirmed"] == true
                && f["status"] == "fact"
                && facts.contains_key(text(f, "id"))
                && records.contains_key(text(f, "subjectId"))
                && events.contains_key(text(f, "eventId"))
        })
        .collect::<Vec<_>>();
    out.limited = [
        "events",
        "records",
        "facts",
        "boards",
        "plotPoints",
        "issues",
    ]
    .iter()
    .any(|k| rows(doc, k).len() > ROWS);
    if out.limited {
        out.unresolved = 1;
        return out;
    }
    if enabled.contains("chronology-cycle") {
        let mut seen = BTreeSet::new();
        for event in rows(doc, "events").iter().take(ROWS) {
            if !budget.visit() {
                break;
            }
            if let Some(ids) = analysis.contributors(text(event, "id")) {
                let ids = ids.to_vec();
                if seen.insert(ids.clone()) {
                    out.findings.push(finding("chronology-cycle","Confirmed chronology cannot satisfy these links","Review the contributing authored ordering/relative links. Dates have not been changed.",ids,[]));
                }
            }
        }
    }
    if enabled.contains("missing-prerequisite") {
        for board in rows(doc, "boards").iter().take(ROWS) {
            let cards = unique(rows(board, "cards"));
            if rows(board, "cards").len() > ROWS || rows(board, "edges").len() > ROWS {
                out.limited = true;
                out.unresolved += 1;
                continue;
            }
            for edge in rows(board, "edges").iter().take(ROWS) {
                if !budget.visit() {
                    break;
                }
                if edge["confirmed"] != true || edge["kind"] != "depends-on" {
                    continue;
                }
                let dependent = cards.get(text(edge, "fromId")).map(|c| &c["reference"]);
                let prerequisite = cards.get(text(edge, "toId")).map(|c| &c["reference"]);
                let (Some(a), Some(b)) = (dependent, prerequisite) else {
                    out.unresolved += 1;
                    continue;
                };
                let (aid, bid) = (text(a, "id"), text(b, "id"));
                if a["kind"] != "event"
                    || b["kind"] != "event"
                    || !events.contains_key(aid)
                    || !events.contains_key(bid)
                {
                    out.unresolved += 1;
                    continue;
                }
                match analysis.order(aid,bid){EventOrder::Before=>out.findings.push(finding("missing-prerequisite","An event precedes its confirmed prerequisite","Check the explicit dependency or the established story order; no missing prose is assumed.",vec![text(edge,"id").into(),aid.into(),bid.into()],[link("event",aid),link("event",bid)])),EventOrder::Unknown|EventOrder::Unordered|EventOrder::Conflict=>out.unresolved+=1,_=>{}}
            }
        }
    }
    if enabled.contains("knowledge-before-learning") {
        for use_fact in approved.iter().filter(|f| f["property"] == "knowledge:use") {
            if !budget.visit() {
                break;
            }
            if !records
                .get(text(use_fact, "subjectId"))
                .is_some_and(|r| r["kind"] == "character")
            {
                out.unresolved += 1;
                continue;
            }
            let mut learns = Vec::new();
            for f in &approved {
                if !budget.visit() {
                    break;
                }
                if f["property"] == "knowledge:learn"
                    && f["subjectId"] == use_fact["subjectId"]
                    && f["value"] == use_fact["value"]
                {
                    learns.push(f);
                }
            }
            if budget.limited {
                break;
            }
            if learns.is_empty() {
                out.unresolved += 1;
                continue;
            }
            let mut all_early = true;
            let mut uncertain = false;
            for learn in &learns {
                if !budget.visit() {
                    all_early = false;
                    break;
                }
                match analysis.order(text(use_fact, "eventId"), text(learn, "eventId")) {
                    EventOrder::Before => {}
                    EventOrder::Unknown | EventOrder::Unordered | EventOrder::Conflict => {
                        uncertain = true;
                        all_early = false;
                    }
                    _ => all_early = false,
                }
            }
            if all_early {
                let mut ids = vec![text(use_fact, "id").into()];
                let mut links = vec![
                    link("record", text(use_fact, "subjectId")),
                    link("event", text(use_fact, "eventId")),
                ];
                for learn in learns {
                    ids.push(text(learn, "id").into());
                    links.push(link("event", text(learn, "eventId")));
                }
                out.findings.push(finding("knowledge-before-learning","Knowledge is used before its recorded acquisition","The approved use precedes every approved matching learning record. Review an intentional secret or off-page explanation; frozen character knowledge is not treated as use.",ids,links));
            } else if uncertain {
                out.unresolved += 1;
            }
        }
    }
    for (rule, property) in [
        ("exclusive-presence", Some("exclusive-location")),
        ("confirmed-state-conflict", None),
    ] {
        if !enabled.contains(rule) {
            continue;
        }
        let selected = approved
            .iter()
            .copied()
            .filter(|f| {
                if let Some(p) = property {
                    f["property"] == p
                } else {
                    !matches!(
                        text(f, "property"),
                        "knowledge:learn" | "knowledge:use" | "exclusive-location"
                    )
                }
            })
            .collect::<Vec<_>>();
        let mut ranges = HashMap::new();
        for fact in &selected {
            if !budget.visit() {
                break;
            }
            let mut range = definite_range(&analysis, &events, text(fact, "eventId"), &mut budget);
            if property.is_none() {
                if let Some(until) = fact["untilEventId"].as_str() {
                    range = range
                        .zip(definite_range(&analysis, &events, until, &mut budget))
                        .filter(|_| {
                            matches!(
                                analysis.order(text(fact, "eventId"), until),
                                EventOrder::Before | EventOrder::Same
                            )
                        })
                        .map(|(start, end)| (start.0, end.1));
                }
            }
            if range.is_none() {
                out.unresolved += 1;
            }
            ranges.insert(text(fact, "id"), range);
        }
        'pairs: for (i, a) in selected.iter().enumerate() {
            for b in selected.iter().skip(i + 1) {
                if !budget.visit() {
                    break 'pairs;
                }
                if a["subjectId"] != b["subjectId"]
                    || a["value"] == b["value"]
                    || property.is_none() && a["property"] != b["property"]
                {
                    continue;
                }
                if property.is_some()
                    && [a, b].iter().any(|f| {
                        !records
                            .get(text(f, "value"))
                            .is_some_and(|r| r["kind"] == "location")
                    })
                {
                    out.unresolved += 1;
                    continue;
                }
                let (Some(Some(ar)), Some(Some(br))) =
                    (ranges.get(text(a, "id")), ranges.get(text(b, "id")))
                else {
                    continue;
                };
                if !overlap(*ar, *br) {
                    continue;
                }
                let mut links = vec![
                    link("record", text(a, "subjectId")),
                    link("event", text(a, "eventId")),
                    link("event", text(b, "eventId")),
                ];
                for f in [a, b] {
                    if property.is_some() {
                        links.push(link("record", text(f, "value")));
                    } else if let Some(id) = f["untilEventId"].as_str() {
                        links.push(link("event", id));
                    }
                }
                out.findings.push(finding(rule,if property.is_some(){"Two explicit exclusive locations overlap"}else{"Confirmed states disagree during established applicability"},if property.is_some(){"The same subject is explicitly assigned different exclusive locations at overlapping definite story times. No travel-speed assumption is added."}else{"Different approved values of one property have overlapping definite applicability. Review the sources; unknown endings are not extended indefinitely."},vec![text(a,"id").into(),text(b,"id").into()],links));
            }
        }
    }
    if enabled.contains("unresolved-setup") {
        for point in rows(doc, "plotPoints").iter().take(ROWS) {
            if !budget.visit() {
                break;
            }
            if point["requiresPayoff"] != true || point["scopeClosed"] != true {
                continue;
            }
            let payoff = point["payoffId"].as_str();
            if payoff.is_some_and(|id| points.contains_key(id)) {
                continue;
            }
            out.findings.push(finding("unresolved-setup",&format!("Review the payoff for “{}”",excerpt(text(point,"title"))),if payoff.is_some(){"The closed authored setup refers to a missing payoff. This is a reminder, not proof of a plot hole."}else{"The closed authored setup explicitly requires a payoff and has no linked payoff. This is a reminder, not proof of a plot hole."},vec![text(point,"id").into()],[link("plot-point",text(point,"id"))]));
        }
    }
    if out.findings.len() > 200 {
        out.findings.truncate(200);
        out.limited = true;
    }
    for f in &mut out.findings {
        for id in &f.ids {
            if !budget.visit() {
                break;
            }
            if let Some(fact) = facts.get(id.as_str()) {
                f.explanation += &format!(
                    "\nAuthored fact {}: {} = {} · {} · Source: {}",
                    excerpt(id),
                    excerpt(text(fact, "property")),
                    excerpt(text(fact, "value")),
                    excerpt(text(fact, "eventId")),
                    excerpt(text(fact, "source"))
                );
            } else if let Some(event) = events.get(id.as_str()) {
                f.explanation += &format!(
                    "\nAuthored event {}: {}",
                    excerpt(id),
                    excerpt(text(event, "title"))
                );
            }
        }
        f.annotation = exact_annotations(doc, f, &mut budget);
    }
    out.limited |= budget.limited || analysis.limited();
    out
}
fn report_text(report: &Report) -> String {
    let mut s = String::from(
        "Provisional questions from saved authored records. No prose was inspected.\n\n",
    );
    for f in &report.findings {
        s += &format!(
            "{} · {}\n{}\nContributors: {}\n",
            f.rule,
            excerpt(&f.title),
            f.explanation,
            f.ids
                .iter()
                .map(|id| excerpt(id))
                .collect::<Vec<_>>()
                .join(", ")
        );
        for note in &f.annotation {
            s += &format!("{note}\n");
        }
        s.push('\n');
    }
    if report.findings.is_empty() {
        s += if report.limited {
            "No conclusive results are shown because checked coverage is incomplete.\n"
        } else {
            "No issues found in the checked records and enabled rules.\n"
        };
    }
    if report.unresolved > 0 {
        s+=&format!("{} checks lacked complete, definite or consistent authored evidence; they remain unresolved.\n",report.unresolved);
    }
    if report.limited {
        s += "Coverage limit reached. Some records or comparisons were not checked.\n";
    }
    s+="Intentional/dismissed annotations are displayed only for an exact rule category and complete matching reference set. They do not remove evidence or become inferred exceptions.\n";
    s
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageKind {
    Checks,
    Lanes,
}
struct ReadPage {
    origin: Rc<Workspace>,
    project: String,
    widget: glib::WeakRef<gtk::Box>,
    kind: PageKind,
    snapshot: RefCell<Option<Value>>,
    status: gtk::Label,
    buffer: gtk::TextBuffer,
    choices: Vec<(String, gtk::CheckButton)>,
    scene: gtk::ComboBoxText,
    loading: Cell<bool>,
}
fn checked_snapshot(snapshot: &Value, project: &str) -> Result<(), String> {
    if snapshot["index"]["project"]["id"] != project {
        return Err("The saved source belongs to a different novel.".into());
    }
    for key in ["planning", "sceneMetadata"] {
        if !snapshot[key].is_null()
            && (!snapshot[key]["revisionId"].is_string() || !snapshot[key]["payload"].is_object())
        {
            return Err(
                "The saved source is incomplete. Refresh without changing stored data.".into(),
            );
        }
    }
    if !snapshot["index"]["scenes"].is_array() {
        return Err("Saved scene metadata is unavailable.".into());
    }
    Ok(())
}
fn source_caption(snapshot: &Value) -> String {
    format!("Saved planning revision: {} · Scene metadata revision: {} · {} current scenes · Authored metadata only",snapshot["planning"]["revisionId"].as_str().unwrap_or("None"),snapshot["sceneMetadata"]["revisionId"].as_str().unwrap_or("None"),rows(&snapshot["index"],"scenes").len())
}
impl ReadPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let mut owner = None;
        let widget = self.widget.upgrade()?;
        desk::with_button_owner(widget.upcast_ref(), || {
            let a = desk::effective(&self.origin);
            if !a.desk.is_closed()
                && *a.pid.borrow() == self.project
                && a.pages.page_num(&widget).is_some()
            {
                owner = Some(a);
            }
        });
        owner
    }
    fn enabled(&self) -> BTreeSet<String> {
        self.choices
            .iter()
            .filter(|(_, button)| button.is_active())
            .map(|(id, _)| id.clone())
            .collect()
    }
    fn render(&self) {
        let saved = self.snapshot.borrow();
        let Some(snapshot) = saved.as_ref() else {
            return;
        };
        let doc = if snapshot["planning"].is_null() {
            crate::tools::initial("planning")
        } else {
            snapshot["planning"]["payload"].clone()
        };
        self.status.set_text(&source_caption(snapshot));
        if self.kind == PageKind::Checks {
            self.buffer
                .set_text(&report_text(&check(&doc, &self.enabled())));
        } else {
            let live = rows(&snapshot["index"], "scenes")
                .iter()
                .filter_map(|s| s["id"].as_str().map(str::to_owned))
                .collect::<HashSet<_>>();
            let report = lanes::project(&doc, &live);
            self.buffer.set_text(&lanes::render(&report));
            let selected = self.scene.active_id();
            self.scene.remove_all();
            let linked = report
                .rows
                .iter()
                .flat_map(|r| r.scenes.iter())
                .collect::<BTreeSet<_>>();
            for s in rows(&snapshot["index"], "scenes") {
                let id = text(s, "id");
                if linked.contains(&id.to_owned()) {
                    self.scene.append(Some(id), &excerpt(text(s, "title")));
                }
            }
            if let Some(id) = selected {
                self.scene.set_active_id(Some(&id));
            }
            if self.scene.active_id().is_none() {
                self.scene.set_active(Some(0));
            }
        }
    }
    fn load(self: &Rc<Self>) {
        if self.loading.replace(true) {
            return;
        }
        let Some(a) = self.owner() else {
            self.loading.set(false);
            return;
        };
        self.snapshot.borrow_mut().take();
        self.buffer.set_text("");
        self.status
            .set_text("Loading atomic saved planning and scene sources…");
        let p = self.clone();
        a.client.rpc(
            "planning_read_snapshot",
            json!({"projectId":self.project}),
            move |reply| {
                if p.owner().is_none() {
                    return;
                }
                p.loading.set(false);
                match reply.and_then(|s| checked_snapshot(&s, &p.project).map(|_| s)) {
                    Ok(s) => {
                        *p.snapshot.borrow_mut() = Some(s);
                        if p.kind == PageKind::Lanes {
                            p.render();
                        } else {
                            p.status.set_text(&format!(
                                "{} · Choose enabled rules, then Check saved records",
                                source_caption(p.snapshot.borrow().as_ref().unwrap())
                            ));
                        }
                    }
                    Err(e) => p.status.set_text(&format!("Saved view unavailable. {e}")),
                }
            },
        );
    }
    fn verify(self: &Rc<Self>, scene: Option<(String, bool)>) {
        if self.loading.replace(true) {
            return;
        }
        let Some(a) = self.owner() else {
            self.loading.set(false);
            return;
        };
        let Some(captured) = self.snapshot.borrow().clone() else {
            self.loading.set(false);
            return;
        };
        let p = self.clone();
        let weak = Rc::downgrade(&a);
        let generation = a.desk.generation.get();
        let enabled = self.enabled();
        self.status.set_text("Rechecking saved sources…");
        a.client.rpc("planning_read_snapshot",json!({"projectId":self.project}),move|reply|{
        p.loading.set(false);let Some(owner)=p.owner()else{return;};let Some(start)=weak.upgrade()else{return;};if !Rc::ptr_eq(&owner,&start)||owner.desk.generation.get()!=generation||owner.desk.busy.get(){p.status.set_text("The page owner or navigation changed. Retry from the current page.");return;}
        let current=match reply.and_then(|s|checked_snapshot(&s,&p.project).map(|_|s)){Ok(s)=>s,Err(e)=>{p.buffer.set_text("");p.status.set_text(&format!("Saved source verification failed. {e}"));return;}};
        if p.enabled()!=enabled{p.buffer.set_text("");p.status.set_text("Rule selection changed while checking. Check saved records again.");return;}
        if current!=captured{p.snapshot.borrow_mut().take();p.buffer.set_text("");p.scene.remove_all();p.status.set_text("Saved sources changed. Refresh and review the new revision before checking or opening a link.");return;}
        if let Some((id,separate))=scene{if rows(&current["index"],"scenes").iter().any(|s|s["id"]==id){owner.open_scene(&id,separate);}}else{p.render();}
    });
    }
}
impl Workspace {
    pub(super) fn continuity_checks(self: &Rc<Self>) {
        self.authored_read_page(PageKind::Checks)
    }
    pub(super) fn occurrence_lanes(self: &Rc<Self>) {
        self.authored_read_page(PageKind::Lanes)
    }
    fn authored_read_page(self: &Rc<Self>, kind: PageKind) {
        let a = desk::effective(self);
        let project = a.pid.borrow().clone();
        if project.is_empty() {
            a.error("Choose a novel before opening saved story tools.");
            return;
        }
        let origin = a.clone();
        a.navigate(move||{
        if origin.desk.is_closed()||*origin.pid.borrow()!=project{return;}let panel=origin.panel(if kind==PageKind::Checks{"Continuity checks"}else{"Occurrence lanes"});let body=panel.content_area();let hint=gtk::Label::new(Some(if kind==PageKind::Checks{"Read-only questions from approved, explicitly authored records. These checks never inspect prose, approve claims, reschedule events or change issues. Disable a rule for an intentional exception; choices apply to this page only."}else{"Main and subplot memberships show the same canonical events. Unknown dates, missing links and contradictions remain visible. Linked scene navigation rechecks the saved source and uses the ordinary draft decision."}));hint.set_line_wrap(true);hint.set_xalign(0.);body.pack_start(&hint,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Vertical,4);body.pack_start(&controls,false,false,0);let choices=if kind==PageKind::Checks{RULES.iter().map(|(id,title)|{let choice=gtk::CheckButton::with_label(title);choice.set_active(true);controls.pack_start(&choice,false,false,0);(id.to_string(),choice)}).collect()}else{Vec::new()};
        let scene=gtk::ComboBoxText::new();scene.set_tooltip_text(Some("Occurrence linked scene"));if kind==PageKind::Lanes{body.pack_start(&scene,false,false,0);}let status=gtk::Label::new(None);status.set_line_wrap(true);status.set_xalign(0.);body.pack_start(&status,false,false,0);let view=gtk::TextView::new();view.set_editable(false);view.set_cursor_visible(false);view.set_wrap_mode(gtk::WrapMode::WordChar);body.pack_start(&scroll(&view),true,true,0);
        let page=Rc::new(ReadPage{origin:origin.clone(),project,widget:panel.widget.downgrade(),kind,snapshot:RefCell::new(None),status,buffer:view.buffer().unwrap(),choices,scene,loading:Cell::new(false)});
        for(_,choice)in &page.choices{let weak=Rc::downgrade(&page);choice.connect_toggled(move |_|if let Some(p)=weak.upgrade(){p.buffer.set_text("");p.status.set_text("Rule selection changed. Check saved records to review this selection.");});}
        let p=page.clone();button(&controls,"Refresh saved source",move||p.load());if kind==PageKind::Checks{let p=page.clone();button(&controls,"Check saved records",move||p.verify(None));}else{for(title,separate)in[("Open linked scene",false),("Open linked scene in new window",true)]{let p=page.clone();button(&controls,title,move||{if let Some(id)=p.scene.active_id(){p.verify(Some((id.to_string(),separate)));}});}}
        panel.show_all();page.load();
    });
    }
}
#[cfg(test)]
#[path = "continuity_tests.rs"]
pub(crate) mod tests;
