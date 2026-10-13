//! Notebook chrome and navigation. All windows share one local companion.
use super::*;
use std::rc::Weak;

thread_local! {
    pub(super) static WINDOWS: RefCell<Vec<Rc<Workspace>>> = const { RefCell::new(Vec::new()) };
    static ACTION_WINDOW: RefCell<Option<Rc<Workspace>>> = const { RefCell::new(None) };
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum SaveChoice { Save, Discard, Cancel }
#[derive(Clone)]
enum Draft { Scene(Rc<EditorPane>), Form(Rc<DataPage>), Chapter(Rc<ChapterPage>), Alternatives(Rc<AlternativesPage>), Arrangement(Rc<SceneArrangementPage>), Shortcuts(Rc<ShortcutsPage>), Preferences(Rc<DesktopPreferencesPage>), Ripple(Rc<RipplePage>), Knowledge(Rc<KnowledgePage>), ReviewDecisions(Rc<ReviewDecisionsPage>), Translation(Rc<TranslationPreviewPage>), Alphabet(Rc<AlphabetPage>), LicensedImports(Rc<LicensedImportsPage>), Sequence(Rc<SequencePage>) }
impl Draft {
    fn dirty(&self)->bool { match self { Self::Scene(p)=>p.dirty.get(), Self::Form(p)=>p.dirty.get(), Self::Chapter(p)=>p.dirty.get(), Self::Alternatives(p)=>p.dirty.get(), Self::Arrangement(p)=>p.dirty.get(), Self::Shortcuts(p)=>p.dirty.get(), Self::Preferences(p)=>p.dirty.get(), Self::Ripple(p)=>p.dirty.get(), Self::Knowledge(p)=>p.dirty.get(), Self::ReviewDecisions(p)=>p.dirty.get(), Self::Translation(p)=>p.dirty.get(), Self::Alphabet(p)=>p.dirty.get(), Self::LicensedImports(p)=>p.dirty.get(), Self::Sequence(p)=>p.dirty.get() } }
    fn busy(&self)->bool { match self { Self::Scene(p)=>p.pending.get()!=0 || p.saving.get() || !p.open.get(), Self::Form(p)=>p.saving.get(), Self::Chapter(p)=>p.saving.get(), Self::Alternatives(p)=>p.saving.get(), Self::Arrangement(p)=>p.saving.get(), Self::Shortcuts(p)=>p.saving.get(), Self::Preferences(p)=>p.saving.get(), Self::Ripple(p)=>p.saving.get(), Self::Knowledge(p)=>p.saving.get(), Self::ReviewDecisions(p)=>p.saving.get(), Self::Translation(p)=>p.saving.get(), Self::Alphabet(p)=>p.saving.get() || !p.loaded.get(), Self::LicensedImports(p)=>p.saving.get(), Self::Sequence(p)=>p.saving.get() } }
    fn composing(&self)->bool { match self { Self::Scene(p)=>p.composing.get(), Self::Chapter(p)=>p.composing.get(), Self::Alternatives(p)=>p.composing.get(), Self::Arrangement(p)=>p.composing.get(), Self::Shortcuts(p)=>p.composing.get(), Self::Preferences(p)=>p.composing.get(), Self::Ripple(p)=>p.composing.get(), Self::Knowledge(p)=>p.composing.get(), Self::ReviewDecisions(p)=>p.composing.get(), Self::Translation(p)=>p.composing.get(), Self::Alphabet(p)=>p.composing.get(), Self::LicensedImports(p)=>p.composing.get(), Self::Sequence(p)=>p.composing.get(), Self::Form(_)=>false } }
    fn title(&self)->String { match self { Self::Scene(p)=>p.title.borrow().clone(), Self::Form(p)=>data_title(&p.kind).into(), Self::Chapter(_)=>"Edit chapter".into(), Self::Alternatives(_)=>"Paragraph alternatives".into(), Self::Arrangement(_)=>"Arrange scene".into(), Self::Shortcuts(_)=>"Keyboard shortcuts".into(), Self::Preferences(_)=>"Writing view".into(), Self::Ripple(_)=>"Preview story change".into(), Self::Knowledge(p)=>p.kind.title().into(), Self::ReviewDecisions(_)=>"Review decisions".into(), Self::Translation(_)=>"Translation preview".into(), Self::Alphabet(_)=>"Alphabets".into(), Self::LicensedImports(_)=>"Licensed fonts and dictionaries".into(), Self::Sequence(_)=>"Command sequences".into() } }
}
pub(super) struct Desk {
    pub sidebar:gtk::Box,
    menus:gtk::Box,
    pub area:gtk::ComboBoxText,
    groups:RefCell<Vec<(String,gtk::Box)>>,
    pub find:RefCell<Option<gtk::Expander>>,
    pub find_entry:RefCell<Option<gtk::Entry>>,
    pub format:RefCell<Option<gtk::Expander>>,
    last_scene:RefCell<Option<String>>,
    pub binaries:PathBuf,
    focus:bool,
    group:String,
    selected:RefCell<Option<gtk::Widget>>,
    suppress:Cell<bool>,
    pub(super) busy:Cell<bool>,
    closed:Cell<bool>,
    pub generation:Cell<u64>,
    bindings:RefCell<Vec<(gtk::Widget,Rc<DataPage>)>>,
    titles:RefCell<HashMap<gtk::Widget,String>>,
    dirty_state:Cell<bool>,
    board_controls:gtk::Box,
    board_views:RefCell<Vec<(gtk::Widget,gtk::Widget)>>,
    #[cfg(test)] pub(super) choices:RefCell<std::collections::VecDeque<SaveChoice>>,
}
impl Desk {
    pub fn is_closed(&self)->bool { self.closed.get() }
    pub fn new(sidebar:gtk::Box,menus:gtk::Box,area:gtk::ComboBoxText,binaries:PathBuf,focus:bool)->Self {
        Self { sidebar,menus,area,binaries,focus,group:format!("novel-tabs-{}",uuid::Uuid::new_v4()),groups:RefCell::new(Vec::new()),find:RefCell::new(None),find_entry:RefCell::new(None),format:RefCell::new(None),last_scene:RefCell::new(None),selected:RefCell::new(None),suppress:Cell::new(false),busy:Cell::new(false),closed:Cell::new(false),generation:Cell::new(0),bindings:RefCell::new(Vec::new()),titles:RefCell::new(HashMap::new()),dirty_state:Cell::new(false),board_controls:gtk::Box::new(gtk::Orientation::Vertical,4),board_views:RefCell::new(Vec::new()),#[cfg(test)] choices:RefCell::new(std::collections::VecDeque::new()) }
    }
}
pub(super) fn effective(app:&Rc<Workspace>)->Rc<Workspace> {
    ACTION_WINDOW.with(|current|current.borrow().clone()).unwrap_or_else(||app.clone())
}
pub(super) fn with_button_owner(widget:&gtk::Widget,action:impl FnOnce()) {
    let app=workspace_for_widget(widget);
    let previous=ACTION_WINDOW.with(|current|current.replace(app));action();ACTION_WINDOW.with(|current|current.replace(previous));
}
pub(super) fn workspace_for_widget(widget:&gtk::Widget)->Option<Rc<Workspace>> {
    let window=widget.toplevel().and_then(|w|w.downcast::<gtk::Window>().ok())?;
    WINDOWS.with(|windows|windows.borrow().iter().find(|a|a.window==window && !a.desk.closed.get()).cloned())
}
pub(super) fn register(app:&Rc<Workspace>) { WINDOWS.with(|windows|windows.borrow_mut().push(app.clone())); }
fn owner(window:&gtk::Window)->Rc<Workspace> {
    ACTION_WINDOW.with(|current|current.borrow().clone()).unwrap_or_else(||WINDOWS.with(|windows|windows.borrow().iter().find(|a|a.window==*window).expect("Registered notebook window").clone()))
}
pub(super) fn bind_pane(pane:&Rc<EditorPane>) {
    let weak=Rc::downgrade(pane);
    pane.area.connect_focus_in_event(move|_,_|{
        if let Some(p)=weak.upgrade(){
            WINDOWS.with(|windows|for a in windows.borrow().iter(){
                if a.pages.page_num(&p.widget).is_some(){*a.active.borrow_mut()=Some(p.clone());a.remember_scene(&p.scene_id);}
            });
        }
        glib::Propagation::Proceed
    });
}
pub(super) fn apply_style() {
    let provider=gtk::CssProvider::new();
    provider.load_from_data(br#"
        * { color: #292929; }
        *:disabled { color: #888; }
        window { background: #fff; color: #292929; }
        .desk-sidebar { font-size: 13px; }
        .desk-heading { font-size: 11px; letter-spacing: 2px; color: #707070; }
        button { background-image: none; background-color: #fff; border: 1px solid #d7d7d7; border-radius: 2px; box-shadow: none; text-shadow: none; padding: 5px 9px; }
        button:hover { background-color: #f3f3f3; }
        button:checked, button:active { background-color: #eaeaea; }
        button:focus, entry:focus, combobox:focus { border-color: #646464; }
        .desk-link { border-color: transparent; }
        .desk-link:hover { border-color: #ddd; }
        /* Own every paper surface, including GTK's nested content nodes.
           Dark desktop themes otherwise remain behind our dark text. */
        menu, .menu, menubar, menuitem, popover, popover.background,
        notebook, notebook > stack, scrolledwindow, viewport, .view,
        entry, spinbutton, textview, textview text, treeview, list, row {
            background-image: none; background-color: #fff; color: #292929;
        }
        entry, combobox button { border-radius: 2px; box-shadow: none; background-image: none; }
        menuitem:hover, menuitem:selected, row:selected, treeview:selected,
        .view:selected, textview text:selected, textview text selection,
        entry selection { background-image: none; background-color: #e7e7e7; color: #161616; }
        tooltip { background-image: none; background-color: #fff; color: #292929; border: 1px solid #d7d7d7; }
        scrollbar, scrollbar trough { background-image: none; background-color: #f6f6f6; }
        scrollbar slider { background-image: none; background-color: #c6c6c6; }
        .desk-tabs > header { background-color: #fafafa; border-bottom: 1px solid #ddd; }
        .desk-tabs > header tabs tab { background-image: none; background-color: #fafafa; border: 0; border-right: 1px solid #e3e3e3; border-radius: 0; padding: 7px 12px; box-shadow: none; }
        .desk-tabs > header tabs tab:checked { background-color: #fff; border-bottom: 2px solid #555; }
        .page-title { font-family: serif; font-size: 25px; margin-bottom: 18px; }
        .notebook-hint { color: #777; font-size: 12px; }
        expander title { padding: 5px 0; }
        paned separator { background-color: #e5e5e5; min-width: 1px; }
    "#).expect("Valid notebook style");
    if let Some(screen)=gdk::Screen::default(){gtk::StyleContext::add_provider_for_screen(&screen,&provider,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);}
}
fn data_title(kind:&str)->&str {
    match kind { "planning"=>"Story notes", "settings"=>"Review settings", "sceneMetadata"=>"Scene details", "exportProfile"=>"Word styles", "writingSystems"=>"Alphabets", "sceneTranslations"=>"Translations", _=>"Project notes" }
}

/// A tool page is ordinary tab content, never a separate GTK window.
pub(super) struct Panel {
    owner:Weak<Workspace>,
    pub(super) widget:gtk::Box,
    body:gtk::Box,
    title:String,
    mounted:Cell<bool>,
    generation:u64,
}
impl Panel {
    pub fn content_area(&self)->gtk::Box { self.body.clone() }
    pub fn show_all(self:&Rc<Self>) {
        if let Some(a)=self.owner.upgrade(){
            if self.mounted.get(){self.widget.show_all();return;}
            if self.generation!=a.desk.generation.get() || a.desk.closed.get(){return;}
            let p=self.clone();let app=a.clone();
            a.navigate(move||{p.mounted.set(true);app.install(&p.widget,&p.title,true);});
        }
    }
    pub fn close(&self) {
        let a=WINDOWS.with(|windows|windows.borrow().iter().find(|a|a.pages.page_num(&self.widget).is_some()).cloned());
        if let Some(a)=a{a.close_tab(&self.widget.clone().upcast());}
    }
}
pub(super) fn text_page(parent:&gtk::Window,title:&str,text:&str)->Rc<Panel> {
    let a=effective(&owner(parent));let panel=a.panel(title);
    let view=gtk::TextView::new();view.set_editable(false);view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_left_margin(10);view.set_right_margin(10);view.buffer().unwrap().set_text(text);
    panel.body.pack_start(&scroll(&view),true,true,0);panel.show_all();panel
}
impl Workspace {
    pub(super) fn context_pane(&self)->Option<Rc<EditorPane>> {
        self.active.borrow().clone().or_else(||self.desk.last_scene.borrow().as_ref().and_then(|id|self.panes.borrow().get(id).cloned()))
    }
    pub(super) fn remember_scene(&self,id:&str){*self.desk.last_scene.borrow_mut()=Some(id.into());scene_reference::scene_changed(&self.pid.borrow(),id);}
    pub(super) fn group_name(&self)->&str { &self.desk.group }
    pub(super) fn references(&self)->Vec<(String,String)> {
        self.index.borrow()["scenes"].as_array().into_iter().flatten().filter_map(|s|Some((s["id"].as_str()?.into(),s["title"].as_str().unwrap_or("Scene").into()))).collect()
    }
    fn group(&self,area:&str)->gtk::Box {
        let group=gtk::Box::new(gtk::Orientation::Vertical,6);group.set_no_show_all(true);
        self.desk.menus.pack_start(&group,false,false,0);self.desk.groups.borrow_mut().push((area.into(),group.clone()));group
    }
    fn menu(&self,parent:&gtk::Box,title:&str,expanded:bool)->gtk::Box {
        let expander=gtk::Expander::new(Some(title));expander.set_expanded(expanded);
        let body=gtk::Box::new(gtk::Orientation::Vertical,3);body.set_margin_start(12);expander.add(&body);
        parent.pack_start(&expander,false,false,0);body
    }
    pub(super) fn link(self:&Rc<Self>,row:&gtk::Box,title:&str,action:impl Fn(Rc<Self>)+'static)->gtk::Button {
        let action:Rc<dyn Fn(Rc<Self>)>=Rc::new(action);let other=action.clone();
        self.link_routes(row,title,action,Rc::new(move|a|a.open_link_window(other.clone())))
    }
    fn link_routes(self:&Rc<Self>,row:&gtk::Box,title:&str,action:Rc<dyn Fn(Rc<Self>)>,middle:Rc<dyn Fn(Rc<Self>)>)->gtk::Button {
        let weak=Rc::downgrade(self);
        let b=button(row,title,move||if let Some(a)=weak.upgrade(){let a_=a.clone();let action=action.clone();if a.sequence_window_blocked(){return;}a.navigate(move||action(a_));});
        b.style_context().add_class("desk-link");b.set_halign(gtk::Align::Fill);
        if let Some(label)=b.child().and_then(|w|w.downcast::<gtk::Label>().ok()){label.set_xalign(0.);label.set_ellipsize(gtk::pango::EllipsizeMode::End);}
        b.set_tooltip_text(Some("Open here. Middle click to open in another window."));
        b.add_events(gdk::EventMask::BUTTON_PRESS_MASK|gdk::EventMask::BUTTON_RELEASE_MASK);
        let weak=Rc::downgrade(self);
        b.connect_button_press_event(move|_,event|{
            if event.button()==2{if let Some(a)=weak.upgrade(){if !a.sequence_window_blocked(){middle(a);}}glib::Propagation::Stop}else{glib::Propagation::Proceed}
        });b
    }
    fn control(self:&Rc<Self>,row:&gtk::Box,title:&str,action:fn(&Rc<Self>)) {
        let weak=Rc::downgrade(self);let b=button(row,title,move||if let Some(a)=weak.upgrade(){if !a.sequence_window_blocked(){action(&a);}});
        if let Some(label)=b.child().and_then(|w|w.downcast::<gtk::Label>().ok()){label.set_xalign(0.);}
    }
    pub(super) fn build_menus(self:&Rc<Self>) {
        self.control(&self.desk.sidebar,"Save   Ctrl+S",Self::save_now);
        // Keep Save above the menus, directly below the area selector.
        self.desk.sidebar.reorder_child(self.desk.sidebar.children().last().unwrap(),3);
        let write=self.group("Write");
        let outline=self.menu(&write,"Manuscript",true);outline.pack_start(&self.search,false,false,4);outline.pack_start(&self.binder,true,true,0);
        let create=self.menu(&write,"Add & arrange",false);
        self.control(&create,"New chapter",Self::new_chapter);self.control(&create,"New scene",Self::new_scene);self.control(&create,"Edit chapter",Self::edit_chapter);self.control(&create,"Arrange scene",Self::arrange_scene);self.control(&create,"Rename selected item",Self::rename);
        let edit=self.menu(&write,"Edit",false);
        self.control(&edit,"Undo",|a|a.undo());self.control(&edit,"Redo",|a|a.redo());
        self.control(&edit,"Bold",|a|a.bold());self.control(&edit,"Italic",|a|a.italic());self.control(&edit,"Underline",|a|a.underline());self.control(&edit,"Paragraph format",|a|a.format());
        let format=gtk::Expander::new(Some("Paragraph"));let formatting=gtk::Box::new(gtk::Orientation::Vertical,5);
        let language=gtk::ComboBoxText::new();for (id,name) in [("en","English"),("ar","Arabic"),("zh-Hans","Chinese (simplified)"),("zh-Hant","Chinese (traditional)"),("und","Unspecified language")]{language.append(Some(id),name);}language.set_active(Some(0));
        let direction=gtk::ComboBoxText::new();for (id,name) in [("auto","Automatic direction"),("ltr","Left to right"),("rtl","Right to left")]{direction.append(Some(id),name);}direction.set_active(Some(0));
        let style=gtk::ComboBoxText::new();style.append(Some("paragraph"),"Paragraph");style.append(Some("heading"),"Heading");style.set_active(Some(0));
        let spacing=gtk::ComboBoxText::new();for (id,name) in [("1","Single spacing"),("1.15","1.15 spacing"),("1.5","1.5 spacing"),("2","Double spacing")]{spacing.append(Some(id),name);}spacing.set_active(Some(1));
        for c in [&language,&direction,&style,&spacing]{formatting.pack_start(c,false,false,0);}
        let weak=Rc::downgrade(self);button(&formatting,"Apply to paragraph",move||if let Some(a)=weak.upgrade(){if a.sequence_window_blocked(){return;}if let Some(p)=a.active.borrow().clone(){
            p.operation(json!({"type":"command","command":"blockStyle","language":language.active_id().unwrap().as_str(),"direction":direction.active_id().unwrap().as_str(),"style":style.active_id().unwrap().as_str()}),true);
            p.operation(json!({"type":"command","command":"lineSpacing","lineSpacing":spacing.active_id().unwrap().parse::<f64>().unwrap()}),true);
        }});format.add(&formatting);write.pack_start(&format,false,false,0);*self.desk.format.borrow_mut()=Some(format);
        let find=gtk::Expander::new(Some("Find & replace"));let find_body=gtk::Box::new(gtk::Orientation::Vertical,5);
        let query=gtk::Entry::new();query.set_placeholder_text(Some("Find exact text"));let replacement=gtk::Entry::new();replacement.set_placeholder_text(Some("Replace with"));
        find_body.pack_start(&query,false,false,0);find_body.pack_start(&replacement,false,false,0);
        let weak=Rc::downgrade(self);let q=query.clone();button(&find_body,"Find next",move||if let Some(a)=weak.upgrade(){if a.sequence_window_blocked(){return;}if let Some(p)=a.active.borrow().clone(){p.operation(json!({"type":"command","command":"find","query":q.text().as_str()}),false);}});
        let weak=Rc::downgrade(self);let q=query.clone();button(&find_body,"Replace match",move||if let Some(a)=weak.upgrade(){if a.sequence_window_blocked(){return;}if let Some(p)=a.active.borrow().clone(){p.operation(json!({"type":"command","command":"replace","query":q.text().as_str(),"replacement":replacement.text().as_str()}),true);}});
        find.add(&find_body);write.pack_start(&find,false,false,0);*self.desk.find.borrow_mut()=Some(find);*self.desk.find_entry.borrow_mut()=Some(query);
        let keeps=self.menu(&write,"Keep & return",false);
        self.control(&keeps,"Keep selection as scrap",Self::keep_scrap);
        self.link(&keeps,"Cutting-room floor",|a|a.cutting_room());
        self.control(&keeps,"Leave return bookmark",Self::leave_bookmark);
        self.link(&keeps,"Return to bookmark",|a|a.return_to_bookmark());
        self.control(&keeps,"Clear return bookmark",Self::clear_bookmark);
        let pages=self.menu(&write,"Other pages",false);
        self.link(&pages,"Manuscript overview",|a|a.open_book());
        self.link(&pages,"Scene details",|a|a.details());
        self.link(&pages,"Scene references",|a|a.scene_references());self.link(&pages,"Character cut",|a|a.character_cut());
        self.link(&pages,"Writing view",|a|a.writing_view());self.link(&pages,"Keyboard shortcuts",|a|a.keyboard_shortcuts());
        self.link(&pages,"Choose hold comparison version",|a|a.choose_peek_revision());
        let peek_hint=gtk::Label::new(Some("On the writing page: hold F6 for saved references, F7 for changes."));peek_hint.set_line_wrap(true);peek_hint.set_xalign(0.);pages.pack_start(&peek_hint,false,false,0);
        self.link(&pages,"Paragraph alternatives",|a|a.alternatives());self.link(&pages,"Command sequences",|a|a.command_sequences());
        for kind in ["exportProfile","sceneTranslations"]{self.link(&pages,data_title(kind),move|a|a.open_data(kind,data_title(kind)));}
        self.link(&pages,"Alphabets",|a|a.alphabet_palette());self.link(&pages,"Licensed fonts & dictionaries",|a|a.licensed_imports());
        self.link(&pages,"New translation draft",|a|a.new_translation());
        let planning=self.group("Planning");let story=self.menu(&planning,"Story",true);
        let board_tools=self.menu(&planning,"Board tools",true);board_tools.pack_start(&self.desk.board_controls,false,false,0);
        self.link(&story,"Story notes",|a|a.open_data("planning","Story notes"));self.link(&story,"Plot board",|a|a.open_board());self.link(&story,"Chronology",|a|{text_dialog(&a.window,"Chronology",&crate::planning::chronology(&a.page("planning").value.borrow()));});
        self.link(&story,"Character knowledge",|a|a.character_knowledge());self.link(&story,"Reader learns timeline",|a|a.reader_timeline());
        self.link(&story,"Reader rehearsal",|a|a.reader_rehearsal());self.link(&story,"Preview story change",|a|a.ripple());
        let review=self.group("Review");let checks=self.menu(&review,"Review",true);
        self.link(&checks,"Read like a stranger",|a|a.reader());
        self.link(&checks,"Passages to revisit",|a|a.attention());
        self.link(&checks,"Dictionary and synonyms",|a|a.dictionary());
        self.link(&checks,"Authored continuity checks",|a|a.continuity_checks());
        self.link(&checks,"Spelling exceptions",|a|a.proofreading_exceptions());
        self.link(&checks,"Occurrence chronology and lanes",|a|a.occurrence_lanes());
        self.link(&checks,"Review decisions",|a|a.review_decisions());self.link(&checks,"Translation preview",|a|a.translation_preview());
        self.link(&checks,"Local review",|a|a.review());self.link(&checks,"Review settings",|a|a.open_data("settings","Review settings"));
        let versions=self.menu(&review,"Saved versions",false);self.link(&versions,"Scene history",|a|a.history());self.link_routes(&versions,"Scene conflicts",Rc::new(|a|a.conflicts()),Rc::new(|a|a.open_conflicts_window()));
        self.control(&versions,"Create scene snapshot",Self::snapshot);
        let project=self.group("Project");let library=self.menu(&project,"Novels & files",true);
        self.link(&library,"Library",|a|a.library());self.control(&library,"New novel",Self::new_project);self.control(&library,"Import project",Self::import);self.control(&library,"Export manuscript",Self::export);self.control(&library,"Export alphabet manuscript",Self::alphabet_export);self.control(&library,"Saved manuscript counts",Self::manuscript_statistics);self.control(&library,"Shared World notes",Self::universe_world_notes);
        let recovery=self.menu(&project,"History & recovery",false);self.link(&recovery,"What-if branches",|a|a.branches());self.control(&recovery,"New what-if branch",Self::new_branch);self.link(&recovery,"Project time machine",|a|a.time_machine());self.link(&recovery,"Deleted scenes",|a|a.deleted());
        self.link(&recovery,"Backup now",|a|a.simple_rpc("backup_now",json!({}),"Backup"));self.link(&recovery,"Private sync",|a|a.private_sync());
        self.link(&recovery,"Project data conflicts",|a|a.data_conflicts());
        self.link(&recovery,"Scene writing and deletion recovery",|a|a.scene_conflicts());
        self.link(&recovery,"Manuscript organization conflicts",|a|a.metadata_conflicts());
        let window=self.menu(&self.desk.sidebar,"Tabs & windows",false);
        self.control(&window,"New tab",Self::new_tab);self.control(&window,"New window",|a|{let child=a.spawn_window();child.new_tab();});
        self.control(&window,"Move tab to new window",Self::detach);self.control(&window,"Close this tab",|a|{if let Some(w)=a.current_widget(){a.close_tab(&w);}});
        self.control(&window,"Back",Self::back);self.control(&window,"Forward",Self::forward);
    }
    pub(super) fn update_area(&self) {
        let area=self.desk.area.active_id().unwrap_or_else(||"Write".into());
        for (name,group) in self.desk.groups.borrow().iter(){if name==area.as_str(){group.set_no_show_all(false);group.show_all();group.set_no_show_all(true);}else{group.hide();}}
    }
    pub(super) fn connect_desk(self:&Rc<Self>) {
        let weak=Rc::downgrade(self);self.desk.area.connect_changed(move|_|if let Some(a)=weak.upgrade(){a.update_area();});
        // All renderers of a heading are insensitive, so mouse/key selection
        // cannot treat a universe/series label as a project identity.
        for renderer in self.projects.cells(){self.projects.set_cell_data_func(&renderer,Some(Box::new(|_,cell,model,iter|{let identity=model.value(iter,1).get::<Option<String>>().ok().flatten();cell.set_sensitive(identity.is_some());})));}
        let weak=Rc::downgrade(self);self.projects.connect_changed(move|c|if let Some(a)=weak.upgrade(){if !a.changing.get(){if let Some(pid)=c.active_id(){a.switch_project(pid.into());}else{a.changing.set(true);let current=a.pid.borrow().clone();if !current.is_empty(){c.set_active_id(Some(&current));}a.changing.set(false);}}});
        let weak=Rc::downgrade(self);self.tree.connect_row_activated(move|_,path,_|if let Some(a)=weak.upgrade(){if let Some(i)=a.outline.iter(path){a.open_outline_row(&i,false);}});
        let weak=Rc::downgrade(self);self.tree.connect_button_press_event(move|tree,event|{
            if event.button()==2{
                if let Some(a)=weak.upgrade(){
                    let(x,y)=event.position();
                    if let Some((Some(path),_,_,_))=tree.path_at_pos(x as i32,y as i32){
                        if let Some(i)=a.outline.iter(&path){a.open_outline_row(&i,true);}
                    }
                }
                glib::Propagation::Stop
            }else{glib::Propagation::Proceed}
        });
        let weak=Rc::downgrade(self);self.search.connect_search_changed(move|_|if let Some(a)=weak.upgrade(){a.populate_outline();});
        let weak=Rc::downgrade(self);self.pages.connect_switch_page(move|_,widget,_|if let Some(a)=weak.upgrade(){a.tab_switched(widget);});
        let weak=Rc::downgrade(self);self.pages.connect_local("create-window",false,move|values|{
            let widget=values[1].get::<gtk::Widget>().expect("GTK drag page");
            let a=weak.upgrade().expect("Dragging from a live notebook");
            if a.sequence_window_blocked(){return Some(Option::<gtk::Notebook>::None.to_value());}
            if a.draft(&widget).is_some_and(|d|d.composing()){a.error("Finish the current input composition before moving this tab.");return Some(Option::<gtk::Notebook>::None.to_value());}
            let child=a.spawn_window();child.pages.set_group_name(a.pages.group_name().as_deref());a.transfer_ownership(&widget,&child);
            let child_=child.clone();glib::idle_add_local_once(move||child_.pages.set_group_name(Some(&child_.desk.group)));
            Some(child.pages.to_value())
        });
        let weak=Rc::downgrade(self);self.pages.connect_page_added(move|_,widget,_|if let Some(a)=weak.upgrade(){
            a.decorate_tab(widget);let suppressed=a.desk.suppress.replace(true);a.pages.set_current_page(a.pages.page_num(widget));*a.desk.selected.borrow_mut()=Some(widget.clone());a.activate_widget(widget);a.desk.suppress.set(suppressed);
        });
        let weak=Rc::downgrade(self);self.pages.connect_page_removed(move|_,widget,_|if let Some(a)=weak.upgrade(){
            if a.desk.selected.borrow().as_ref()==Some(widget){*a.desk.selected.borrow_mut()=a.current_widget();if let Some(w)=a.current_widget(){a.activate_widget(&w);}else{*a.active.borrow_mut()=None;}}
        });
        let weak=Rc::downgrade(self);self.window.connect_delete_event(move|_,_|{if let Some(a)=weak.upgrade(){a.close();}glib::Propagation::Stop});
        let weak=Rc::downgrade(self);self.window.connect_key_press_event(move|_,event|weak.upgrade().map(|a|a.workspace_shortcut(event)).unwrap_or(glib::Propagation::Proceed));
        let weak=Rc::downgrade(self);self.window.connect_key_release_event(move|_,event|{
            if let Some(a)=weak.upgrade(){let pane=a.active.borrow().clone();if let Some(pane)=pane{if let Some(result)=peek::key_release(&pane,event){return result;}}}
            glib::Propagation::Proceed
        });
        let weak=Rc::downgrade(self);self.window.connect_focus_out_event(move|_,_|{if let Some(a)=weak.upgrade(){a.cancel_peek();}glib::Propagation::Proceed});
        let weak=Rc::downgrade(self);self.window.connect_unmap(move|_|{if let Some(a)=weak.upgrade(){a.cancel_peek();}});
        let weak=Rc::downgrade(self);glib::timeout_add_local(Duration::from_millis(250),move||{
            let Some(a)=weak.upgrade()else{return glib::ControlFlow::Break;};if a.desk.closed.get(){return glib::ControlFlow::Break;}a.refresh_titles();glib::ControlFlow::Continue
        });
    }
    pub(super) fn current_widget(&self)->Option<gtk::Widget> { self.pages.current_page().and_then(|i|self.pages.nth_page(Some(i))) }
    pub(super) fn shortcut_composing(&self)->bool {self.current_widget().and_then(|w|self.draft(&w)).is_some_and(|d|d.composing())}
    fn draft(&self,widget:&gtk::Widget)->Option<Draft> {
        if let Some(p)=self.sequence_page(widget){return Some(Draft::Sequence(p));}
        if let Some(p)=self.alphabet_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Alphabet(p));}
        if let Some(p)=self.licensed_import_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::LicensedImports(p));}
        if let Some(p)=self.review_decision_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::ReviewDecisions(p));}
        if let Some(p)=self.translation_preview_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Translation(p));}
        if let Some(p)=self.knowledge_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Knowledge(p));}
        if let Some(p)=self.ripple_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Ripple(p));}
        if let Some(p)=self.shortcut_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Shortcuts(p));}
        if let Some(p)=self.desktop_preference_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Preferences(p));}
        if let Some(p)=self.scene_arrangement_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Arrangement(p));}
        if let Some(p)=self.alternative_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Alternatives(p));}
        if let Some(p)=self.chapter_pages.borrow().iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Chapter(p));}
        if let Some(p)=self.panes.borrow().values().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned(){return Some(Draft::Scene(p));}
        if let Some(p)=self.data_pages().into_iter().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget){return Some(Draft::Form(p));}
        self.desk.bindings.borrow().iter().find(|(w,_)|w==widget).map(|(_,p)|Draft::Form(p.clone()))
    }
    fn choice(&self,title:&str)->SaveChoice {
        #[cfg(test)] if let Some(choice)=self.desk.choices.borrow_mut().pop_front(){return choice;}
        let dialog=gtk::Dialog::with_buttons(Some("Unsaved changes"),Some(&self.window),gtk::DialogFlags::MODAL,&[("Cancel",gtk::ResponseType::Cancel),("Discard",gtk::ResponseType::Reject),("Save",gtk::ResponseType::Accept)]);
        dialog.set_default_response(gtk::ResponseType::Cancel);
        let label=gtk::Label::new(Some(&format!("Save changes to “{title}” before leaving?")));label.set_line_wrap(true);label.set_margin_start(20);label.set_margin_end(20);label.set_margin_top(18);label.set_margin_bottom(18);dialog.content_area().pack_start(&label,false,false,0);
        dialog.show_all();let response=dialog.run();dialog.close();match response{gtk::ResponseType::Accept=>SaveChoice::Save,gtk::ResponseType::Reject=>SaveChoice::Discard,_=>SaveChoice::Cancel}
    }
    fn save_draft(self:&Rc<Self>,draft:Draft,callback:impl FnOnce(Result<(),String>)+'static) { match draft{Draft::Scene(p)=>self.save_pane(p,callback),Draft::Form(p)=>{let owner=self.clone();let settings=p.kind=="settings";p.save(move|result|{if settings&&result.is_ok(){let windows=WINDOWS.with(|windows|windows.borrow().iter().filter(|a|*a.pid.borrow()==*owner.pid.borrow()&&!a.desk.is_closed()).cloned().collect::<Vec<_>>());for window in windows{window.reload_shortcuts();window.reload_desktop_preferences();}}callback(result);});},Draft::Chapter(p)=>p.save(self,callback),Draft::Alternatives(p)=>p.save(self,callback),Draft::Arrangement(p)=>p.save(self,callback),Draft::Shortcuts(p)=>p.save(self,callback),Draft::Preferences(p)=>p.save(self,callback),Draft::Ripple(p)=>p.save(self,callback),Draft::Knowledge(p)=>p.save(self,callback),Draft::ReviewDecisions(p)=>p.save(self,callback),Draft::Translation(p)=>p.save(self,callback),Draft::Alphabet(p)=>p.save(Box::new(callback)),Draft::LicensedImports(p)=>p.save(callback),Draft::Sequence(p)=>p.save(self,callback)} }
    // Called ONLY after the explicit Scene Save/Discard choice, never around
    // confirm_all or arbitrary callbacks. Capture the narrow lease synchronously.
    fn confirm_scene_mutation(self:&Rc<Self>,pane:Rc<EditorPane>,kind:sequence_host::DraftMutation,callback:Box<dyn FnOnce(bool)>){
        let completion=Rc::new(RefCell::new(Some(callback)));let finished=completion.clone();let owner=self.clone();
        let done=move|result:Result<(),String>|{
            let accepted=match result{Ok(())=>true,Err(error)=>{owner.error(&format!("Scene action failed. Your draft stays open. {error}"));false}};
            if let Some(callback)=finished.borrow_mut().take(){callback(accepted);}
        };
        let result=sequence_host::with_drain_draft_mutation(&pane,kind,||match kind{
            sequence_host::DraftMutation::Save=>self.save_pane(pane.clone(),done),
            sequence_host::DraftMutation::Discard=>pane.discard(done),
        });
        if let Err(error)=result{self.error(&error);if let Some(callback)=completion.borrow_mut().take(){callback(false);}}
    }
    fn confirm(self:&Rc<Self>,draft:Draft,started:std::time::Instant,callback:Box<dyn FnOnce(bool)>) {
        if draft.composing(){self.error("Finish the current input composition before leaving.");callback(false);return;}
        if draft.busy(){
            if started.elapsed()>Duration::from_secs(15){self.error("The page is still busy. Your draft stays open.");callback(false);return;}
            let a=self.clone();glib::timeout_add_local_once(Duration::from_millis(30),move||a.confirm(draft,started,callback));return;
        }
        if !draft.dirty(){callback(true);return;}
        match self.choice(&draft.title()) {
            SaveChoice::Cancel=>callback(false),
            SaveChoice::Save=>match draft{
                Draft::Scene(p)=>self.confirm_scene_mutation(p,sequence_host::DraftMutation::Save,callback),
                other=>{let a=self.clone();self.save_draft(other,move|r|match r{Ok(())=>callback(true),Err(e)=>{a.error(&format!("Save failed. Your draft stays open. {e}"));callback(false);}});},
            },
            SaveChoice::Discard=>match draft {
                Draft::Scene(p)=>self.confirm_scene_mutation(p,sequence_host::DraftMutation::Discard,callback),
                Draft::Form(p)=>{p.discard();callback(true);}
                Draft::Chapter(p)=>{p.discard();callback(true);}
                Draft::Alternatives(p)=>{p.discard();callback(true);}
                Draft::Arrangement(p)=>{p.discard();callback(true);}
                Draft::Shortcuts(p)=>{p.discard();callback(true);}
                Draft::Preferences(p)=>{p.discard();callback(true);}
                Draft::Ripple(p)=>{p.discard();callback(true);}
                Draft::Knowledge(p)=>{p.discard();callback(true);}
                Draft::ReviewDecisions(p)=>{p.discard();callback(true);}
                Draft::Translation(p)=>{p.discard();callback(true);}
                Draft::Alphabet(p)=>{p.discard();callback(true);}
                Draft::LicensedImports(p)=>{p.discard();callback(true);}
                Draft::Sequence(p)=>{p.discard();callback(true);}
            }
        }
    }
    pub(super) fn navigate(self:&Rc<Self>,action:impl FnOnce()+'static){self.navigate_with_recording_decision(true,action);}
    fn navigate_retained(self:&Rc<Self>,action:impl FnOnce()+'static){self.navigate_with_recording_decision(false,action);}
    fn navigate_with_recording_decision(self:&Rc<Self>,recording:bool,action:impl FnOnce()+'static){
        if self.sequence_window_blocked(){return;}self.cancel_peek();
        if self.desk.busy.replace(true){return;}
        self.window.set_sensitive(false);let a=self.clone();let widget=self.current_widget();
        let done=move|accepted:bool|{
            if !accepted||a.desk.closed.get(){a.window.set_sensitive(true);a.desk.busy.set(false);return;}
            let owner=a.clone();a.capture_departure(move|captured|{owner.window.set_sensitive(true);owner.desk.busy.set(false);if captured&&!owner.desk.closed.get()&&!owner.sequence_window_blocked(){owner.desk.generation.set(owner.desk.generation.get()+1);action();}});
        };
        let owner=self.clone();let leaving=widget.clone();let confirm=move|accepted:bool|{
            if !accepted{done(false);return;}
            if let Some(draft)=widget.and_then(|w|owner.draft(&w)){owner.confirm(draft,std::time::Instant::now(),Box::new(done));}else{done(true);}
        };
        if recording{self.sequence_confirm_leave(leaving,confirm);}else{confirm(true);}
    }
    pub(super) fn confirm_all(self:&Rc<Self>,callback:impl FnOnce(bool)+'static) {
        self.cancel_peek();
        if self.desk.busy.replace(true){callback(false);return;}
        self.window.set_sensitive(false);
        let started=std::time::Instant::now();let failures=self.notebook_failures.get();
        let mut drafts=self.panes.borrow().values().cloned().map(Draft::Scene).collect::<Vec<_>>();
        drafts.extend(self.data_pages().into_iter().map(Draft::Form));
        drafts.extend(self.chapter_pages.borrow().iter().cloned().map(Draft::Chapter));
        drafts.extend(self.alternative_pages.borrow().iter().cloned().map(Draft::Alternatives));
        drafts.extend(self.scene_arrangement_pages.borrow().iter().cloned().map(Draft::Arrangement));
        drafts.extend(self.shortcut_pages.borrow().iter().cloned().map(Draft::Shortcuts));
        drafts.extend(self.desktop_preference_pages.borrow().iter().cloned().map(Draft::Preferences));
        drafts.extend(self.ripple_pages.borrow().iter().cloned().map(Draft::Ripple));
        drafts.extend(self.knowledge_pages.borrow().iter().cloned().map(Draft::Knowledge));
        drafts.extend(self.sequence_host.pages.borrow().iter().cloned().map(Draft::Sequence));
        drafts.extend(self.alphabet_pages.borrow().iter().cloned().map(Draft::Alphabet));
        drafts.extend(self.licensed_import_pages.borrow().iter().cloned().map(Draft::LicensedImports));
        drafts.extend(self.review_decision_pages.borrow().iter().cloned().map(Draft::ReviewDecisions));
        drafts.extend(self.translation_preview_pages.borrow().iter().cloned().map(Draft::Translation));
        let a=self.clone();let owner=self.clone();
        let done:Box<dyn FnOnce(bool)>=Box::new(move|ok|{owner.window.set_sensitive(true);owner.desk.busy.set(false);callback(ok);});
        self.confirm_next(drafts,0,Box::new(move|ok|{
            if ok{a.wait_notebook_writes(started,failures,done);}else{done(false);}
        }));
    }
    fn wait_notebook_writes(self:&Rc<Self>,start:std::time::Instant,failures:u64,callback:Box<dyn FnOnce(bool)>){
        if self.notebook_failures.get()!=failures{
            self.error(&format!("{} · A notebook note could not be saved. This window stays open.",self.status.text()));callback(false);return;
        }
        if self.notebook_pending.get()!=0{
            if start.elapsed()>Duration::from_secs(15){self.error("A notebook note is still saving. The window stays open.");callback(false);return;}
            let a=self.clone();glib::timeout_add_local_once(Duration::from_millis(30),move||a.wait_notebook_writes(start,failures,callback));return;
        }
        callback(true);
    }
    fn confirm_next(self:&Rc<Self>,drafts:Vec<Draft>,i:usize,callback:Box<dyn FnOnce(bool)>) {
        if i==drafts.len(){callback(true);return;}
        let a=self.clone();self.confirm(drafts[i].clone(),std::time::Instant::now(),Box::new(move|ok|{if ok{a.confirm_next(drafts,i+1,callback);}else{callback(false);}}));
    }
    pub(super) fn save_current(self:&Rc<Self>,callback:impl FnOnce(Result<(),String>)+'static) {
        if let Some(draft)=self.current_widget().and_then(|w|self.draft(&w)){self.save_draft(draft,callback);}else{callback(Ok(()));}
    }
    pub(super) fn clear_tabs(&self)->bool {
        if let Err(error)=self.sequence_clear(){self.error(&error);return false;}
        self.cancel_peek();
        self.alphabet_pages.borrow_mut().clear();self.licensed_import_pages.borrow_mut().clear();
        for p in self.translation_preview_pages.borrow().iter(){p.cancel_generation();}
        self.desk.suppress.set(true);while self.pages.n_pages()!=0{self.pages.remove_page(Some(0));}*self.desk.selected.borrow_mut()=None;self.desk.bindings.borrow_mut().clear();self.desk.titles.borrow_mut().clear();self.desk.board_views.borrow_mut().clear();for w in self.desk.board_controls.children(){self.desk.board_controls.remove(&w);}self.desk.suppress.set(false);true
    }
    pub(super) fn install(self:&Rc<Self>,widget:&impl IsA<gtk::Widget>,title:&str,replace:bool) {
        if self.sequence_window_blocked(){return;}
        self.cancel_peek();
        let widget=widget.as_ref();if self.desk.closed.get(){return;}
        self.desk.suppress.set(true);self.desk.titles.borrow_mut().insert(widget.clone(),title.into());
        if self.pages.page_num(widget).is_none(){
            if replace{if let Some(old)=self.current_widget(){self.pages.remove_page(self.pages.page_num(&old));self.release_view(&old);}}
            if let Some(parent)=widget.parent(){if let Ok(c)=parent.downcast::<gtk::Container>(){c.remove(widget);}}
            widget.show_all();self.pages.append_page(widget,Some(&gtk::Label::new(Some(title))));
        }
        self.decorate_tab(widget);self.pages.set_current_page(self.pages.page_num(widget));*self.desk.selected.borrow_mut()=Some(widget.clone());self.activate_widget(widget);self.apply_desktop_to_widget(widget);widget.show_all();self.desk.suppress.set(false);self.refresh_titles();
    }
    fn decorate_tab(self:&Rc<Self>,widget:&gtk::Widget) {
        if self.pages.page_num(widget).is_none(){return;}
        let title=self.draft(widget).and_then(|d|match d{Draft::Scene(p)=>Some(p.title.borrow().clone()),_=>None}).or_else(||self.desk.titles.borrow().get(widget).cloned()).unwrap_or_else(||{
            self.pages.tab_label(widget).and_then(|w|{
                if let Ok(label)=w.clone().downcast::<gtk::Label>(){Some(label.text().to_string())}
                else{w.downcast::<gtk::Box>().ok().and_then(|b|b.children().first().cloned()).and_then(|w|w.downcast::<gtk::Label>().ok()).map(|l|l.text().to_string())}
            }).unwrap_or_else(||"Notebook".into())
        });
        let tab=gtk::Box::new(gtk::Orientation::Horizontal,8);tab.set_tooltip_text(Some(&title));let label=gtk::Label::new(Some(&title));label.set_max_width_chars(25);label.set_width_chars(18);label.set_ellipsize(gtk::pango::EllipsizeMode::End);tab.pack_start(&label,true,true,0);
        let close=gtk::Button::with_label("×");close.set_relief(gtk::ReliefStyle::None);close.set_tooltip_text(Some("Close tab"));close.set_can_focus(false);tab.pack_start(&close,false,false,0);
        let weak=Rc::downgrade(self);let widget_=widget.clone();close.connect_clicked(move|_|if let Some(a)=weak.upgrade(){a.close_tab(&widget_);});
        self.apply_desktop_to_widget(tab.upcast_ref());self.pages.set_tab_label(widget,Some(&tab));self.pages.set_tab_reorderable(widget,true);self.pages.set_tab_detachable(widget,true);tab.show_all();
    }
    fn tab_switched(self:&Rc<Self>,target:&gtk::Widget) {
        if self.desk.suppress.get(){return;}
        let old=self.desk.selected.borrow().clone();if old.as_ref()==Some(target){return;}
        if self.sequence_window_blocked(){let a=self.clone();glib::idle_add_local_once(move||{if let Some(old)=old{if a.pages.page_num(&old).is_some(){a.desk.suppress.set(true);a.pages.set_current_page(a.pages.page_num(&old));a.desk.suppress.set(false);a.activate_widget(&old);}}});return;}
        let Some(old)=old else{*self.desk.selected.borrow_mut()=Some(target.clone());self.activate_widget(target);return;};
        if self.desk.busy.get(){return;}
        self.window.set_sensitive(false);let a=self.clone();let target=target.clone();
        // GTK emits switch-page before its default handler changes the selected page.
        glib::idle_add_local_once(move||{
            if a.pages.page_num(&old).is_none(){a.window.set_sensitive(true);return;}
            a.desk.suppress.set(true);a.pages.set_current_page(a.pages.page_num(&old));a.desk.suppress.set(false);
            let app=a.clone();a.navigate_retained(move||{if app.pages.page_num(&target).is_some(){app.desk.suppress.set(true);app.pages.set_current_page(app.pages.page_num(&target));*app.desk.selected.borrow_mut()=Some(target.clone());app.activate_widget(&target);app.desk.suppress.set(false);}});
        });
    }
    pub(super) fn close_tab(self:&Rc<Self>,widget:&gtk::Widget) {
        if self.sequence_window_blocked(){return;}
        self.cancel_peek();
        if self.pages.page_num(widget).is_none() || self.desk.busy.replace(true){return;}
        self.window.set_sensitive(false);let a=self.clone();let widget=widget.clone();let draft=self.draft(&widget);let leaving=widget.clone();
        let done=move|accepted:bool|{
            a.desk.busy.set(false);a.window.set_sensitive(true);
            if accepted{a.release_review_page(&widget);a.desk.suppress.set(true);a.pages.remove_page(a.pages.page_num(&widget));a.desk.suppress.set(false);
                if let Some(draft)=a.draft(&widget){match draft{
                    Draft::Scene(p)=>{a.panes.borrow_mut().remove(&p.scene_id);p.process.stop();},
                    Draft::Form(p)=>{a.data.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));a.extra_data.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Chapter(p)=>{a.chapter_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Alternatives(p)=>{a.alternative_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Arrangement(p)=>{a.scene_arrangement_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Shortcuts(p)=>{a.shortcut_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Preferences(p)=>{a.desktop_preference_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Ripple(p)=>{a.ripple_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Knowledge(p)=>{a.knowledge_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::ReviewDecisions(p)=>{a.review_decision_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Translation(p)=>{p.cancel_generation();a.translation_preview_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Alphabet(p)=>{a.alphabet_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::LicensedImports(p)=>{a.licensed_import_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));}
                    Draft::Sequence(_)=>{a.sequence_release_view(&widget);}
                }}a.release_view(&widget);
                *a.desk.selected.borrow_mut()=a.current_widget();if let Some(w)=a.current_widget(){a.activate_widget(&w);}else{*a.active.borrow_mut()=None;a.new_tab();}
            }
        };
        let a=self.clone();self.sequence_confirm_leave(Some(leaving),move|accepted|{if !accepted{done(false);return;}if let Some(draft)=draft{a.confirm(draft,std::time::Instant::now(),Box::new(done));}else{done(true);}});
    }
    pub(super) fn new_tab(self:&Rc<Self>) {
        let a=self.clone();self.navigate(move||a.blank_tab());
    }
    fn blank_tab(self:&Rc<Self>) {
        let box_=gtk::Box::new(gtk::Orientation::Vertical,16);box_.set_margin_start(48);box_.set_margin_top(56);
        let title=gtk::Label::new(Some("Your notebook"));title.set_xalign(0.);title.style_context().add_class("page-title");box_.pack_start(&title,false,false,0);
        let empty=self.pid.borrow().is_empty();
        let text=if empty{"Create a novel to begin.\n\nSave when you are ready. You will be asked before leaving an edited page."}else{"Choose a scene from the left to begin.\n\nWrite, Planning, Review and Project each have their own menus.\nSave when you are ready. You will be asked before leaving an edited page."};
        let hint=gtk::Label::new(Some(text));hint.set_xalign(0.);hint.set_line_wrap(true);box_.pack_start(&hint,false,false,0);
        if empty{let row=gtk::Box::new(gtk::Orientation::Horizontal,0);box_.pack_start(&row,false,false,0);let a=self.clone();button(&row,"New novel",move||a.new_project());}
        self.install(&box_,"Notebook",false);
    }
    pub(super) fn panel(self:&Rc<Self>,title:&str)->Rc<Panel> {
        let widget=gtk::Box::new(gtk::Orientation::Vertical,12);widget.set_margin_start(32);widget.set_margin_end(32);widget.set_margin_top(26);widget.set_margin_bottom(20);
        let heading=gtk::Label::new(Some(title));heading.set_xalign(0.);heading.style_context().add_class("page-title");widget.pack_start(&heading,false,false,0);
        let body=gtk::Box::new(gtk::Orientation::Vertical,8);widget.pack_start(&body,true,true,0);
        Rc::new(Panel{owner:Rc::downgrade(self),widget,body,title:title.into(),mounted:Cell::new(false),generation:self.desk.generation.get()})
    }
    pub(super) fn update_page_controls(&self,widget:&gtk::Widget){
        for child in self.desk.board_controls.children(){self.desk.board_controls.remove(&child);}
        if let Some((_,controls))=self.desk.board_views.borrow().iter().find(|(w,_)|w==widget){
            if let Some(parent)=controls.parent(){if let Ok(c)=parent.downcast::<gtk::Container>(){c.remove(controls);}}
            self.desk.board_controls.pack_start(controls,false,false,0);self.desk.board_controls.show_all();
        }
    }
    fn release_view(&self,widget:&gtk::Widget){
        self.cancel_peek();
        self.sequence_release_view(widget);
        self.release_review_page(widget);
        self.alphabet_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.licensed_import_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        for p in self.translation_preview_pages.borrow().iter().filter(|p|p.widget.upcast_ref::<gtk::Widget>()==widget){p.cancel_generation();}
        self.translation_preview_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.review_decision_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.ripple_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.knowledge_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.scene_arrangement_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.shortcut_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);self.desktop_preference_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.alternative_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.chapter_pages.borrow_mut().retain(|p|p.widget.upcast_ref::<gtk::Widget>()!=widget);
        self.desk.titles.borrow_mut().remove(widget);
        self.desk.bindings.borrow_mut().retain(|(w,_)|w!=widget);
        self.desk.board_views.borrow_mut().retain(|(w,_)|w!=widget);
    }
    pub(super) fn current_data(&self,kind:&str)->Option<Rc<DataPage>>{
        self.current_widget().and_then(|w|self.draft(&w)).and_then(|draft|match draft{Draft::Form(p) if p.kind==kind=>Some(p),_=>None})
    }
    pub(super) fn data_for_view(&self,kind:&str)->Rc<DataPage>{
        let page=self.page(kind);let current=self.current_widget();
        let other=self.pages.page_num(&page.widget).is_some() && current.as_ref()!=Some(page.widget.upcast_ref());
        let other_board=self.desk.bindings.borrow().iter().any(|(w,p)|Rc::ptr_eq(p,&page) && self.pages.page_num(w).is_some() && current.as_ref()!=Some(w));
        if !other && !other_board{return page;}
        let page=DataPage::new(self.client.clone(),kind);page.load(&self.pid.borrow(),self.references());self.data.borrow_mut().push(page.clone());page
    }
    pub(super) fn when_loaded(self:&Rc<Self>,page:Rc<DataPage>,callback:impl FnOnce()+'static){
        self.wait_data(page,self.desk.generation.get(),std::time::Instant::now(),Box::new(callback));
    }
    fn wait_data(self:&Rc<Self>,page:Rc<DataPage>,generation:u64,start:std::time::Instant,callback:Box<dyn FnOnce()>){
        if self.desk.closed.get() || self.desk.generation.get()!=generation{return;}
        if page.loaded.get(){callback();return;}
        if start.elapsed()>Duration::from_secs(15){self.error("The saved page could not be loaded. Your current page stays open.");return;}
        let a=self.clone();glib::timeout_add_local_once(Duration::from_millis(30),move||a.wait_data(page,generation,start,callback));
    }
    pub(super) fn open_data(self:&Rc<Self>,kind:&str,title:&str) {
        if let Some(p)=self.current_data(kind){if self.current_widget()==Some(p.widget.clone().upcast()){return;}}
        let a=self.clone();let kind=kind.to_owned();let title=title.to_owned();self.navigate(move||{let page=a.data_for_view(&kind);a.install(&page.widget,&title,true);});
    }
    pub(super) fn open_book(self:&Rc<Self>) { let a=self.clone();self.navigate(move||{if let Some(parent)=a.book.parent(){if let Ok(c)=parent.downcast::<gtk::Container>(){c.remove(&a.book);}}let scroller=scroll(&a.book);a.install(&scroller,"Manuscript",true);}); }
    pub(super) fn open_board(self:&Rc<Self>) {
        let a=self.clone();self.navigate(move||{let page=a.data_for_view("planning");a.mount_board(page,std::time::Instant::now());});
    }
    fn mount_board(self:&Rc<Self>,page:Rc<DataPage>,start:std::time::Instant){
        if !page.loaded.get(){if start.elapsed()>Duration::from_secs(15){self.error("Story notes could not be opened.");return;}let a=self.clone();let generation=self.desk.generation.get();glib::timeout_add_local_once(Duration::from_millis(30),move||{if a.desk.generation.get()==generation{a.mount_board(page,start);}});return;}
        let a=self;let board=crate::planning::board(page.clone(),a.window.clone());
        if let Some(controls)=board.children().first().cloned(){board.remove(&controls);a.desk.board_views.borrow_mut().push((board.clone().upcast(),controls));}
        a.desk.bindings.borrow_mut().push((board.clone().upcast(),page));a.install(&board,"Plot board",true);
    }
    pub(super) fn spawn_window(self:&Rc<Self>)->Rc<Self> {
        let child=Self::build(self.client.clone(),&self.cache,&self.desk.binaries,self.desk.focus,true).expect("A second notebook uses the existing runtime");
        *child.pid.borrow_mut()=self.pid.borrow().clone();*child.index.borrow_mut()=self.index.borrow().clone();child.pages.set_group_name(Some(&child.desk.group));
        child.populate_outline();child.populate_book();if !child.pid.borrow().is_empty(){child.reload_shortcuts();child.reload_desktop_preferences();child.refresh_projects(Some(child.pid.borrow().clone()));}
        child.status.set_text("Manual saving · Ctrl+S to save");child
    }
    fn open_link_window(self:&Rc<Self>,action:Rc<dyn Fn(Rc<Self>)>) {
        let child=self.spawn_window();let source=self.context_pane();
        if let Some(p)=source{let app=child.clone();child.ensure_pane(&p.scene_id,move|r|match r{Ok(p)=>{*app.active.borrow_mut()=Some(p);action(app);},Err(e)=>app.error(&e)});}else{action(child);}
    }
    pub(super) fn open_conflicts_window(self:&Rc<Self>){
        let Some(p)=self.context_pane()else{self.error("Open a scene before reviewing its versions.");return;};
        if p.composing.get(){self.error("Finish the current input composition first.");return;}
        let child=self.spawn_window();*child.active.borrow_mut()=Some(p);child.conflicts();
    }
    pub(super) fn confirm_project(self:&Rc<Self>,callback:impl FnOnce(bool)+'static){let pid=self.pid.borrow().clone();self.confirm_project_id(&pid,callback);}
    pub(super) fn confirm_project_id(self:&Rc<Self>,pid:&str,callback:impl FnOnce(bool)+'static){
        let windows=WINDOWS.with(|windows|windows.borrow().iter().filter(|a|a.pid.borrow().as_str()==pid&&!a.desk.is_closed()).cloned().collect::<Vec<_>>());
        for a in &windows{a.window.set_sensitive(false);}
        let all=windows.clone();self.confirm_windows(windows,0,Box::new(move|ok|{for a in all{if !a.desk.closed.get(){a.window.set_sensitive(true);}}callback(ok);}));
    }
    fn confirm_windows(self:&Rc<Self>,windows:Vec<Rc<Self>>,i:usize,callback:Box<dyn FnOnce(bool)>){
        if i==windows.len(){callback(true);return;}
        let a=self.clone();let window=windows[i].clone();let window_=window.clone();
        window.confirm_all(move|ok|{window_.window.set_sensitive(false);if ok{a.confirm_windows(windows,i+1,callback);}else{callback(false);}});
    }
    fn transfer_ownership(self:&Rc<Self>,widget:&gtk::Widget,child:&Rc<Self>) {
        if self.sequence_window_blocked(){return;}
        self.cancel_peek();
        if let Some(draft)=self.draft(widget){match draft{
            Draft::Scene(p)=>{self.panes.borrow_mut().remove(&p.scene_id);child.apply_desktop_to_pane(&p);child.panes.borrow_mut().insert(p.scene_id.clone(),p.clone());*child.active.borrow_mut()=Some(p);},
            Draft::Shortcuts(p)=>{self.shortcut_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.shortcut_pages.borrow_mut().push(p);},
            Draft::Preferences(p)=>{self.desktop_preference_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.desktop_preference_pages.borrow_mut().push(p);},
            Draft::Ripple(p)=>{self.ripple_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.ripple_pages.borrow_mut().push(p);},
            Draft::Knowledge(p)=>{self.knowledge_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.knowledge_pages.borrow_mut().push(p);},
            Draft::Alphabet(p)=>{self.alphabet_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.alphabet_pages.borrow_mut().push(p);},
            Draft::LicensedImports(p)=>{self.licensed_import_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.licensed_import_pages.borrow_mut().push(p);},
            Draft::Sequence(_)=>{},
            Draft::ReviewDecisions(p)=>{self.review_decision_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.review_decision_pages.borrow_mut().push(p);},
            Draft::Translation(p)=>{self.translation_preview_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.translation_preview_pages.borrow_mut().push(p);},
            Draft::Chapter(p)=>{self.chapter_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.chapter_pages.borrow_mut().push(p);},
            Draft::Alternatives(p)=>{self.alternative_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.alternative_pages.borrow_mut().push(p);},
            Draft::Arrangement(p)=>{self.scene_arrangement_pages.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));child.scene_arrangement_pages.borrow_mut().push(p);},
            Draft::Form(p)=>{
                self.data.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));self.extra_data.borrow_mut().retain(|d|!Rc::ptr_eq(d,&p));
                if p.project_id.borrow().as_str()==self.pid.borrow().as_str(){child.data.borrow_mut().push(p.clone());}else{child.extra_data.borrow_mut().push(p.clone());}
                if widget!=p.widget.upcast_ref::<gtk::Widget>(){child.desk.bindings.borrow_mut().push((widget.clone(),p));}
                self.desk.bindings.borrow_mut().retain(|(w,_)|w!=widget);
            }
        }}
        self.sequence_transfer(widget,child);
        if let Some((_,controls))=self.desk.board_views.borrow().iter().find(|(w,_)|w==widget){
            if let Some(parent)=controls.parent(){if let Ok(c)=parent.downcast::<gtk::Container>(){c.remove(controls);}}
            child.desk.board_views.borrow_mut().push((widget.clone(),controls.clone()));
        }
        self.desk.board_views.borrow_mut().retain(|(w,_)|w!=widget);
        if let Some(title)=self.desk.titles.borrow().get(widget){child.desk.titles.borrow_mut().insert(widget.clone(),title.clone());}
        child.desk.area.set_active_id(self.desk.area.active_id().as_deref());
    }
    pub(super) fn detach_current(self:&Rc<Self>) {
        if self.sequence_window_blocked(){return;}
        if self.desk.busy.get(){return;}let Some(widget)=self.current_widget()else{return;};
        if self.draft(&widget).is_some_and(|d|d.composing()){self.error("Finish the current input composition before moving this tab.");return;}
        let title=self.pages.tab_label(&widget).and_then(|w|w.downcast::<gtk::Box>().ok()).and_then(|b|b.children().first().cloned()).and_then(|w|w.downcast::<gtk::Label>().ok()).map(|l|l.text().to_string()).unwrap_or_else(||"Notebook".into());
        let child=self.spawn_window();self.transfer_ownership(&widget,&child);self.desk.suppress.set(true);self.pages.remove_page(self.pages.page_num(&widget));self.desk.suppress.set(false);child.install(&widget,&title,false);
        *self.desk.selected.borrow_mut()=self.current_widget();if let Some(w)=self.current_widget(){self.activate_widget(&w);}else{*self.active.borrow_mut()=None;self.blank_tab();}
    }
    pub(super) fn finish_close(self:&Rc<Self>) {
        if let Err(error)=self.sequence_clear(){self.error(&error);return;}
        self.cancel_peek();
        self.cancel_review_generations();
        for p in self.translation_preview_pages.borrow().iter(){p.cancel_generation();}
        self.desk.closed.set(true);self.desk.generation.set(self.desk.generation.get()+1);
        for p in self.panes.borrow().values(){p.process.stop();}self.panes.borrow_mut().clear();self.window.hide();unsafe{self.window.destroy();}
        let last=WINDOWS.with(|windows|{windows.borrow_mut().retain(|a|!Rc::ptr_eq(a,self));windows.borrow().is_empty()});
        if last{self.client.stop();if gtk::main_level()>0{gtk::main_quit();}}
    }
    fn refresh_titles(&self) {
        for i in 0..self.pages.n_pages(){if let Some(w)=self.pages.nth_page(Some(i)){if let Some(draft)=self.draft(&w){if let Some(tab)=self.pages.tab_label(&w).and_then(|w|w.downcast::<gtk::Box>().ok()){if let Some(label)=tab.children().first().and_then(|w|w.clone().downcast::<gtk::Label>().ok()){let title=match &draft{Draft::Scene(p)=>p.title.borrow().clone(),_=>self.desk.titles.borrow().get(&w).cloned().unwrap_or_else(||draft.title())};label.set_text(&format!("{}{}",title,if draft.dirty(){" *"}else{""}));}}}}}
        let dirty=self.current_widget().and_then(|w|self.draft(&w)).is_some_and(|d|d.dirty());
        if self.desk.dirty_state.replace(dirty)!=dirty{self.normal_status(if dirty{"Unsaved changes · Ctrl+S to save"}else{"Saved locally · Manual saving"});}
    }
}

#[cfg(test)]
#[path="desk_tests.rs"]
pub(super) mod tests;
