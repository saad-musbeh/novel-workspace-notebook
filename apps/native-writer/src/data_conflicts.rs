//! Explicit whole-version project-data recovery. No revision is merged or deleted.
use super::*;
use novel_storage::ProjectDataSnapshot;

const KINDS: &[(&str, &str)] = &[
    ("planning", "Story bible and plot board"),
    ("notebookTools", "Notebook scraps, reactions and reminders"),
    ("sceneMetadata", "Scene details"),
    ("paragraphAlternatives", "Paragraph alternatives"),
    ("sceneTranslations", "Translation drafts"),
    ("writingSystems", "Writing systems"),
    ("languagePacks", "Language packs"),
    ("settings", "Review settings"),
    ("assistanceJobs", "Review jobs"),
    ("universeCatalogue", "Universe catalogue"),
    ("chapterNumbering", "Chapter numbering"),
    ("exportProfile", "Word styles"),
    ("branchLineage", "Branch lineage (inspection only)"),
];
#[derive(Clone)]
struct Capture {
    kind: String,
    heads: Vec<String>,
    history: Vec<Value>,
}
struct ConflictPage {
    // Button closures own this controller. Its container and those buttons are
    // weak references so closing the tab releases the controller and its previews.
    widget: glib::WeakRef<gtk::Box>,
    kinds: gtk::ComboBoxText,
    versions: gtk::ComboBoxText,
    status: gtk::Label,
    preview: gtk::TextBuffer,
    heads: gtk::Box,
    resolve: glib::WeakRef<gtk::Button>,
    pid: String,
    capture: RefCell<Option<Capture>>,
    selected: RefCell<Option<ProjectDataSnapshot>>,
    generation: Cell<u64>,
    preview_generation: Cell<u64>,
    updating: Cell<bool>,
    pending: Cell<bool>,
}
fn label(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.);
    l.set_line_wrap(true);
    l.set_selectable(true);
    l
}
impl ConflictPage {
    fn owner(&self) -> Option<Rc<Workspace>> {
        let widget = self.widget.upgrade()?;
        let a = desk::workspace_for_widget(widget.upcast_ref())?;
        if a.pid.borrow().as_str() == self.pid && a.pages.page_num(&widget).is_some() {
            Some(a)
        } else {
            None
        }
    }
    fn can_apply(&self) -> bool {
        !self.pending.get()
            && self.selected.borrow().is_some()
            && self
                .capture
                .borrow()
                .as_ref()
                .is_some_and(|c| c.heads.len() > 1 && c.kind != "branchLineage")
    }
    fn update_apply(&self) {
        if let Some(b) = self.resolve.upgrade() {
            b.set_sensitive(self.can_apply());
        }
    }
    fn load(self: &Rc<Self>) {
        if self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(kind) = self.kinds.active_id().map(|s| s.to_string()) else {
            return;
        };
        let epoch = self.generation.get() + 1;
        self.generation.set(epoch);
        self.preview_generation
            .set(self.preview_generation.get() + 1);
        self.updating.set(true);
        self.versions.remove_all();
        self.updating.set(false);
        *self.capture.borrow_mut() = None;
        *self.selected.borrow_mut() = None;
        self.preview.set_text("");
        for child in self.heads.children() {
            self.heads.remove(&child);
        }
        self.update_apply();
        self.status.set_text("Loading retained versions…");
        let p = self.clone();
        a.client.rpc("project_data_heads",json!({"projectId":self.pid,"kind":kind}),move|reply|{
            if p.owner().is_none() || p.generation.get()!=epoch{return;}
            let heads=match reply.and_then(|v|serde_json::from_value::<novel_storage::RevisionHeads>(v).map_err(|_|"Unsupported retained heads.".into())){
                Ok(h)=>h.revision_ids,Err(e)=>{p.status.set_text(&e);return;}
            };
            let Some(a)=p.owner()else{return;};let next=p.clone();
            a.client.rpc("project_data_history",json!({"projectId":p.pid,"kind":kind}),move|reply|{
                if next.owner().is_none() || next.generation.get()!=epoch{return;}
                let history=match reply{Ok(v)=>match v.as_array(){Some(rows)=>rows.clone(),None=>{next.status.set_text("Unsupported saved history.");return;}},Err(e)=>{next.status.set_text(&e);return;}};
                let count=heads.len();
                *next.capture.borrow_mut()=Some(Capture{kind:kind.clone(),heads:heads.clone(),history:history.clone()});
                next.updating.set(true);
                for revision in &history{
                    let Some(id)=revision["id"].as_str()else{continue;};
                    let current=heads.iter().position(|h|h==id);
                    let name=format!("{} · {}",current.map(|i|format!("Competing version {}",i+1)).unwrap_or_else(||"Earlier retained version".into()),revision["createdAt"].as_str().unwrap_or("Saved revision"));
                    next.versions.append(Some(id),&name);
                }
                next.updating.set(false);
                if count>1{
                    next.status.set_text(&format!("{count} competing versions. Preview each, then explicitly choose the complete version to keep. All earlier revisions remain in history."));
                }else{next.status.set_text(if count==0{"No saved data in this category."}else{"No competing versions. Saved history is available for inspection."});}
                if kind=="branchLineage"{next.status.set_text("Branch lineage is immutable. Its retained history is available for inspection; generic conflict resolution is disabled.");}
                // Each current head receives an independently checksum-checked preview.
                for (i,id) in heads.iter().enumerate(){next.head_preview(epoch,&kind,id,i+1);}
                let first=heads.first().cloned().or_else(||history.first().and_then(|r|r["id"].as_str()).map(str::to_owned));
                if let Some(id)=first{next.versions.set_active_id(Some(&id));}
                next.update_apply();
            });
        });
    }
    fn head_preview(self: &Rc<Self>, epoch: u64, kind: &str, id: &str, number: usize) {
        let Some(a) = self.owner() else {
            return;
        };
        let expander = gtk::Expander::new(Some(&format!("Competing version {number}")));
        let view = gtk::TextView::new();
        view.set_editable(false);
        view.set_cursor_visible(false);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        let buffer = view.buffer().unwrap();
        buffer.set_text("Loading checked preview…");
        let scroller = scroll(&view);
        scroller.set_min_content_height(140);
        expander.add(&scroller);
        self.heads.pack_start(&expander, false, false, 0);
        self.heads.show_all();
        let p = self.clone();
        a.client.rpc(
            "load_project_data_revision",
            json!({"projectId":self.pid,"kind":kind,"revisionId":id}),
            move |reply| {
                if p.owner().is_none() || p.generation.get() != epoch {
                    return;
                }
                match reply.and_then(snapshot) {
                    Ok(v) => buffer.set_text(&describe(&v.payload)),
                    Err(e) => buffer.set_text(&format!("Preview unavailable. {e}")),
                }
            },
        );
    }
    fn select_version(self: &Rc<Self>) {
        if self.updating.get() || self.pending.get() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(capture) = self.capture.borrow().clone() else {
            return;
        };
        let Some(id) = self.versions.active_id() else {
            return;
        };
        if !capture
            .history
            .iter()
            .any(|r| r["id"].as_str() == Some(id.as_str()))
        {
            return;
        }
        *self.selected.borrow_mut() = None;
        self.update_apply();
        self.preview.set_text("Loading checked preview…");
        let generation = self.generation.get();
        let epoch = self.preview_generation.get() + 1;
        self.preview_generation.set(epoch);
        let p = self.clone();
        a.client.rpc(
            "load_project_data_revision",
            json!({"projectId":self.pid,"kind":capture.kind,"revisionId":id.as_str()}),
            move |reply| {
                if p.owner().is_none()
                    || p.generation.get() != generation
                    || p.preview_generation.get() != epoch
                {
                    return;
                }
                match reply.and_then(snapshot) {
                    Ok(v) => {
                        p.preview.set_text(&describe(&v.payload));
                        *p.selected.borrow_mut() = Some(v);
                    }
                    Err(e) => p.preview.set_text(&format!("Preview unavailable. {e}")),
                }
                p.update_apply();
            },
        );
    }
    fn apply(self: &Rc<Self>) {
        if !self.can_apply() {
            return;
        }
        let Some(a) = self.owner() else {
            return;
        };
        let Some(capture) = self.capture.borrow().clone() else {
            return;
        };
        let Some(selected) = self.selected.borrow().clone() else {
            return;
        };
        self.pending.set(true);
        self.kinds.set_sensitive(false);
        self.versions.set_sensitive(false);
        self.update_apply();
        self.status
            .set_text("Checking unsaved pages in this novel’s windows…");
        let p = self.clone();
        a.confirm_project(move|accepted|{
            let Some(owner)=p.owner()else{p.finish("The novel changed. Nothing was resolved.");return;};
            if !accepted{p.finish("Resolution cancelled. The retained versions and your drafts stay available.");return;}
            // Use only the head set captured when these previews were loaded.
            // The storage transaction compares every current head atomically.
            p.status.set_text("Saving your explicit version choice…");let page=p.clone();
            owner.client.rpc("resolve_project_data",json!({"projectId":p.pid,"kind":capture.kind,"payload":selected.payload,"heads":capture.heads}),move|reply|{
                match reply{
                    Ok(_)=>{
                        page.finish("Version choice saved. Every previous version remains in history.");
                        if let Some(owner)=page.owner(){owner.load_index();}
                        // The consumed capture must never permit a second resolution.
                        *page.capture.borrow_mut()=None;*page.selected.borrow_mut()=None;page.update_apply();
                    },
                    Err(e)=>page.finish(&format!("Nothing resolved; all retained versions remain. {e} Refresh versions before choosing again if the heads changed.")),
                }
            });
        });
    }
    fn finish(&self, message: &str) {
        self.pending.set(false);
        self.kinds.set_sensitive(true);
        self.versions.set_sensitive(true);
        self.status.set_text(message);
        self.update_apply();
    }
}
fn snapshot(value: Value) -> Result<ProjectDataSnapshot, String> {
    serde_json::from_value(value).map_err(|_| "Unsupported saved version.".into())
}
impl Workspace {
    pub(super) fn data_conflicts(self: &Rc<Self>) {
        let a = desk::effective(self);
        if a.pid.borrow().is_empty() {
            a.error("Choose a novel before inspecting its project versions.");
            return;
        }
        let origin = a.clone();
        a.navigate(move || origin.open_data_conflicts());
    }
    fn open_data_conflicts(self: &Rc<Self>) {
        let panel = self.panel("Project data conflicts");
        let body = panel.content_area();
        body.pack_start(&label("Inspect complete retained versions of your novel’s notes and settings. Choosing a version creates a new revision; it does not erase any history or merge anything automatically."),false,false,0);
        body.pack_start(&label("Immutable originals, translation sources and writing-system versions must remain intact. A choice that drops or changes them is refused, with all versions preserved."),false,false,0);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let kinds = gtk::ComboBoxText::new();
        for (kind, title) in KINDS {
            kinds.append(Some(kind), title);
        }
        kinds.set_active(Some(0));
        controls.pack_start(&kinds, true, true, 0);
        body.pack_start(&controls, false, false, 0);
        let status = label("Loading retained versions…");
        body.pack_start(&status, false, false, 0);
        let heads = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.pack_start(&scroll(&heads), true, true, 0);
        body.pack_start(&label("Preview a saved version"), false, false, 0);
        let versions = gtk::ComboBoxText::new();
        body.pack_start(&versions, false, false, 0);
        let view = gtk::TextView::new();
        view.set_editable(false);
        view.set_cursor_visible(false);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_left_margin(12);
        view.set_right_margin(12);
        let preview = view.buffer().unwrap();
        let scroller = scroll(&view);
        scroller.set_min_content_height(170);
        body.pack_start(&scroller, true, true, 0);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        body.pack_start(&actions, false, false, 0);
        let resolve = gtk::Button::with_label("Use this retained version");
        actions.pack_start(&resolve, false, false, 0);
        resolve.set_sensitive(false);
        let refresh = gtk::Button::with_label("Refresh versions");
        controls.pack_start(&refresh, false, false, 0);
        let page = Rc::new(ConflictPage {
            widget: panel.widget.downgrade(),
            kinds: kinds.clone(),
            versions: versions.clone(),
            status,
            preview,
            heads,
            resolve: resolve.downgrade(),
            pid: self.pid.borrow().clone(),
            capture: RefCell::new(None),
            selected: RefCell::new(None),
            generation: Cell::new(0),
            preview_generation: Cell::new(0),
            updating: Cell::new(false),
            pending: Cell::new(false),
        });
        let weak = Rc::downgrade(&page);
        kinds.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.load();
            }
        });
        let weak = Rc::downgrade(&page);
        versions.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.select_version();
            }
        });
        let p = page.clone();
        resolve.connect_clicked(move |_| p.apply());
        let p = page.clone();
        refresh.connect_clicked(move |_| p.load());
        self.install(&panel.widget, "Project data conflicts", true);
        page.load();
    }
}
