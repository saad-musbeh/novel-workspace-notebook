//! Editable translation drafts from explicit captured saved sources. Generation
//! is optional; notebook drain must call cancel_generation, never await inference.
//! Settings/drafts load through the unambiguous project-data read. Checked saves
//! use empty source dependencies: old translation sources remain valid historical
//! captures, while current translation metadata still requires one exact head.
use super::*;

fn language(value: &str) -> bool {
    LANGUAGES.iter().any(|(code, _)| *code == value)
}
fn source_draft(saved: &Value, source: &str, target: &str, id: &str) -> Result<Value, String> {
    if !language(source) || !language(target) || source == target {
        return Err("Choose two different supported languages.".into());
    }
    let scene = saved["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Missing saved source scene.")?;
    let revision = saved["revisionId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Missing saved source revision.")?;
    let text = novel_storage::plain_text(&saved["scene"]["document"]);
    if text.trim().is_empty() || text.encode_utf16().count() > 500000 {
        return Err(
            "A translation needs nonempty saved source text up to 500,000 UTF-16 units.".into(),
        );
    }
    Ok(
        json!({"id":id,"sceneId":scene,"sceneRevisionId":revision,"sourceLanguage":source,"targetLanguage":target,"sourceText":text,"text":""}),
    )
}
fn append_draft(document: &Value, draft: Value) -> Result<Value, String> {
    let mut next = document.clone();
    let drafts = next["drafts"]
        .as_array_mut()
        .ok_or("Invalid retained translations.")?;
    if next_schema(document) != Some(1)
        || drafts.len() >= 1000
        || drafts.iter().any(|d| d["id"] == draft["id"])
    {
        return Err("Translation draft count or identity limit reached.".into());
    }
    drafts.push(draft);
    if serde_json::to_vec(&next)
        .map_err(|_| "Invalid translation document.")?
        .len()
        > 8_000_000
    {
        return Err("Translations exceed 8 MB.".into());
    }
    Ok(next)
}
fn next_schema(document: &Value) -> Option<u64> {
    document["schemaVersion"].as_u64()
}
fn edit_draft(document: &Value, id: &str, text: &str) -> Result<Value, String> {
    if text.encode_utf16().count() > 500000 {
        return Err("Translation text exceeds 500,000 UTF-16 units.".into());
    }
    let mut next = document.clone();
    let draft = next["drafts"]
        .as_array_mut()
        .and_then(|d| d.iter_mut().find(|d| d["id"] == id))
        .ok_or("Choose a saved translation draft.")?;
    draft["text"] = json!(text);
    if serde_json::to_vec(&next)
        .map_err(|_| "Invalid translations.")?
        .len()
        > 8_000_000
    {
        return Err("Translations exceed 8 MB.".into());
    }
    Ok(next)
}
/// A token covers capture as well as inference. Cancel removes it synchronously,
/// so an out-of-order source reply cannot even start the optional provider.
#[derive(Default)]
struct Generation {
    active: Option<String>,
}
impl Generation {
    fn start(&mut self, token: String) -> Result<(), String> {
        if self.active.is_some() {
            return Err("A translation is already being generated.".into());
        }
        self.active = Some(token);
        Ok(())
    }
    fn current(&self, token: &str) -> bool {
        self.active.as_deref() == Some(token)
    }
    fn cancel(&mut self) -> Option<String> {
        self.active.take()
    }
    fn finish(&mut self, token: &str, result: Value) -> Result<Option<String>, String> {
        if !self.current(token) {
            return Ok(None);
        }
        self.active = None;
        let text = result["text"]
            .as_str()
            .ok_or("The local model returned no translation text.")?;
        if text.trim().is_empty() || text.encode_utf16().count() > 50000 {
            return Err("The local model returned empty or excessive translation text.".into());
        }
        Ok(Some(text.to_owned()))
    }
}

pub(super) struct TranslationPreviewPage {
    pub widget: gtk::Box,
    pub text: gtk::TextView,
    pub original: gtk::TextView,
    pub drafts: gtk::ComboBoxText,
    pub source_language: gtk::ComboBoxText,
    pub target_language: gtk::ComboBoxText,
    pub status: gtk::Label,
    pub dirty: Cell<bool>,
    pub saving: Cell<bool>,
    pub composing: Cell<bool>,
    pub loaded: Cell<bool>,
    project_id: String,
    scene_id: String,
    client: Process,
    value: RefCell<Value>,
    confirmed: RefCell<Value>,
    parent: RefCell<Value>,
    generation: RefCell<Generation>,
    updating: Cell<bool>,
    controls: gtk::Box,
    generate_button: gtk::Button,
    manual_button: gtk::Button,
    current_text: RefCell<String>,
}
impl TranslationPreviewPage {
    pub(super) fn new(project_id: String, scene_id: String, client: Process) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.set_margin_start(32);
        widget.set_margin_end(32);
        widget.set_margin_top(26);
        let heading = gtk::Label::new(Some("Scene translations"));
        heading.set_xalign(0.);
        heading.style_context().add_class("page-title");
        widget.pack_start(&heading, false, false, 0);
        let caption=gtk::Label::new(Some("Translations are editable drafts. The manuscript stays authoritative. Each new translation retains earlier drafts and the exact saved original used."));
        caption.set_line_wrap(true);
        caption.set_xalign(0.);
        widget.pack_start(&caption, false, false, 0);
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 8);
        widget.pack_start(&controls, true, true, 0);
        let languages = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.pack_start(&languages, false, false, 0);
        let source_language = gtk::ComboBoxText::new();
        let target_language = gtk::ComboBoxText::new();
        for combo in [&source_language, &target_language] {
            for (code, label) in LANGUAGES {
                combo.append(Some(code), label);
            }
        }
        source_language.set_active_id(Some("en"));
        target_language.set_active_id(Some("ar"));
        languages.pack_start(&gtk::Label::new(Some("Original language")), false, false, 0);
        languages.pack_start(&source_language, false, false, 0);
        languages.pack_start(&gtk::Label::new(Some("Translate into")), false, false, 0);
        languages.pack_start(&target_language, false, false, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.pack_start(&row, false, false, 0);
        let generate_button = button(&row, "Generate translation", || {});
        let manual_button = button(&row, "Add translation myself", || {});
        let cancel = button(&row, "Cancel generation", || {});
        let drafts = gtk::ComboBoxText::new();
        controls.pack_start(&gtk::Label::new(Some("Retained draft")), false, false, 0);
        controls.pack_start(&drafts, false, false, 0);
        let original = gtk::TextView::new();
        original.set_editable(false);
        original.set_cursor_visible(false);
        original.set_wrap_mode(gtk::WrapMode::WordChar);
        let text = gtk::TextView::new();
        text.set_wrap_mode(gtk::WrapMode::WordChar);
        let editors = gtk::Paned::new(gtk::Orientation::Horizontal);
        for (label, view, first) in [
            ("Exact captured original", &original, true),
            ("Editable translation", &text, false),
        ] {
            let col = gtk::Box::new(gtk::Orientation::Vertical, 5);
            let caption = gtk::Label::new(Some(label));
            caption.set_xalign(0.);
            col.pack_start(&caption, false, false, 0);
            let scroller = scroll(view);
            scroller.set_min_content_width(160);
            scroller.set_min_content_height(180);
            col.pack_start(&scroller, true, true, 0);
            if first {
                editors.pack1(&col, true, false);
            } else {
                editors.pack2(&col, true, false);
            }
        }
        controls.pack_start(&editors, true, true, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.pack_start(&row, false, false, 0);
        let save = button(&row, "Save translation", || {});
        let discard = button(&row, "Discard translation edits", || {});
        let refresh = button(&row, "Check current original", || {});
        let status = gtk::Label::new(Some("Loading retained translations…"));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_selectable(true);
        widget.pack_start(&status, false, false, 0);
        let initial = crate::tools::initial("sceneTranslations");
        let p = Rc::new(Self {
            widget,
            text,
            original,
            drafts,
            source_language,
            target_language,
            status,
            controls,
            generate_button,
            manual_button,
            project_id,
            scene_id,
            client,
            value: RefCell::new(initial.clone()),
            confirmed: RefCell::new(initial),
            parent: RefCell::new(Value::Null),
            generation: RefCell::new(Generation::default()),
            updating: Cell::new(false),
            current_text: RefCell::new(String::new()),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            composing: Cell::new(false),
            loaded: Cell::new(false),
        });
        let weak = Rc::downgrade(&p);
        p.text.buffer().unwrap().connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() && p.loaded.get() {
                    p.dirty.set(true);
                    p.status
                        .set_text("Unsaved translation · Save translation to keep your wording");
                    p.sensitivity();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.text.connect_preedit_changed(move |_, text| {
            if let Some(p) = weak.upgrade() {
                p.composing.set(!text.is_empty());
            }
        });
        let weak = Rc::downgrade(&p);
        p.drafts.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() && !p.dirty.get() {
                    p.render_selected();
                }
            }
        });
        for (b, action) in [
            (&p.generate_button, 0),
            (&p.manual_button, 1),
            (&cancel, 2),
            (&save, 3),
            (&discard, 4),
            (&refresh, 5),
        ] {
            let weak = Rc::downgrade(&p);
            b.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    if let Some(a) = p.owner() {
                        match action {
                            0 => p.create(&a, true),
                            1 => p.create(&a, false),
                            2 => p.cancel_generation(),
                            3 => p.save(&a, |_| {}),
                            4 => {
                                if !p.saving.get() && !p.composing.get() {
                                    p.discard();
                                }
                            }
                            _ => p.check_current(&a),
                        }
                    }
                }
            });
        }
        p.sensitivity();
        p
    }
    fn owner(&self) -> Option<Rc<Workspace>> {
        desk::workspace_for_widget(self.widget.upcast_ref())
            .filter(|a| *a.pid.borrow() == self.project_id)
    }
    fn selected(&self) -> Option<Value> {
        let id = self.drafts.active_id()?;
        self.value.borrow()["drafts"]
            .as_array()?
            .iter()
            .find(|d| d["id"] == id.as_str() && d["sceneId"] == self.scene_id)
            .cloned()
    }
    fn sensitivity(&self) {
        let generating = self.generation.borrow().active.is_some();
        let ready = self.loaded.get() && !self.saving.get();
        let clean = ready && !self.dirty.get() && !self.composing.get() && !generating;
        self.controls.set_sensitive(ready);
        self.drafts.set_sensitive(clean);
        self.source_language.set_sensitive(clean);
        self.target_language.set_sensitive(clean);
        self.generate_button.set_sensitive(clean);
        self.manual_button.set_sensitive(clean);
        self.text
            .set_sensitive(ready && self.selected().is_some() && !generating);
    }
    fn render(&self, selected: Option<&str>) {
        let previous = selected
            .map(str::to_owned)
            .or_else(|| self.drafts.active_id().map(|s| s.to_string()));
        self.updating.set(true);
        self.drafts.remove_all();
        let filtered = self.value.borrow()["drafts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["sceneId"] == self.scene_id)
            .cloned()
            .collect::<Vec<_>>();
        for (i, d) in filtered.iter().enumerate() {
            self.drafts.append(
                d["id"].as_str(),
                &format!(
                    "Draft {} · {} → {}",
                    i + 1,
                    d["sourceLanguage"].as_str().unwrap_or(""),
                    d["targetLanguage"].as_str().unwrap_or("")
                ),
            );
        }
        if let Some(id) = previous {
            self.drafts.set_active_id(Some(&id));
        }
        if self.drafts.active_id().is_none() {
            if let Some(d) = filtered.last() {
                self.drafts.set_active_id(d["id"].as_str());
            }
        }
        self.updating.set(false);
        self.render_selected();
    }
    fn render_selected(&self) {
        self.updating.set(true);
        if let Some(d) = self.selected() {
            self.text
                .buffer()
                .unwrap()
                .set_text(d["text"].as_str().unwrap_or(""));
            self.original
                .buffer()
                .unwrap()
                .set_text(d["sourceText"].as_str().unwrap_or(""));
            self.text.set_direction(if d["targetLanguage"] == "ar" {
                gtk::TextDirection::Rtl
            } else {
                gtk::TextDirection::Ltr
            });
            self.status.set_text(
                if d["sourceText"].as_str() != Some(self.current_text.borrow().as_str()) {
                    "Translation may need updating. This draft keeps its exact captured original."
                } else {
                    "Saved draft · edit wording and Save translation"
                },
            );
        } else {
            self.text.buffer().unwrap().set_text("");
            self.original
                .buffer()
                .unwrap()
                .set_text(&self.current_text.borrow());
        }
        self.updating.set(false);
        self.sensitivity();
    }
    pub(super) fn load(self: &Rc<Self>, owner: &Rc<Workspace>) {
        let p = self.clone();
        owner.client.rpc(
            "load_project_data_unambiguous",
            json!({"projectId":self.project_id,"kind":"sceneTranslations"}),
            move |r| {
                if p.owner().is_none() {
                    return;
                }
                match r {
                    Ok(v) => {
                        let value = if v.is_null() {
                            crate::tools::initial("sceneTranslations")
                        } else {
                            v["payload"].clone()
                        };
                        *p.parent.borrow_mut() = v["revisionId"].clone();
                        *p.value.borrow_mut() = value.clone();
                        *p.confirmed.borrow_mut() = value;
                        p.loaded.set(true);
                        p.render(None);
                        if let Some(a) = p.owner() {
                            p.check_current(&a);
                        }
                    }
                    Err(e) => p.status.set_text(&e),
                }
            },
        );
    }
    fn check_current(self: &Rc<Self>, owner: &Rc<Workspace>) {
        let p = self.clone();
        owner.client.rpc(
            "load_project_scene",
            json!({"projectId":self.project_id,"sceneId":self.scene_id}),
            move |r| {
                if p.owner().is_none() {
                    return;
                }
                match r {
                    Ok(v) => {
                        *p.current_text.borrow_mut() =
                            novel_storage::plain_text(&v["scene"]["document"]);
                        if !p.dirty.get()
                            && !p.saving.get()
                            && p.generation.borrow().active.is_none()
                        {
                            p.render_selected();
                        }
                    }
                    Err(e) => p.status.set_text(&e),
                }
            },
        );
    }
    /// Root calls on accepted close/release/project switch. During detach the
    /// active page moves normally; widget ownership is looked up per callback.
    pub(super) fn cancel_generation(&self) {
        let token = self.generation.borrow_mut().cancel();
        if let Some(token) = token {
            self.client.rpc(
                "provider_cancel",
                json!({"jobId":token,"runToken":format!("{token}:1")}),
                |_| {},
            );
            self.status
                .set_text("Translation generation cancelled. Retained drafts are unchanged.");
        }
        self.sensitivity();
    }
    pub(super) fn discard(&self) {
        if self.saving.get() || self.composing.get() {
            return;
        }
        self.cancel_generation();
        *self.value.borrow_mut() = self.confirmed.borrow().clone();
        self.dirty.set(false);
        self.render(None);
        self.status
            .set_text("Translation edits discarded; earlier saved drafts retained.");
    }
    pub(super) fn save(
        self: &Rc<Self>,
        _owner: &Rc<Workspace>,
        callback: impl FnOnce(Result<(), String>) + 'static,
    ) {
        let Some(a) = self.owner() else {
            callback(Err(
                "The translation page is no longer open in this novel.".into()
            ));
            return;
        };
        if !self.loaded.get() || self.saving.get() || self.composing.get() {
            callback(Err(
                "Finish loading, saving or input composition first.".into()
            ));
            return;
        }
        if !self.dirty.get() {
            callback(Ok(()));
            return;
        }
        let Some(draft) = self.selected() else {
            callback(Err("Choose a translation draft.".into()));
            return;
        };
        let id = draft["id"].as_str().unwrap().to_owned();
        let buffer = self.text.buffer().unwrap();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .map(|s| s.to_string())
            .unwrap_or_default();
        let candidate = match edit_draft(&self.value.borrow(), &id, &text) {
            Ok(v) => v,
            Err(e) => {
                self.status.set_text(&e);
                callback(Err(e));
                return;
            }
        };
        *self.value.borrow_mut() = candidate.clone();
        self.saving.set(true);
        self.sensitivity();
        let p = self.clone();
        self.status.set_text("Saving translation…");
        a.client.rpc("save_project_data_checked",json!({"projectId":self.project_id,"kind":"sceneTranslations","payload":candidate,"parent":self.parent.borrow().clone(),"dependencies":[]}),move|r|{p.saving.set(false);match r{Ok(v)=>{*p.parent.borrow_mut()=v["revisionId"].clone();*p.value.borrow_mut()=v["payload"].clone();*p.confirmed.borrow_mut()=v["payload"].clone();p.dirty.set(false);p.render(Some(&id));p.status.set_text("Translation saved locally. Manuscript unchanged.");callback(Ok(()));},Err(e)=>{p.sensitivity();p.status.set_text(&format!("Translation save failed; edited draft retained. Retry Save or discard edits. {e}"));callback(Err(e));}}});
    }
    fn create(self: &Rc<Self>, a: &Rc<Workspace>, generated: bool) {
        if !self.loaded.get() || self.dirty.get() || self.saving.get() || self.composing.get() {
            self.status
                .set_text("Save or discard translation edits and finish composition first.");
            return;
        }
        let source = self
            .source_language
            .active_id()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let target = self
            .target_language
            .active_id()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !language(&source) || !language(&target) || source == target {
            self.status
                .set_text("Choose two different supported languages.");
            return;
        }
        let token = uuid::Uuid::new_v4().to_string();
        if let Err(e) = self.generation.borrow_mut().start(token.clone()) {
            self.status.set_text(&e);
            return;
        }
        self.sensitivity();
        self.status.set_text("Capturing the exact saved original…");
        let p = self.clone();
        let client = a.client.clone();
        client.rpc("load_project_scene",json!({"projectId":self.project_id,"sceneId":self.scene_id}),move|r|{
            if p.owner().is_none()||!p.generation.borrow().current(&token){return;}
            let saved=match r{Ok(v)if v["id"]==p.scene_id=>v,Ok(_)=>{p.generation_failed(&token,"Source scene identity changed.");return;},Err(e)=>{p.generation_failed(&token,&e);return;}};
            let draft=match source_draft(&saved,&source,&target,&uuid::Uuid::new_v4().to_string()){Ok(v)=>v,Err(e)=>{p.generation_failed(&token,&e);return;}};
            *p.current_text.borrow_mut()=draft["sourceText"].as_str().unwrap().into();
            if !generated {p.generation.borrow_mut().cancel();p.add(draft);return;}
            if draft["sourceText"].as_str().unwrap().encode_utf16().count()>12000{p.generation_failed(&token,"Local translation supports 12,000 UTF-16 units. Add and edit a translation yourself for longer scenes.");return;}
            // Configuration is loaded only for this explicit Generate action.
            let out=p.clone();p.client.rpc("load_project_data_unambiguous",json!({"projectId":p.project_id,"kind":"settings"}),move|r|{
                if out.owner().is_none()||!out.generation.borrow().current(&token){return;}
                let settings=match r{Ok(v)=>v["payload"].clone(),Err(e)=>{out.generation_failed(&token,&e);return;}};
                let config=settings["assistance"]["providers"].as_array().and_then(|a|a.iter().find(|v|v["kind"]=="llm")).cloned();let Some(config)=config else{out.generation_failed(&token,"Configure a local writing model in Review settings, or add a translation yourself.");return;};
                let p=out.clone();out.status.set_text("Generating a draft with the explicitly configured local model…");out.client.rpc("provider_translate",json!({"jobId":token,"runToken":format!("{token}:1"),"projectId":out.project_id,"sceneId":out.scene_id,"sceneRevisionId":draft["sceneRevisionId"],"text":draft["sourceText"],"sourceLanguage":source,"targetLanguage":target,"endpoint":config["endpoint"],"model":config["model"],"timeoutMs":120000}),move|r|{
                    if p.owner().is_none()||!p.generation.borrow().current(&token){return;}
                    let result=match r{Ok(v)=>p.generation.borrow_mut().finish(&token,v),Err(e)=>{p.generation_failed(&token,&e);return;}};
                    match result{Ok(Some(text))=>{let mut draft=draft;draft["text"]=json!(text);p.add(draft);},Ok(None)=>(),Err(e)=>{p.sensitivity();p.status.set_text(&format!("{e} Add and edit a translation yourself."));}}
                });
            });
        });
    }
    fn generation_failed(&self, token: &str, error: &str) {
        if !self.generation.borrow().current(token) {
            return;
        }
        self.generation.borrow_mut().cancel();
        self.sensitivity();
        self.status
            .set_text(&format!("{error} Your retained drafts remain available."));
    }
    fn add(&self, draft: Value) {
        let id = draft["id"].as_str().unwrap().to_owned();
        let next = append_draft(&self.value.borrow(), draft);
        match next {
            Ok(v) => {
                *self.value.borrow_mut() = v;
                self.dirty.set(true);
                self.render(Some(&id));
                self.status.set_text(
                    "New editable translation draft · review its wording and Save translation",
                );
            }
            Err(e) => {
                self.sensitivity();
                self.status.set_text(&e)
            }
        }
    }
}
impl Drop for TranslationPreviewPage {
    fn drop(&mut self) {
        if let Some(token) = self.generation.get_mut().cancel() {
            self.client.rpc(
                "provider_cancel",
                json!({"jobId":token,"runToken":format!("{token}:1")}),
                |_| {},
            );
        }
    }
}
#[cfg(test)]
#[path = "translation_preview_tests.rs"]
pub(super) mod tests;
