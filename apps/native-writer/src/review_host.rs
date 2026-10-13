//! Concrete notebook host. Root declares this as an app child and adds
//! review_decision_pages / translation_preview_pages Vec<Rc<...>> registries.
//! Route menus to review_decisions, translation_preview and run_local_review;
//! replace the old review body with a run_local_review forwarder. Add both page
//! types to Draft save/discard/dirty/composition/metadata-busy and ownership
//! transfer. Generation is deliberately NOT Draft busy state. On accepted tab
//! release call release_review_page before removing its registry entry; on
//! accepted project/window release call cancel_review_generations. Do not cancel
//! on detach/transfer: move registry ownership of the same page instead.
use super::review_decisions::ReviewDecisionsPage;
use super::translation_preview::TranslationPreviewPage;
use super::*;
use crate::process::Reply;
use std::rc::Weak;
use std::time::Instant;

thread_local! {
    static REVIEW_RUNS: RefCell<HashMap<gtk::Widget, Rc<ReviewRun>>> = RefCell::new(HashMap::new());
}

fn live(a: &Workspace, project: &str) -> bool {
    !a.desk.is_closed() && !project.is_empty() && *a.pid.borrow() == project
}
fn pane_ready(p: &EditorPane) -> bool {
    p.open.get() && !p.dirty.get() && !p.composing.get() && !p.saving.get() && p.pending.get() == 0
}
fn exact_pane(p: &EditorPane, saved: &Value) -> bool {
    pane_ready(p)
        && saved["id"] == p.scene_id
        && saved["revisionId"] == *p.parent.borrow()
        && saved["scene"]["document"] == p.frame.borrow()["document"]
}
fn endpoint_is_local(endpoint: &str) -> bool {
    if endpoint.len() > 200 {
        return false;
    }
    let rest = endpoint
        .strip_prefix("http://127.0.0.1")
        .or_else(|| endpoint.strip_prefix("http://[::1]"));
    let Some(rest) = rest else {
        return false;
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    rest.is_empty()
        || rest.strip_prefix(':').is_some_and(|port| {
            !port.is_empty()
                && port.bytes().all(|c| c.is_ascii_digit())
                && port.parse::<u16>().is_ok_and(|n| n != 0)
        })
}
fn configuration(snapshot: &Value, provider: &str) -> Result<Value, String> {
    if !matches!(provider, "languagetool" | "llm") {
        return Err("Choose a supported local provider.".into());
    }
    let payload = if snapshot.is_null() {
        crate::tools::initial("settings")
    } else {
        snapshot["payload"].clone()
    };
    let assistance = payload
        .get("assistance")
        .cloned()
        .unwrap_or_else(|| crate::tools::initial("settings")["assistance"].clone());
    let config = assistance["providers"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["kind"] == provider))
        .cloned()
        .ok_or("Configure this local provider in Review settings first.")?;
    if !config["endpoint"].as_str().is_some_and(endpoint_is_local) {
        return Err("Review providers require literal loopback HTTP without credentials, paths or redirects.".into());
    }
    if config.get("model").is_some_and(|v| {
        !v.is_null() && !v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 200)
    }) {
        return Err("Invalid configured local model identifier.".into());
    }
    Ok(config)
}
fn source_ref(source: &Value) -> Value {
    json!({"sceneId":source["sceneId"],"revisionId":source["revisionId"],"hash":source["hash"]})
}
fn frozen_request(
    project: &str,
    scene: &str,
    saved: &Value,
    hash: Value,
    provider: &str,
    language: &str,
    id: &str,
    created_at: &str,
) -> Result<Value, String> {
    if project.is_empty()
        || saved["id"] != scene
        || !saved["revisionId"].as_str().is_some_and(|s| !s.is_empty())
    {
        return Err(
            "The saved review source changed identity or has no committed revision.".into(),
        );
    }
    if !matches!(provider, "languagetool" | "llm")
        || !LANGUAGES.iter().any(|(code, _)| *code == language)
    {
        return Err("Choose a supported review provider and language.".into());
    }
    if !hash.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err("Saved review source requires its exact SHA-256 hash.".into());
    }
    let text = novel_storage::plain_text(&saved["scene"]["document"]);
    if text.trim().is_empty() || text.encode_utf16().count() > 50_000 {
        return Err("Local review requires nonempty saved text up to 50,000 UTF-16 units. Editing and saving remain available.".into());
    }
    Ok(
        json!({"schemaVersion":1,"id":id,"idempotencyKey":uuid::Uuid::new_v4().to_string(),
        "projectId":project,"provider":provider,"operation":if provider=="llm"{"wording"}else{"grammar"},
        "language":language,"sources":[{"sceneId":scene,"revisionId":saved["revisionId"],"hash":hash,
            "language":language,"text":text,"kind":"scene"}],"contextBudget":4096,"timeoutMs":120000,"createdAt":created_at}),
    )
}
// Backend snapshot owns the source projection and hashes; no local fact inference
// or filtering may replace it. Exactly one scene plus one planning source is sent.
fn consistency_request(
    project: &str,
    scene: &str,
    context: &Value,
    language: &str,
    id: &str,
    created: &str,
) -> Result<Value, String> {
    let sources = context["sources"]
        .as_array()
        .filter(|s| s.len() == 2)
        .ok_or("Consistency context requires exactly one scene and one saved fact projection.")?;
    if sources[0]["kind"] != "scene"
        || sources[0]["sceneId"] != scene
        || sources[0]["revisionId"] != context["scene"]["revisionId"]
        || sources[0]["text"].as_str()
            != Some(novel_storage::plain_text(&context["scene"]["scene"]["document"]).as_str())
        || sources[1]["kind"] != "facts"
        || sources[1]["sceneId"] != format!("planning:{project}")
        || context["scope"]["includedSceneId"] != scene
        || context["scope"]["timingIncluded"] != false
        || !context["scope"]["includedFactIds"]
            .as_array()
            .is_some_and(|ids| ids.len() <= 1000)
    {
        return Err(
            "The authoritative saved scene/fact context or included scope is invalid.".into(),
        );
    }
    let facts_text = sources[1]["text"]
        .as_str()
        .ok_or("Missing saved canonical projection.")?;
    let facts: Value =
        serde_json::from_str(facts_text).map_err(|_| "Invalid saved canonical projection.")?;
    let ids = facts["facts"]
        .as_array()
        .filter(|f| f.len() <= 1000)
        .ok_or("Invalid confirmed fact scope.")?
        .iter()
        .map(|f| f["id"].clone())
        .collect::<Vec<_>>();
    if facts["schemaVersion"] != 1
        || facts_text.len() > 30000
        || context["scope"]["includedFactIds"] != json!(ids)
    {
        return Err("Included facts differ from the authoritative canonical projection.".into());
    }
    let mut total = 0;
    for source in sources {
        let text = source["text"]
            .as_str()
            .ok_or("Missing exact saved context text.")?;
        total += text.encode_utf16().count();
        if !source["revisionId"].as_str().is_some_and(|s| !s.is_empty())
            || !source["hash"].as_str().is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err("Saved context requires exact revision/hash identities.".into());
        }
    }
    if total > 50000 {
        return Err(
            "Complete consistency context exceeds the review limit. No source was truncated."
                .into(),
        );
    }
    let mut request = frozen_request(
        project,
        scene,
        &context["scene"],
        sources[0]["hash"].clone(),
        "llm",
        language,
        id,
        created,
    )?;
    request["operation"] = json!("consistency");
    request["scope"] = context["scope"].clone();
    request["sources"] = json!(sources
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s["language"] = json!(if s["kind"] == "facts" {
                "und"
            } else {
                language
            });
            s
        })
        .collect::<Vec<_>>());
    Ok(request)
}
fn confirm_consistency(a: &Workspace, context: &Value) -> bool {
    let dialog = gtk::Dialog::with_buttons(
        Some("Confirm one-scene consistency scope"),
        Some(&a.window),
        gtk::DialogFlags::MODAL,
        &[
            ("Cancel", gtk::ResponseType::Cancel),
            ("Review this saved scope", gtk::ResponseType::Accept),
        ],
    );
    dialog.set_default_size(900, 600);
    dialog.set_accept_focus(a.window.accepts_focus());
    dialog.set_focus_on_map(a.window.accepts_focus());
    let scope = &context["scope"];
    let summary=gtk::Label::new(Some(&format!("Included: this one saved scene and {} explicitly confirmed canonical facts. Excluded: {} other scenes and {} unconfirmed, belief, rumor, proposal or unknown rows. Whole-book review is excluded. Event timing is unknown or unprovided: eventId/untilEventId scopes remain attached to facts and must not be treated as universal truths or contradictions without timing evidence. Findings remain provisional questions; accepting a review note never changes canon or writing.",scope["includedFactIds"].as_array().map_or(0,Vec::len),scope["excludedSceneCount"],scope["excludedFacts"].as_array().map_or(0,Vec::len))));
    summary.set_line_wrap(true);
    summary.set_xalign(0.);
    dialog.content_area().pack_start(&summary, false, false, 8);
    let panes = gtk::Paned::new(gtk::Orientation::Horizontal);
    let facts: Value = serde_json::from_str(context["sources"][1]["text"].as_str().unwrap_or(""))
        .unwrap_or(Value::Null);
    let text = facts["facts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            let name = facts["records"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["id"] == f["subjectId"])
                .and_then(|r| r["name"].as_str())
                .unwrap_or("Subject");
            format!(
                "{} · {}: {}\nAuthored temporal scope: eventId={}, untilEventId={} (timing unknown or unprovided)",
                name,
                f["property"].as_str().unwrap_or(""),
                f["value"].as_str().unwrap_or(""),
                f["eventId"],
                f["untilEventId"]
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let exclusions = scope["excludedFacts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            format!(
                "{} · {}",
                f["id"].as_str().unwrap_or(""),
                f["reason"].as_str().unwrap_or("Excluded")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    for (title, text, left) in [
        (
            "Exact saved scene",
            context["sources"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
            true,
        ),
        (
            "Confirmed facts and excluded rows",
            format!("{text}\n\nExcluded fact rows:\n{exclusions}"),
            false,
        ),
    ] {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        column.pack_start(&gtk::Label::new(Some(title)), false, false, 0);
        let view = gtk::TextView::new();
        view.set_editable(false);
        view.set_cursor_visible(false);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.buffer().unwrap().set_text(&text);
        column.pack_start(&scroll(&view), true, true, 0);
        if left {
            panes.pack1(&column, true, false);
        } else {
            panes.pack2(&column, true, false);
        }
    }
    dialog.content_area().pack_start(&panes, true, true, 8);
    dialog.show_all();
    let accepted = dialog.run() == gtk::ResponseType::Accept;
    dialog.close();
    accepted
}
fn result_within_scope(result: &Value, request: &Value) -> Result<(), String> {
    let sources = request["sources"]
        .as_array()
        .ok_or("Missing captured review scope.")?;
    let same = |source: &Value| {
        sources
            .iter()
            .any(|captured| source_ref(captured) == source_ref(source))
    };
    let suggestions = result["suggestions"]
        .as_array()
        .filter(|items| items.len() <= 100)
        .ok_or("The local provider returned an invalid suggestion list.")?;
    for suggestion in suggestions {
        super::review_decisions::validate_provider_evidence(suggestion, &request["sources"])?;
        if request["operation"] == "consistency" {
            let expected = sources.iter().map(source_ref).collect::<Vec<_>>();
            let actual = suggestion["sources"]
                .as_array()
                .map(|s| s.iter().map(source_ref).collect::<Vec<_>>());
            if suggestion["action"]["kind"] != "question"
                || actual.as_ref() != Some(&expected)
                || !suggestion["evidence"].as_array().is_some_and(|anchors| {
                    anchors
                        .iter()
                        .any(|a| a["sceneId"] == sources[0]["sceneId"])
                })
            {
                return Err("Consistency output must be a provisional question with manuscript evidence and the complete saved scene/fact basis.".into());
            }
        }
        if suggestion["status"] != "pending"
            || !suggestion["sources"]
                .as_array()
                .is_some_and(|items| !items.is_empty() && items.iter().all(same))
            || !suggestion["evidence"]
                .as_array()
                .is_some_and(|items| items.iter().all(same))
        {
            return Err("Local results must remain pending proposals within the exact captured review scope.".into());
        }
    }
    Ok(())
}

/// Original evidence/scope must be checked before author exceptions are applied.
/// Uses root's separately-owned exact quote/language proofreading-only helper.
fn filter_validated_result(
    result: &Value,
    settings: &Value,
    language: &str,
) -> Result<Value, String> {
    let retained = super::proofreading_exceptions::filter(
        result["suggestions"]
            .as_array()
            .ok_or("Invalid validated review result.")?,
        settings,
        language,
    )?;
    let mut next = result.clone();
    next["suggestions"] = json!(retained);
    Ok(next)
}

fn checked_provider_result(
    result: &Value,
    request: &Value,
    settings: &Value,
) -> Result<Value, String> {
    result_within_scope(result, request)?;
    filter_validated_result(result, settings, request["language"].as_str().unwrap_or(""))
}

struct ReviewRun {
    page: Weak<ReviewDecisionsPage>,
    pane: RefCell<Weak<EditorPane>>,
    widget: gtk::Widget,
    project: String,
    id: String,
    client: Process,
    active: Cell<bool>,
    request: RefCell<Value>,
    result: RefCell<Option<Value>>,
    binding: Cell<bool>,
    operation: String,
    settings: RefCell<Value>,
}
impl ReviewRun {
    fn mounted(&self) -> Option<(Rc<Workspace>, Rc<ReviewDecisionsPage>)> {
        let page = self.page.upgrade()?;
        let a = desk::workspace_for_widget(&self.widget)?;
        if !live(&a, &self.project)
            || a.pages.page_num(&self.widget).is_none()
            || !a
                .review_decision_pages
                .borrow()
                .iter()
                .any(|p| Rc::ptr_eq(p, &page))
        {
            return None;
        }
        Some((a, page))
    }
    /// Widget ownership and registry membership are checked per callback; no
    /// async reply presents a window or remounts a page in its origin window.
    fn owner(&self) -> Option<(Rc<Workspace>, Rc<ReviewDecisionsPage>)> {
        if !self.active.get()
            || !REVIEW_RUNS.with(|runs| {
                runs.borrow()
                    .get(&self.widget)
                    .is_some_and(|run| run.id == self.id)
            })
        {
            return None;
        }
        self.mounted()
    }
    fn retire(&self) {
        self.active.set(false);
        REVIEW_RUNS.with(|runs| {
            let current = runs
                .borrow()
                .get(&self.widget)
                .is_some_and(|run| run.id == self.id);
            if current {
                runs.borrow_mut().remove(&self.widget);
            }
        });
    }
    fn cancel(&self, message: Option<&str>) {
        let mounted = self.mounted();
        if !self.active.replace(false) {
            return;
        }
        self.result.borrow_mut().take();
        self.retire();
        self.client.rpc(
            "provider_cancel",
            json!({"jobId":self.id,"runToken":format!("{}:1",self.id)}),
            |_| {},
        );
        if let (Some((_, page)), Some(message)) = (mounted, message) {
            page.status.set_text(message);
        }
    }
    fn fail(&self, error: &str) {
        if let Some((_, page)) = self.owner() {
            page.status.set_text(error);
        }
        self.cancel(None);
    }
    fn wait_loaded(self: &Rc<Self>, provider: String, language: String, start: Instant) {
        let Some((_, page)) = self.owner() else {
            self.cancel(None);
            return;
        };
        if page.loaded.get() && !page.saving.get() && !page.dirty.get() && !page.composing.get() {
            self.prepare(provider, language);
            return;
        }
        if start.elapsed() > Duration::from_secs(15) {
            self.fail("Review records did not become ready. No provider request was sent; editing and saving remain available.");
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(30), move || {
            if let Some(run) = weak.upgrade() {
                run.wait_loaded(provider, language, start);
            }
        });
    }
    fn prepare(self: &Rc<Self>, provider: String, language: String) {
        let Some((_, page)) = self.owner() else {
            self.cancel(None);
            return;
        };
        page.status
            .set_text("Reading saved review settings and exact source…");
        let run = self.clone();
        self.client.rpc("load_project_data_unambiguous",json!({"projectId":self.project,"kind":"settings"}),move|reply|{
            if run.owner().is_none(){run.cancel(None);return;}
            let settings=match reply{Ok(v)=>v,Err(e)=>{run.fail(&e);return;}};
            let config=match configuration(&settings,&provider){Ok(v)=>v,Err(e)=>{run.fail(&e);return;}};
            *run.settings.borrow_mut()=if settings.is_null(){crate::tools::initial("settings")}else{settings["payload"].clone()};
            let Some(pane)=run.pane.borrow().upgrade()else{run.fail("The captured writing session closed before review. Open the scene and request review again.");return;};
            if !pane_ready(&pane){run.fail("Save the scene and finish pending input before requesting a review. No provider request was sent.");return;}
            if run.operation=="consistency" {run.prepare_consistency(pane,config,language);return;}
            let out=run.clone();run.client.rpc("load_project_scene",json!({"projectId":run.project,"sceneId":pane.scene_id}),move|reply|{
                if out.owner().is_none(){out.cancel(None);return;}
                let saved=match reply{Ok(v)if exact_pane(&pane,&v)=>v,Ok(_)=>{out.fail("The saved scene or current writing changed during review capture. No provider request was sent.");return;},Err(e)=>{out.fail(&e);return;}};
                let run=out.clone();out.client.rpc("text_hash",json!({"text":novel_storage::plain_text(&saved["scene"]["document"])}),move|reply|{
                    if run.owner().is_none(){run.cancel(None);return;}
                    let hash=match reply{Ok(v)=>v,Err(e)=>{run.fail(&e);return;}};
                    let created=match glib::DateTime::now_utc().and_then(|time|time.format("%Y-%m-%dT%H:%M:%SZ")){Ok(v)=>v,Err(_)=>{run.fail("Could not timestamp the explicit review request.");return;}};
                    let request=match frozen_request(&run.project,&pane.scene_id,&saved,hash,&provider,&language,&run.id,&created){Ok(v)=>v,Err(e)=>{run.fail(&e);return;}};
                    let refs=json!(request["sources"].as_array().unwrap().iter().map(source_ref).collect::<Vec<_>>());
                    let out=run.clone();run.client.rpc("review_sources_check",json!({"projectId":run.project,"sources":refs}),move|reply|{
                        if out.owner().is_none(){out.cancel(None);return;}
                        match reply{Ok(v)if v["sources"]==refs=>(),Ok(_)=>{out.fail("The atomic review source check did not match the captured scene. No provider request was sent.");return;},Err(e)=>{out.fail(&format!("Saved review sources could not be checked. No provider request was sent. {e}"));return;}}
                        if !exact_pane(&pane,&saved){out.fail("Writing changed before the provider request. Save and explicitly request a new review.");return;}
                        if let Some((_,page))=out.owner(){page.bind_scene(&pane);page.status.set_text("Reviewing the exact saved source. Editing and manual Save remain available; cancel to discard late results.");}
                        *out.request.borrow_mut()=request.clone();out.dispatch(request,config);
                    });
                });
            });
        });
    }
    fn prepare_consistency(self: &Rc<Self>, pane: Rc<EditorPane>, config: Value, language: String) {
        let run = self.clone();
        self.client.rpc("review_context_snapshot",json!({"projectId":self.project,"sceneId":pane.scene_id}),move|reply|{
            let Some((a,_))=run.owner()else{run.cancel(None);return;};
            let context=match reply{Ok(v)if exact_pane(&pane,&v["scene"])=>v,Ok(_)=>{run.fail("Saved scene changed during consistency capture. No provider request was sent.");return;},Err(e)=>{run.fail(&e);return;}};
            let created=match glib::DateTime::now_utc().and_then(|t|t.format("%Y-%m-%dT%H:%M:%SZ")){Ok(v)=>v,Err(_)=>{run.fail("Could not timestamp review.");return;}};
            let request=match consistency_request(&run.project,&pane.scene_id,&context,&language,&run.id,&created){Ok(v)=>v,Err(e)=>{run.fail(&e);return;}};
            if !confirm_consistency(&a,&context){run.cancel(Some("Consistency review cancelled; no provider request was sent."));return;}
            if run.owner().is_none(){run.cancel(None);return;}
            let refs=json!(request["sources"].as_array().unwrap().iter().map(source_ref).collect::<Vec<_>>());let out=run.clone();
            run.client.rpc("review_context_check",json!({"projectId":run.project,"sources":refs}),move|reply|{
                if out.owner().is_none(){out.cancel(None);return;}
                match reply{Ok(v)if v["sources"]==refs=>(),Ok(_)=>{out.fail("The consistency basis changed; no provider request was sent.");return;},Err(e)=>{out.fail(&e);return;}}
                if !exact_pane(&pane,&context["scene"]){out.fail("Writing changed after scope confirmation. No provider request was sent.");return;}
                if let Some((_,page))=out.owner(){page.bind_scene(&pane);page.status.set_text("Reviewing one saved scene with the explicitly confirmed fact scope. Findings remain provisional questions; editing and manual Save remain available.");}
                *out.request.borrow_mut()=request.clone();out.dispatch(request,config);
            });
        });
    }
    fn dispatch(self: &Rc<Self>, request: Value, config: Value) {
        if self.owner().is_none() {
            self.cancel(None);
            return;
        }
        #[cfg(test)]
        {
            // A GTK fixture can exercise real source/storage workers but never
            // contact a provider. Missing injected data fails closed in tests.
            let fixture = MOCK_PROVIDER_REPLY.with(|cell| cell.borrow_mut().take());
            let run = self.clone();
            match fixture {
                Some((reply, delay)) => {
                    glib::timeout_add_local_once(delay, move || run.receive(reply));
                }
                None => run.receive(Err(
                    "Synthetic provider reply missing; tests never invoke inference.".into(),
                )),
            }
            let _ = (request, config);
        }
        #[cfg(not(test))]
        {
            let run = self.clone();
            self.client.rpc("provider_run",json!({"request":request,"runToken":format!("{}:1",self.id),"endpoint":config["endpoint"],"model":config["model"]}),move|reply|run.receive(reply));
        }
    }
    fn receive(self: &Rc<Self>, reply: Reply) {
        if self.owner().is_none() {
            self.cancel(None);
            return;
        }
        let result = match reply.and_then(|result| {
            let request = self.request.borrow();
            checked_provider_result(&result, &request, &self.settings.borrow())
        }) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                return;
            }
        };
        let mut result = result;
        if self.request.borrow()["operation"] == "consistency" {
            let request = self.request.borrow();
            for suggestion in result["suggestions"].as_array_mut().unwrap() {
                suggestion["reviewOperation"] = json!("consistency");
                suggestion["reviewScope"] = request["scope"].clone();
                for key in ["sources", "evidence"] {
                    for source in suggestion[key].as_array_mut().unwrap() {
                        let kind = request["sources"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|s| s["sceneId"] == source["sceneId"])
                            .unwrap()["kind"]
                            .clone();
                        source["kind"] = kind;
                    }
                }
            }
        }
        *self.result.borrow_mut() = Some(result);
        self.deliver();
    }
    fn deliver(self: &Rc<Self>) {
        let Some((a, page)) = self.owner() else {
            self.cancel(None);
            return;
        };
        if self.result.borrow().is_none() {
            return;
        }
        if !page.loaded.get() || page.dirty.get() || page.saving.get() || page.composing.get() {
            page.status.set_text("Local review result retained here. Finish composition and save or discard pending review records, then choose Receive review result.");
            return;
        }
        // Reuse normal notebook-owned sessions. A closed origin may still have
        // live Rc references in callbacks, so Weak upgrade alone is insufficient.
        let pane = a
            .panes
            .borrow()
            .get(
                self.request.borrow()["sources"][0]["sceneId"]
                    .as_str()
                    .unwrap_or(""),
            )
            .cloned()
            .or_else(|| {
                self.pane.borrow().upgrade().filter(|pane| {
                    desk::workspace_for_widget(pane.widget.upcast_ref()).is_some_and(|owner| {
                        live(&owner, &self.project)
                            && owner
                                .panes
                                .borrow()
                                .get(&pane.scene_id)
                                .is_some_and(|p| Rc::ptr_eq(p, pane))
                    })
                })
            });
        let Some(pane) = pane else {
            if self.binding.replace(true) {
                return;
            }
            let Some(scene) = self.request.borrow()["sources"][0]["sceneId"]
                .as_str()
                .map(str::to_owned)
            else {
                self.binding.set(false);
                self.fail("Captured review scene is unavailable.");
                return;
            };
            let run = self.clone();
            a.ensure_pane(&scene,move|reply|{
                run.binding.set(false);
                if run.owner().is_none(){run.cancel(None);return;}
                match reply{Ok(pane)=>{*run.pane.borrow_mut()=Rc::downgrade(&pane);run.deliver();},Err(e)=>{if let Some((_,page))=run.owner(){page.status.set_text(&format!("Review result retained. Open the reviewed scene and Receive review result. {e}"));}}}
            });
            return;
        };
        page.bind_scene(&pane);
        let result = self.result.borrow_mut().take().unwrap();
        if let Err(e) = page.append_result(&a, &result) {
            // The exact validated result and source-kind manifest are retained.
            *self.result.borrow_mut() = Some(result);
            page.status.set_text(&format!(
                "Review result retained; proposals were not saved. {e}"
            ));
            return;
        }
        self.retire(); // metadata saving is tracked by the page, never by inference
    }
}

fn attach_review_controls(page: &Rc<ReviewDecisionsPage>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let widget = page.widget.downgrade();
    let target = widget.clone();
    button(&row, "Cancel local review", move || {
        let Some(target) = target.upgrade() else {
            return;
        };
        let run = REVIEW_RUNS.with(|runs| {
            runs.borrow()
                .get(target.upcast_ref::<gtk::Widget>())
                .cloned()
        });
        if let Some(run) = run {
            run.cancel(Some(
                "Local review cancelled. Retained proposals and writing are unchanged.",
            ));
        }
    });
    button(&row, "Receive review result", move || {
        let Some(widget) = widget.upgrade() else {
            return;
        };
        let run = REVIEW_RUNS.with(|runs| {
            runs.borrow()
                .get(widget.upcast_ref::<gtk::Widget>())
                .cloned()
        });
        if let Some(run) = run {
            run.deliver();
        }
    });
    page.widget.pack_start(&row, false, false, 0);
}
fn mount_review(a: &Rc<Workspace>, pane: &Rc<EditorPane>) -> Option<Rc<ReviewDecisionsPage>> {
    let project = a.pid.borrow().clone();
    if !live(a, &project) {
        return None;
    }
    let page = ReviewDecisionsPage::new(project);
    page.bind_scene(pane);
    attach_review_controls(&page);
    a.review_decision_pages.borrow_mut().push(page.clone());
    a.install(&page.widget, "Local review", false);
    if a.pages.page_num(&page.widget).is_none() {
        a.review_decision_pages
            .borrow_mut()
            .retain(|p| !Rc::ptr_eq(p, &page));
        return None;
    }
    page.load(a);
    Some(page)
}
fn begin_review_operation(
    a: &Rc<Workspace>,
    pane: &Rc<EditorPane>,
    provider: String,
    language: String,
    operation: String,
) -> Option<Rc<ReviewRun>> {
    let page = mount_review(a, pane)?;
    let run = Rc::new(ReviewRun {
        page: Rc::downgrade(&page),
        pane: RefCell::new(Rc::downgrade(pane)),
        widget: page.widget.clone().upcast(),
        project: a.pid.borrow().clone(),
        id: uuid::Uuid::new_v4().to_string(),
        client: a.client.clone(),
        active: Cell::new(true),
        request: RefCell::new(Value::Null),
        result: RefCell::new(None),
        binding: Cell::new(false),
        operation,
        settings: RefCell::new(crate::tools::initial("settings")),
    });
    REVIEW_RUNS.with(|runs| runs.borrow_mut().insert(run.widget.clone(), run.clone()));
    run.wait_loaded(provider, language, Instant::now());
    Some(run)
}
#[cfg(test)]
fn begin_review(
    a: &Rc<Workspace>,
    pane: &Rc<EditorPane>,
    provider: String,
    language: String,
) -> Option<Rc<ReviewRun>> {
    let operation = if provider == "llm" {
        "wording"
    } else {
        "grammar"
    };
    begin_review_operation(a, pane, provider, language, operation.into())
}
fn pending_review(project: &str) -> bool {
    let runs = REVIEW_RUNS.with(|runs| runs.borrow().values().cloned().collect::<Vec<_>>());
    for run in &runs {
        if run.owner().is_none() {
            run.cancel(None);
        }
    }
    runs.into_iter()
        .any(|run| run.active.get() && run.project == project)
}
impl Workspace {
    pub(super) fn review_decisions(self: &Rc<Self>) {
        let a = desk::effective(self);
        let project = a.pid.borrow().clone();
        let Some(pane) = a.context_pane() else {
            a.error("Open a scene before viewing review decisions.");
            return;
        };
        if !live(&a, &project)
            || !pane.open.get()
            || pane.composing.get()
            || pane.saving.get()
            || pane.pending.get() != 0
        {
            a.error("Finish the current scene operation before opening review decisions.");
            return;
        }
        let out = a.clone();
        a.navigate(move || {
            if !live(&out, &project) {
                return;
            }
            if !pane_ready(&pane) {
                out.error("Save the current scene before opening its review decisions.");
                return;
            }
            mount_review(&out, &pane);
        });
    }
    pub(super) fn translation_preview(self: &Rc<Self>) {
        let a = desk::effective(self);
        let project = a.pid.borrow().clone();
        let Some(pane) = a.context_pane() else {
            a.error("Open a scene before viewing translations.");
            return;
        };
        if !live(&a, &project)
            || !pane.open.get()
            || pane.composing.get()
            || pane.saving.get()
            || pane.pending.get() != 0
        {
            a.error("Finish the current scene operation before opening translations.");
            return;
        }
        let out = a.clone();
        a.navigate(move || {
            if !live(&out, &project) {
                return;
            }
            if !pane_ready(&pane) {
                out.error("Save the current scene before opening translations.");
                return;
            }
            let page =
                TranslationPreviewPage::new(project, pane.scene_id.clone(), out.client.clone());
            out.translation_preview_pages
                .borrow_mut()
                .push(page.clone());
            out.install(&page.widget, "Translations", false);
            if out.pages.page_num(&page.widget).is_none() {
                out.translation_preview_pages
                    .borrow_mut()
                    .retain(|p| !Rc::ptr_eq(p, &page));
                return;
            }
            page.load(&out);
        });
    }
    pub(super) fn run_local_review(self: &Rc<Self>) {
        let a = desk::effective(self);
        let project = a.pid.borrow().clone();
        let Some(pane) = a.context_pane() else {
            a.error("Open a scene before requesting local review.");
            return;
        };
        if !live(&a, &project)
            || !pane.open.get()
            || pane.composing.get()
            || pane.saving.get()
            || pane.pending.get() != 0
        {
            a.error("Finish the current scene operation before requesting review.");
            return;
        }
        if pending_review(&project) {
            a.error(
                "Cancel or receive this novel's pending local review before requesting another.",
            );
            return;
        }
        let Some(provider) = choose_option(
            &a.window,
            "Review with",
            &[
                ("languagetool", "Local grammar checker"),
                ("llm", "Local writing model"),
            ],
            "languagetool",
        ) else {
            return;
        };
        let operation = if provider == "llm" {
            let Some(operation) = choose_option(
                &a.window,
                "Local review operation",
                &[
                    ("wording", "Wording"),
                    ("consistency", "One-scene consistency"),
                ],
                "wording",
            ) else {
                return;
            };
            if !matches!(operation.as_str(), "wording" | "consistency") {
                a.error("Choose Wording or One-scene consistency.");
                return;
            }
            operation
        } else {
            "grammar".into()
        };
        let Some(language) = choose_option(&a.window, "Review language", LANGUAGES, "en") else {
            return;
        };
        if !matches!(provider.as_str(), "languagetool" | "llm")
            || !LANGUAGES.iter().any(|(code, _)| *code == language)
        {
            a.error("Choose a supported local provider and review language.");
            return;
        }
        let out = a.clone();
        a.drain(move|reply|{
            if !live(&out,&project){return;}if let Err(e)=reply{out.error(&e);return;}
            if !pane_ready(&pane)||!out.context_pane().is_some_and(|current|Rc::ptr_eq(&current,&pane)){out.error("The scene context changed during manual drain. Request review again from its writing page.");return;}
            if pending_review(&project){out.error("This novel already has a pending local review.");return;}
            begin_review_operation(&out,&pane,provider,language,operation);
        });
    }
    /// Root calls only after accepting release, before registry removal. A
    /// cancelled close/navigation keeps the generation alive; detach transfers it.
    pub(super) fn release_review_page(&self, widget: &gtk::Widget) {
        let run = REVIEW_RUNS.with(|runs| runs.borrow().get(widget).cloned());
        if let Some(run) = run {
            run.cancel(None);
        }
        let page = self
            .translation_preview_pages
            .borrow()
            .iter()
            .find(|p| p.widget.upcast_ref::<gtk::Widget>() == widget)
            .cloned();
        if let Some(page) = page {
            page.cancel_generation();
        }
    }
    pub(super) fn cancel_review_generations(&self) {
        let widgets = self
            .review_decision_pages
            .borrow()
            .iter()
            .map(|p| p.widget.clone().upcast::<gtk::Widget>())
            .chain(
                self.translation_preview_pages
                    .borrow()
                    .iter()
                    .map(|p| p.widget.clone().upcast::<gtk::Widget>()),
            )
            .collect::<Vec<_>>();
        for widget in widgets {
            self.release_review_page(&widget);
        }
    }
}
#[cfg(test)]
thread_local! {static MOCK_PROVIDER_REPLY: RefCell<Option<(Reply,Duration)>> = const { RefCell::new(None) };}
#[cfg(test)]
#[path = "review_host_tests.rs"]
pub(super) mod tests;
