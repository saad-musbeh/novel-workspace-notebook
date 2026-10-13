use crate::{
    editor::EditorPane,files,process::Process,tools::DataPage
};
use gtk::{
    gdk,prelude::*
};
use serde_json::{
    json,Value
};
use std::{
    cell::{
        Cell,RefCell
    },collections::HashMap,path::{
        Path,PathBuf
    },rc::Rc,time::Duration
};
#[path="desk.rs"]
mod desk;
#[path="notebook.rs"]
mod notebook;
#[path="reader.rs"]
mod reader;
#[path="chapter.rs"]
mod chapter;
use chapter::ChapterPage;
#[path="alternatives.rs"]
mod alternatives;
use alternatives::AlternativesPage;
#[path="data_conflicts.rs"]
mod data_conflicts;
#[path="scene_conflicts.rs"]
mod scene_conflicts;
#[path="scene_arrangement.rs"]
mod scene_arrangement;
use scene_arrangement::SceneArrangementPage;
#[path="navigation.rs"]
mod navigation;
#[path="ripple.rs"]
mod ripple;
use ripple::RipplePage;
#[path="command_sequences.rs"]
pub(crate) mod command_sequences;
#[path="sequence_host.rs"]
pub(crate) mod sequence_host;
use sequence_host::{SequenceHost,SequencePage};
#[path="alphabet_host.rs"]
pub(crate) mod alphabet_host;
#[path="alphabet_export.rs"]
pub(crate) mod alphabet_export;
#[path="statistics.rs"]
pub(crate) mod statistics;
#[path="universe_views.rs"]
pub(crate) mod universe_views;
#[path="continuity.rs"]
pub(crate) mod continuity;
#[path="synonym_context.rs"]
pub(crate) mod synonym_context;
#[path="proofreading_exceptions.rs"]
pub(crate) mod proofreading_exceptions;
#[path="licensed_imports.rs"]
mod licensed_imports;
use crate::alphabets::AlphabetPage;
use licensed_imports::LicensedImportsPage;
#[path="review_host.rs"]
mod review_host;
#[path="review_decisions.rs"]
mod review_decisions;
use review_decisions::ReviewDecisionsPage;
#[path="translation_preview.rs"]
mod translation_preview;
use translation_preview::TranslationPreviewPage;
#[path="knowledge.rs"]
mod knowledge;
use knowledge::KnowledgePage;
#[path="peek.rs"]
pub(crate) mod peek;
#[path="rehearsal.rs"]
mod rehearsal;
#[path="character_cut.rs"]
mod character_cut;
#[path="scene_reference.rs"]
mod scene_reference;
#[path="dictionary.rs"]
mod dictionary;
#[path="private_sync.rs"]
mod private_sync;
#[path="metadata_conflicts.rs"]
mod metadata_conflicts;
#[path="shortcuts.rs"]
pub(crate) mod shortcuts;
#[path="desktop_preferences.rs"]
pub(crate) mod desktop_preferences;
use shortcuts::{ShortcutRuntime,ShortcutsPage};
use desktop_preferences::{DesktopRuntime,DesktopPreferencesPage};
use desk::{Desk, Panel, WINDOWS};
pub struct Workspace {
    pub window:gtk::Window,
    client:Process,
    editor_executable:PathBuf,
    cache:PathBuf,
    projects:gtk::ComboBoxText,
    outline:gtk::TreeStore,
    tree:gtk::TreeView,
    search:gtk::SearchEntry,
    pages:gtk::Notebook,
    binder:gtk::ScrolledWindow,
    book:gtk::Box,
    status:gtk::Label,
    pid:RefCell<String>,
    index:RefCell<Value>,
    panes:RefCell<HashMap<String,Rc<EditorPane>>>,
    active:RefCell<Option<Rc<EditorPane>>>,
    data:RefCell<Vec<Rc<DataPage>>>,
    extra_data:RefCell<Vec<Rc<DataPage>>>,
    chapter_pages:RefCell<Vec<Rc<ChapterPage>>>,
    alternative_pages:RefCell<Vec<Rc<AlternativesPage>>>,
    scene_arrangement_pages:RefCell<Vec<Rc<SceneArrangementPage>>>,
    shortcuts:ShortcutRuntime,
    shortcut_pages:RefCell<Vec<Rc<ShortcutsPage>>>,
    desktop_preferences:DesktopRuntime,
    desktop_preference_pages:RefCell<Vec<Rc<DesktopPreferencesPage>>>,
    ripple_pages:RefCell<Vec<Rc<RipplePage>>>,
    knowledge_pages:RefCell<Vec<Rc<KnowledgePage>>>,
    sequence_host:SequenceHost,
    alphabet_pages:RefCell<Vec<Rc<AlphabetPage>>>,
    licensed_import_pages:RefCell<Vec<Rc<LicensedImportsPage>>>,
    review_decision_pages:RefCell<Vec<Rc<ReviewDecisionsPage>>>,
    translation_preview_pages:RefCell<Vec<Rc<TranslationPreviewPage>>>,
    peek:peek::PeekRuntime,
    changing:Cell<bool>,
    closing:Cell<bool>,
    loading:Cell<bool>,
    notebook_pending:Cell<usize>,
    notebook_failures:Cell<u64>,
    project_refresh_epoch:Cell<u64>,
    trail:RefCell<Vec<navigation::TrailEntry>>,
    trail_index:Cell<usize>,
    desk:Desk,
}
fn button(row:&gtk::Box,title:&str,action:impl Fn()+'static)->gtk::Button{
    let button=gtk::Button::with_label(title);
    button.connect_clicked(move|button|desk::with_button_owner(button.upcast_ref(),&action));
    row.pack_start(&button,false,false,0);
    button
}
fn scroll(widget:&impl IsA<gtk::Widget>)->gtk::ScrolledWindow{
    let scroller=gtk::ScrolledWindow::new(None::<&gtk::Adjustment>,None::<&gtk::Adjustment>);
    scroller.add(widget);
    scroller
}
pub fn prompt(parent:&gtk::Window,title:&str,current:&str)->Option<String>{
    prompt_with_action(parent,title,current,crate::ui_text::text("action.apply"))
}
fn prompt_with_action(parent:&gtk::Window,title:&str,current:&str,action:&str)->Option<String>{
    let dialog=gtk::Dialog::with_buttons(Some(title),Some(parent),gtk::DialogFlags::MODAL,&[(crate::ui_text::text("action.cancel"),gtk::ResponseType::Cancel),(action,gtk::ResponseType::Accept)]);
    dialog.set_accept_focus(parent.accepts_focus());dialog.set_focus_on_map(parent.accepts_focus());if !parent.accepts_focus(){dialog.set_keep_below(true);}
    let entry=gtk::Entry::new();
    entry.set_text(current);
    entry.set_activates_default(true);
    dialog.set_default_response(gtk::ResponseType::Accept);
    dialog.content_area().pack_start(&entry,false,false,12);
    dialog.show_all();
    entry.grab_focus();
    let result=if dialog.run()==gtk::ResponseType::Accept{
        Some(entry.text().to_string())
    }else{
        None
    };
    dialog.hide();unsafe{dialog.destroy();}
    result
}
fn choose_option(parent:&gtk::Window,title:&str,options:&[(&str,&str)],current:&str)->Option<String>{
    let dialog=gtk::Dialog::with_buttons(Some(title),Some(parent),gtk::DialogFlags::MODAL,&[(crate::ui_text::text("action.cancel"),gtk::ResponseType::Cancel),(crate::ui_text::text("action.continue"),gtk::ResponseType::Accept)]);
    dialog.set_accept_focus(parent.accepts_focus());dialog.set_focus_on_map(parent.accepts_focus());if !parent.accepts_focus(){dialog.set_keep_below(true);}
    let combo=gtk::ComboBoxText::new();for (id,label) in options{combo.append(Some(id),label);}combo.set_active_id(Some(current));combo.set_margin_start(18);combo.set_margin_end(18);combo.set_margin_top(18);combo.set_margin_bottom(18);dialog.content_area().pack_start(&combo,false,false,0);dialog.show_all();
    let result=if dialog.run()==gtk::ResponseType::Accept{combo.active_id().map(|id|id.to_string())}else{None};dialog.hide();unsafe{dialog.destroy();}result
}
const LANGUAGES:&[(&str,&str)]=&[("en","English"),("ar","Arabic"),("zh-Hans","Chinese (simplified)"),("zh-Hant","Chinese (traditional)")];
fn text_dialog(parent:&gtk::Window,title:&str,text:&str)->Rc<Panel>{
    desk::text_page(parent,title,text)
}
fn describe(value:&Value)->String{
    if value.get("document").is_some(){
        return novel_storage::plain_text(&value["document"]);
    }match value{
        Value::Object(map)=>map.iter().filter(|(k,_)|!k.to_ascii_lowercase().contains("token") && !["schemaVersion","entityId","projectId","sceneId","hash","checksum","requestId","idempotencyKey"].contains(&k.as_str())).map(|(key,v)|format!("{}: {}",key,describe(v))).collect::<Vec<_>>().join("\n"),Value::Array(values)=>values.iter().map(describe).collect::<Vec<_>>().join("\n\n"),Value::String(s)=>s.clone(),Value::Null=>"Unspecified".into(),_=>value.to_string()
    }
}
impl Workspace {
    pub fn new(directory:&Path,cache:&Path,binaries:&Path)->Result<Rc<Self>,String>{
        Self::new_with_focus(directory,cache,binaries,!cfg!(test))
    }
    pub fn new_with_focus(directory:&Path,cache:&Path,binaries:&Path,focus:bool)->Result<Rc<Self>,String>{
        let companion=binaries.join(if binaries.join("novel-companion-rust").is_file(){
            "novel-companion-rust"
        }else{
            "novel-companion"
        });
        let client=Process::start(&companion,directory,true)?;
        Self::build(client,cache,binaries,focus,false)
    }
    fn build(client:Process,cache:&Path,binaries:&Path,focus:bool,empty:bool)->Result<Rc<Self>,String>{
        desk::apply_style();
        let window=gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_focus_on_map(focus);
        window.set_accept_focus(focus);
        if !focus{window.set_keep_below(true);}
        window.set_title(crate::ui_text::text("app.title"));
        window.set_default_size(1120,800);
        let root=gtk::Box::new(gtk::Orientation::Vertical,0);
        window.add(&root);
        let content=gtk::Paned::new(gtk::Orientation::Horizontal);
        let sidebar=gtk::Box::new(gtk::Orientation::Vertical,10);
        sidebar.style_context().add_class("desk-sidebar");
        sidebar.set_size_request(230,-1);
        sidebar.set_margin_start(14);
        sidebar.set_margin_end(14);
        sidebar.set_margin_top(16);
        sidebar.set_margin_bottom(12);
        let heading=gtk::Label::new(Some(crate::ui_text::text("app.notebook_heading")));
        heading.set_xalign(0.);
        heading.style_context().add_class("desk-heading");
        sidebar.pack_start(&heading,false,false,0);
        let projects=gtk::ComboBoxText::new();
        projects.set_tooltip_text(Some(crate::ui_text::text("app.choose_novel")));
        sidebar.pack_start(&projects,false,false,0);
        let area=gtk::ComboBoxText::new();
        for (id,key) in [("Write","area.write"),("Planning","area.planning"),("Review","area.review"),("Project","area.project")]{area.append(Some(id),crate::ui_text::text(key));}
        area.set_active(Some(0));
        sidebar.pack_start(&area,false,false,0);
        let menu_scroll=gtk::ScrolledWindow::new(None::<&gtk::Adjustment>,None::<&gtk::Adjustment>);
        menu_scroll.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);
        let menus=gtk::Box::new(gtk::Orientation::Vertical,8);
        menu_scroll.add(&menus);
        sidebar.pack_start(&menu_scroll,true,true,0);
        let search=gtk::SearchEntry::new();
        search.set_placeholder_text(Some(crate::ui_text::text("app.find_scene")));
        let outline=gtk::TreeStore::new(&[String::static_type(),String::static_type(),String::static_type()]);
        let tree=gtk::TreeView::with_model(&outline);
        tree.set_headers_visible(false);
        tree.set_activate_on_single_click(true);
        let renderer=gtk::CellRendererText::new();
        renderer.set_property("ellipsize",gtk::pango::EllipsizeMode::End);
        let column=gtk::TreeViewColumn::new();
        gtk::prelude::CellLayoutExt::pack_start(&column,&renderer,true);
        gtk::prelude::CellLayoutExt::add_attribute(&column,&renderer,"text",0);
        tree.append_column(&column);
        let binder=scroll(&tree);
        binder.set_min_content_height(180);
        binder.set_max_content_height(340);
        binder.set_propagate_natural_height(true);
        let pages=gtk::Notebook::new();
        pages.set_scrollable(true);
        pages.set_show_border(false);
        pages.style_context().add_class("desk-tabs");
        content.pack1(&sidebar,false,false);
        content.pack2(&pages,true,false);
        content.set_position(260);
        root.pack_start(&content,true,true,0);
        let status=gtk::Label::new(Some(crate::ui_text::text("status.opening_notebook")));
        status.set_xalign(0.);
        status.set_line_wrap(true);
        status.set_margin_start(16);
        status.set_margin_end(16);
        status.set_margin_bottom(6);
        status.set_margin_top(6);
        root.pack_start(&status,false,false,0);
        let book=gtk::Box::new(gtk::Orientation::Vertical,12);
        book.set_margin_start(36);book.set_margin_end(36);book.set_margin_top(28);
        let app=Rc::new(Self{
            window,client,editor_executable:binaries.join(if binaries.join("novel-editor-rust").is_file(){"novel-editor-rust"}else{"novel-editor"}),
            cache:cache.into(),projects,outline,tree,search,pages,binder,status,
            pid:RefCell::new(String::new()),index:RefCell::new(Value::Null),panes:RefCell::new(HashMap::new()),active:RefCell::new(None),
            data:RefCell::new(Vec::new()),extra_data:RefCell::new(Vec::new()),chapter_pages:RefCell::new(Vec::new()),alternative_pages:RefCell::new(Vec::new()),scene_arrangement_pages:RefCell::new(Vec::new()),shortcuts:ShortcutRuntime::default(),shortcut_pages:RefCell::new(Vec::new()),desktop_preferences:DesktopRuntime::default(),desktop_preference_pages:RefCell::new(Vec::new()),ripple_pages:RefCell::new(Vec::new()),knowledge_pages:RefCell::new(Vec::new()),sequence_host:SequenceHost::default(),alphabet_pages:RefCell::new(Vec::new()),licensed_import_pages:RefCell::new(Vec::new()),review_decision_pages:RefCell::new(Vec::new()),translation_preview_pages:RefCell::new(Vec::new()),peek:peek::PeekRuntime::default(),changing:Cell::new(false),closing:Cell::new(false),loading:Cell::new(false),notebook_pending:Cell::new(0),notebook_failures:Cell::new(0),project_refresh_epoch:Cell::new(0),book,
            trail:RefCell::new(Vec::new()),trail_index:Cell::new(0),desk:Desk::new(sidebar,menus,area,binaries.to_path_buf(),focus)
        });
        desk::register(&app);
        app.build_menus();
        app.connect_desk();
        app.window.show_all();
        app.update_area();
        if !empty{app.new_tab();app.refresh_projects(None);}
        Ok(app)
    }
    fn page(&self,kind:&str)->Rc<DataPage>{
        if let Some(page)=self.current_data(kind){return page;}
        if let Some(page)=self.data.borrow().iter().find(|p|p.kind==kind).cloned(){return page;}
        let page=DataPage::new(self.client.clone(),kind);
        if !self.pid.borrow().is_empty(){page.load(&self.pid.borrow(),self.references());}
        self.data.borrow_mut().push(page.clone());
        page
    }
    fn data_pages(&self)->Vec<Rc<DataPage>>{
        self.data.borrow().iter().cloned().chain(self.extra_data.borrow().iter().cloned()).collect()
    }
    fn command(&self,command:&str){
        if let Some(p)=self.active.borrow().clone(){
            p.operation(json!({
                "type":"command","command":command
            }),command!="selectAll");
        }
    }
    fn undo(&self){
        self.command("undo");
    }fn redo(&self){
        self.command("redo");
    }fn bold(&self){
        self.command("bold");
    }fn italic(&self){
        self.command("italic");
    }fn underline(&self){
        self.command("underline");
    }
    fn error(&self,error:&str){
        self.status.set_text(error);
    }
    fn normal_status(&self,message:&str){
        if let Some(notice)=self.optional_preferences_notice(){self.status.set_text(&format!("{message}\n{notice}"));}else{self.status.set_text(message);}
    }
    fn refresh_projects(self:&Rc<Self>,selection:Option<String>){
        let epoch=self.project_refresh_epoch.get()+1;self.project_refresh_epoch.set(epoch);
        let captured=self.pid.borrow().clone();let weak=Rc::downgrade(self);
        self.client.rpc("universe_catalogue_snapshot",json!({}),move|reply|if let Some(a)=weak.upgrade(){
            if a.desk.is_closed()||a.project_refresh_epoch.get()!=epoch{return;}
            match reply.and_then(|value|serde_json::from_value::<novel_storage::UniverseCatalogueSnapshot>(value).map_err(|e|e.to_string())){
                Ok(snapshot)=>{
                    let rows=universe_views::selector_rows(&snapshot);
                    let current=a.pid.borrow().clone();
                    let wanted=if current!=captured&&!current.is_empty(){Some(current)}else{selection.or_else(||(!current.is_empty()).then_some(current))};
                    let first=rows.iter().find_map(|row|row.project_id.clone());
                    a.changing.set(true);a.projects.remove_all();
                    for row in &rows{a.projects.append(row.project_id.as_deref(),&row.label);}
                    a.changing.set(false);
                    if let Some(id)=wanted.filter(|id|rows.iter().any(|row|row.project_id.as_ref()==Some(id))).or(first){a.projects.set_active_id(Some(&id));}
                    if snapshot.projects.is_empty(){a.status.set_text("Choose New novel to start writing.");}
                },Err(error)=>a.error(&format!("Novel catalogue could not be refreshed. Existing writing stays open. {error}"))
            }
        });
    }
    fn switch_project(self:&Rc<Self>,pid:String){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.switch_project(pid);return;}
        if self.sequence_window_blocked(){self.changing.set(true);self.projects.set_active_id(Some(&self.pid.borrow()));self.changing.set(false);return;}
        if pid==*self.pid.borrow() || self.loading.get(){return;}
        self.loading.set(true);
        let a=self.clone();let previous=self.pid.borrow().clone();
        let owner=self.clone();self.sequence_confirm_leave(None,move|sequence_accepted|{
            if !sequence_accepted{a.loading.set(false);a.changing.set(true);a.projects.set_active_id(Some(&previous));a.changing.set(false);return;}
            owner.confirm_all(move|accepted|{
            a.loading.set(false);
            if !accepted{
                a.changing.set(true);a.projects.set_active_id(Some(&previous));a.changing.set(false);return;
            }
            if let Err(error)=a.sequence_clear(){a.error(&error);a.changing.set(true);a.projects.set_active_id(Some(&previous));a.changing.set(false);return;}
            a.cancel_review_generations();
            if !a.clear_tabs(){a.changing.set(true);a.projects.set_active_id(Some(&previous));a.changing.set(false);return;}
            for pane in a.panes.borrow().values(){pane.process.stop();}
            a.panes.borrow_mut().clear();a.data.borrow_mut().clear();a.extra_data.borrow_mut().clear();a.chapter_pages.borrow_mut().clear();a.alternative_pages.borrow_mut().clear();a.scene_arrangement_pages.borrow_mut().clear();a.shortcut_pages.borrow_mut().clear();a.desktop_preference_pages.borrow_mut().clear();a.ripple_pages.borrow_mut().clear();a.knowledge_pages.borrow_mut().clear();a.alphabet_pages.borrow_mut().clear();a.licensed_import_pages.borrow_mut().clear();a.review_decision_pages.borrow_mut().clear();a.translation_preview_pages.borrow_mut().clear();
            *a.active.borrow_mut()=None;*a.pid.borrow_mut()=pid;
            *a.index.borrow_mut()=Value::Null;a.populate_outline();a.populate_book();
            a.pages.set_group_name(Some(a.group_name()));
            a.cancel_peek();
            a.trail.borrow_mut().clear();a.trail_index.set(0);a.desk.generation.set(a.desk.generation.get()+1);
            a.reload_shortcuts();a.reload_desktop_preferences();a.load_index();
            });
        });
    }
    fn load_index(self:&Rc<Self>){
        let weak=Rc::downgrade(self);
        let pid=self.pid.borrow().clone();
        let selected=pid.clone();
        self.client.rpc("open_project_index",json!({
            "projectId":pid
        }),move|reply|if let Some(a)=weak.upgrade(){
            if a.desk.is_closed() || *a.pid.borrow()!=selected{
                return;
            }match reply{
                Ok(index)=>{
                    *a.index.borrow_mut()=index.clone();
                    a.populate_outline();
                    a.populate_book();
                    let references=index["scenes"].as_array().into_iter().flatten().map(|s|(s["id"].as_str().unwrap().to_owned(),s["title"].as_str().unwrap_or("Scene").to_owned())).collect::<Vec<_>>();
                    for p in a.data.borrow().iter(){
                        p.load(&selected,references.clone());
                    }a.normal_status("Manual saving · Ctrl+S to save · Middle click opens another window");
                    if a.pages.n_pages()==0{
                        if let Some(id)=index["scenes"].as_array().and_then(|v|v.first()).and_then(|s|s["id"].as_str()){
                            a.open_start_scene(id.to_owned());
                        }
                    }
                },Err(e)=>a.error(&e)
            }
        });
    }
    fn populate_outline(&self){
        self.outline.clear();
        let index=self.index.borrow();
        let query=self.search.text().to_lowercase();
        for chapter in index["chapters"].as_array().into_iter().flatten(){
            let scenes:Vec<_>=index["scenes"].as_array().into_iter().flatten().filter(|s|s["chapterId"]==chapter["id"] && (query.is_empty() || s["title"].as_str().unwrap_or("").to_lowercase().contains(&query) || chapter["title"].as_str().unwrap_or("").to_lowercase().contains(&query))).collect();
            if !query.is_empty() && scenes.is_empty() && !chapter["title"].as_str().unwrap_or("").to_lowercase().contains(&query){
                continue;
            }let row=self.outline.append(None);
            let title=chapter["title"].as_str().unwrap_or("Chapter");
            let id=chapter["id"].as_str().unwrap();
            self.outline.set(&row,&[(0,&title),(1,&id),(2,&"chapter")]);
            for scene in scenes{
                let row=self.outline.append(Some(&row));
                self.outline.set(&row,&[(0,&scene["title"].as_str().unwrap_or("Scene")),(1,&scene["id"].as_str().unwrap()),(2,&"scene")]);
            }
        }self.tree.expand_all();
    }
    fn populate_book(self:&Rc<Self>){
        for child in self.book.children(){self.book.remove(&child);}
        let heading=gtk::Label::new(Some("Manuscript"));heading.set_xalign(0.);heading.style_context().add_class("page-title");
        self.book.pack_start(&heading,false,false,0);
        let index=self.index.borrow();
        for chapter in index["chapters"].as_array().into_iter().flatten(){
            let group=gtk::Expander::new(chapter["title"].as_str());group.set_expanded(true);
            let scenes=gtk::Box::new(gtk::Orientation::Vertical,4);
            for scene in index["scenes"].as_array().into_iter().flatten().filter(|s|s["chapterId"]==chapter["id"]){
                let id=scene["id"].as_str().unwrap().to_owned();
                self.link(&scenes,scene["title"].as_str().unwrap_or("Untitled scene"),move|a|a.open_scene(&id,false));
            }
            if scenes.children().is_empty(){
                let id=chapter["id"].as_str().unwrap().to_owned();
                self.link(&scenes,"Start writing",move|a|a.open_chapter(&id,false));
            }
            group.add(&scenes);self.book.pack_start(&group,false,false,8);
        }
        self.book.show_all();
    }
    fn activate_widget(&self,widget:&gtk::Widget){
        self.cancel_peek();
        self.update_page_controls(widget);
        let pane=self.panes.borrow().values().find(|p|p.widget.upcast_ref::<gtk::Widget>()==widget).cloned();
        if let Some(p)=&pane{self.remember_scene(&p.scene_id);}
        *self.active.borrow_mut()=pane;
    }
    pub(super) fn open_outline_row(self:&Rc<Self>,iter:&gtk::TreeIter,new_window:bool){
        let id=self.outline.value(iter,1).get::<String>().unwrap_or_default();
        match self.outline.value(iter,2).get::<String>().unwrap_or_default().as_str(){
            "chapter"=>self.open_chapter(&id,new_window),
            "scene"=>self.open_scene(&id,new_window),
            _=>{}
        }
    }
    fn open_chapter(self:&Rc<Self>,id:&str,new_window:bool){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.open_chapter(id,new_window);return;}
        if new_window{let child=self.spawn_window();child.open_chapter(id,false);return;}
        let (title,scene)={
            let index=self.index.borrow();
            let Some(chapter)=index["chapters"].as_array().into_iter().flatten().find(|c|c["id"]==id)else{self.error("This chapter is no longer available.");return;};
            (chapter["title"].as_str().unwrap_or("Chapter").to_owned(),index["scenes"].as_array().into_iter().flatten().find(|s|s["chapterId"]==id).and_then(|s|s["id"].as_str()).map(str::to_owned))
        };
        if let Some(scene)=scene{self.open_scene(&scene,false);return;}
        let panel=self.panel(&title);let body=panel.content_area();
        let hint=gtk::Label::new(Some("This chapter is empty. Start writing to add its first scene."));hint.set_xalign(0.);hint.set_line_wrap(true);body.pack_start(&hint,false,false,0);
        let row=gtk::Box::new(gtk::Orientation::Horizontal,0);body.pack_start(&row,false,false,0);
        let widget=panel.widget.downgrade();let a=self.clone();let chapter=id.to_owned();let pid=self.pid.borrow().clone();let busy=Rc::new(Cell::new(false));
        let start=gtk::Button::with_label("Start writing");row.pack_start(&start,false,false,0);
        start.connect_clicked(move|button|desk::with_button_owner(button.upcast_ref(),||{
            let a=desk::effective(&a);let Some(widget)=widget.upgrade()else{return;};
            if busy.get() || *a.pid.borrow()!=pid || a.current_widget().as_ref()!=Some(widget.upcast_ref()){return;}
            busy.set(true);button.set_sensitive(false);let button=button.downgrade();let busy=busy.clone();
            a.start_chapter(&chapter,widget.upcast(),move||{busy.set(false);if let Some(button)=button.upgrade(){button.set_sensitive(true);}});
        }));
        let a=self.clone();let chapter=id.to_owned();let pid=self.pid.borrow().clone();
        start.add_events(gdk::EventMask::BUTTON_PRESS_MASK);
        start.connect_button_press_event(move|button,event|{
            if event.button()==2{desk::with_button_owner(button.upcast_ref(),||{let a=desk::effective(&a);if *a.pid.borrow()==pid{a.open_chapter(&chapter,true);}});glib::Propagation::Stop}else{glib::Propagation::Proceed}
        });
        panel.show_all();
    }
    fn start_chapter(self:&Rc<Self>,chapter:&str,source:gtk::Widget,done:impl FnOnce()+'static){
        // Refresh metadata first: another window may already have started this chapter.
        let a=self.clone();let chapter=chapter.to_owned();let pid=self.pid.borrow().clone();let generation=self.desk.generation.get();
        self.client.rpc("open_project_index",json!({"projectId":pid}),move|reply|{
            if a.desk.is_closed() || *a.pid.borrow()!=pid || a.desk.generation.get()!=generation || a.current_widget()!=Some(source){done();return;}
            match reply{
                Ok(index)=>{
                    if !index["chapters"].as_array().into_iter().flatten().any(|c|c["id"]==chapter){a.error("This chapter is no longer available.");done();return;}
                    let scene=index["scenes"].as_array().into_iter().flatten().find(|s|s["chapterId"]==chapter).and_then(|s|s["id"].as_str()).map(str::to_owned);
                    *a.index.borrow_mut()=index;a.populate_outline();a.populate_book();
                    if let Some(scene)=scene{a.open_scene(&scene,false);done();}else{a.create_and_open_scene(chapter,"Scene 1".into(),done);}
                },Err(error)=>{a.error(&error);done();}
            }
        });
    }
    pub fn open_scene(self:&Rc<Self>,id:&str,new_window:bool){
        if self.sequence_window_blocked(){return;}
        if !new_window{
            // GTK can synchronously emit focus-in and update active here.
            // Release the RefCell borrow before calling back into GTK.
            let active=self.active.borrow().clone();
            if let Some(p)=active{if p.scene_id==id && self.current_widget()==Some(p.widget.clone().upcast()){p.area.grab_focus();return;}}
        }
        if new_window{
            let child=self.spawn_window();child.open_scene(id,false);return;
        }
        let a=self.clone();let id=id.to_owned();
        self.navigate(move||{
            let next=a.clone();let generation=a.desk.generation.get();
            a.ensure_pane(&id,move|result|{
                if next.desk.generation.get()!=generation || next.desk.is_closed(){return;}
                match result{Ok(pane)=>next.show_pane(pane,false),Err(error)=>next.error(&error)}
            });
        });
    }
    fn ensure_pane(self:&Rc<Self>,id:&str,callback:impl FnOnce(Result<Rc<EditorPane>,String>)+'static){
        if let Some(pane)=self.panes.borrow().get(id).cloned(){callback(Ok(pane));return;}
        if self.panes.borrow().len()>=16&&!self.sequence_host.panes.borrow().iter().any(|b|b.runtime.busy()){
            let evict=self.panes.borrow().iter().find(|(_,p)|p.widget.parent().is_none()&&!p.dirty.get()&&p.pending.get()==0&&!p.saving.get()).map(|(id,_)|id.clone());
            if let Some(id)=evict{if let Some(p)=self.panes.borrow_mut().remove(&id){p.process.stop();}}
        }
        if self.panes.borrow().len()>=16{callback(Err("Close a writing tab before opening another. Up to 16 scenes may be open at once.".into()));return;}
        let weak=Rc::downgrade(self);let selected=self.pid.borrow().clone();let generation=self.desk.generation.get();let scene_id=id.to_owned();
        self.client.rpc("load_project_scene",json!({"projectId":selected,"sceneId":scene_id}),move|reply|{
            let Some(a)=weak.upgrade()else{return;};
            if *a.pid.borrow()!=selected||a.desk.is_closed()||a.desk.generation.get()!=generation{return;}
            let record=match reply{Ok(record)=>record,Err(error)=>{callback(Err(error));return;}};
            if let Some(existing)=a.panes.borrow().get(&scene_id).cloned(){callback(Ok(existing));return;}
            let weak=Rc::downgrade(&a);let client=a.client.clone();
            alphabet_host::load_editor_profile(&client,selected.clone(),record.clone(),move|loaded|{
                let Some(a)=weak.upgrade()else{return;};
                if *a.pid.borrow()!=selected||a.desk.is_closed()||a.desk.generation.get()!=generation{return;}
                if let Some(existing)=a.panes.borrow().get(&scene_id).cloned(){callback(Ok(existing));return;}
                // Resource capture failure supplies literal/default options.
                // Recheck saved manuscript state independently before spawning.
                let weak=Rc::downgrade(&a);
                a.client.rpc("load_project_scene",json!({"projectId":selected,"sceneId":scene_id}),move|reply|{
                    let Some(a)=weak.upgrade()else{return;};
                    if *a.pid.borrow()!=selected||a.desk.is_closed()||a.desk.generation.get()!=generation{return;}
                    if let Some(existing)=a.panes.borrow().get(&scene_id).cloned(){callback(Ok(existing));return;}
                    let current=match reply{Ok(value)=>value,Err(error)=>{callback(Err(error));return;}};
                    if current!=record{callback(Err("The saved scene changed while its alphabet profile loaded. Open the writing scene again; every saved version remains intact.".into()));return;}
                    if a.panes.borrow().len()>=16{callback(Err("Close a writing tab before opening another. Up to 16 scenes may be open at once.".into()));return;}
                    match EditorPane::new_with_profile(&a.editor_executable,&a.cache,&record,loaded){
                        Ok(pane)=>{
                            if let Some(existing)=a.panes.borrow().get(&pane.scene_id).cloned(){pane.process.stop();callback(Ok(existing));return;}
                            a.apply_desktop_to_pane(&pane);a.panes.borrow_mut().insert(pane.scene_id.clone(),pane.clone());desk::bind_pane(&pane);callback(Ok(pane));
                        },Err(error)=>callback(Err(error))
                    }
                });
            });
        });
    }
    fn show_pane(self:&Rc<Self>,pane:Rc<EditorPane>,_beside:bool){
        if self.sequence_window_blocked(){return;}
        pane.widget.set_size_request(-1,-1);
        let title=pane.title.borrow().clone();
        self.install(&pane.widget,&title,true);
        self.show_return_note(&pane);
        self.remember_scene(&pane.scene_id);*self.active.borrow_mut()=Some(pane.clone());pane.area.grab_focus();
        self.record_navigation(&pane.scene_id);
        self.sequence_bind_pane(&pane);
    }
    pub fn save_pane(self:&Rc<Self>,pane:Rc<EditorPane>,callback:impl FnOnce(Result<(),String>)+'static){
        let permit=match sequence_host::authorize_draft_mutation(&pane,sequence_host::DraftMutation::Save){Ok(permit)=>permit,Err(error)=>{callback(Err(error));return;}};
        if pane.composing.get(){
            callback(Err("Finish the current input composition before leaving.".into()));
            return;
        }if pane.saving.get(){
            callback(Err("A scene save is still running. Wait for its confirmation.".into()));
            return;
        }if !pane.dirty.get() && pane.pending.get()==0{
            callback(Ok(()));
            return;
        }pane.saving.set(true);
        let a=self.clone();
        let p=pane.clone();
        pane.snapshot(move|document|{
            let document=match document{
                Ok(d)=>d,Err(e)=>{
                    p.saving.set(false);
                    callback(Err(e));
                    return;
                }
            };
            let saved_document=document.clone();
            let saved_title=p.title.borrow().clone();
            if !sequence_host::draft_mutation_allowed(&p,&permit,sequence_host::DraftMutation::Save){p.saving.set(false);callback(Err("Save authorization changed before storage dispatch; draft and Undo retained.".into()));return;}
            let client=a.client.clone();
            client.rpc("save_scene",json!({
                "sceneId":p.scene_id,"scene":{
                    "schemaVersion":1,"title":saved_title,"document":document
                },"parent":p.parent.borrow().clone(),"requestId":uuid::Uuid::new_v4().to_string()
            }),move|reply|{
                p.saving.set(false);
                match reply{
                    Ok(saved)=>{
                        *p.saved_document.borrow_mut()=saved_document.clone();*p.saved_title.borrow_mut()=saved_title.clone();
                        *p.parent.borrow_mut()=saved["revisionId"].as_str().unwrap().into();
                        p.dirty.set(p.frame.borrow()["document"]!=saved_document || *p.title.borrow()!=saved_title);
                        for notebook in [&a.pages]{
                            if notebook.page_num(&p.widget).is_none(){continue;}
                            if let Some(tab)=notebook.tab_label(&p.widget).and_then(|w|w.downcast::<gtk::Box>().ok()){
                                if let Some(title)=tab.children().first().and_then(|w|w.clone().downcast::<gtk::Label>().ok()){
                                    title.set_text(&p.title.borrow());
                                }
                            }
                        }
                        a.status.set_text("Saved locally");
                        callback(Ok(()));
                    },Err(e)=>{
                        p.dirty.set(true);
                        p.error.set_text(&format!("Save failed; draft retained. Middle click Scene conflicts in Review to compare this draft. {e}"));
                        a.error(&e);
                        callback(Err(e));
                    }
                }
            });
        });
    }
    fn drain(self:&Rc<Self>,callback:impl FnOnce(Result<(),String>)+'static){
        self.confirm_project(move|accepted|if accepted{callback(Ok(()));}else{callback(Err("Cancelled. Your page stays open.".into()));});
    }
    fn save_now(self:&Rc<Self>){
        if self.sequence_window_blocked(){return;}
        let a=self.clone();
        self.save_current(move|result|match result{Ok(())=>a.normal_status("Saved locally"),Err(e)=>a.error(&e)});
    }
    fn close(self:&Rc<Self>){
        if self.sequence_window_blocked(){return;}self.cancel_peek();
        if self.closing.replace(true){return;}
        let a=self.clone();let owner=self.clone();
        self.sequence_confirm_leave(None,move|accepted|{
            if !accepted{a.closing.set(false);return;}
            owner.confirm_all(move|accepted|{a.closing.set(false);if accepted{a.finish_close();}});
        });
    }
    fn new_project(self:&Rc<Self>){
        if self.sequence_window_blocked(){return;}
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.new_project();return;}
        if self.desk.is_closed() || self.loading.replace(true){return;}
        let Some(title)=prompt_with_action(&self.window,"New novel","","Create")else{
            self.loading.set(false);
            return;
        };
        if title.trim().is_empty(){
            self.loading.set(false);self.error("Enter a name for the new novel.");
            return;
        }
        let a=self.clone();let owner=self.clone();self.sequence_confirm_leave(None,move|accepted|{
            if !accepted || a.desk.is_closed(){a.loading.set(false);return;}
            owner.confirm_all(move|accepted|{
            if !accepted || a.desk.is_closed(){a.loading.set(false);return;}
            // Keep the confirmed drafts unchanged until creation and selection finish.
            a.window.set_sensitive(false);let app=a.clone();
            a.client.rpc("create_novel",json!({"title":title}),move|reply|{
                app.loading.set(false);if app.desk.is_closed(){return;}app.window.set_sensitive(true);
                match reply{
                    Ok(created)=>{
                        if let Some(pid)=created["project"]["id"].as_str(){
                            app.search.set_text("");app.desk.area.set_active_id(Some("Write"));
                            app.switch_project(pid.to_owned());app.refresh_projects(Some(pid.to_owned()));
                        }
                    },Err(error)=>app.error(&error)
                }
            });
            });
        });
    }
    fn new_chapter(self:&Rc<Self>){
        let Some(title)=prompt(&self.window,"New chapter","Chapter")else{
            return;
        };
        let a=self.clone();
        self.client.rpc("create_chapter",json!({
            "projectId":self.pid.borrow().clone(),"title":title
        }),move|reply|match reply{
            Ok(_)=>a.load_index(),Err(e)=>a.error(&e)
        });
    }
    fn selected(&self)->Option<(String,String)>{
        let(model,iter)=self.tree.selection().selected()?;
        Some((model.value(&iter,1).get::<String>().ok()?,model.value(&iter,2).get::<String>().ok()?))
    }
    fn new_scene(self:&Rc<Self>){
        let Some(title)=prompt(&self.window,"New scene","Untitled scene")else{
            return;
        };
        let chapter=self.selected().and_then(|(id,kind)|if kind=="chapter"{
            Some(id)
        }else{
            self.index.borrow()["scenes"].as_array()?.iter().find(|s|s["id"]==id)?["chapterId"].as_str().map(str::to_owned)
        }).or_else(||self.index.borrow()["chapters"].as_array()?.first()?["id"].as_str().map(str::to_owned));
        let Some(chapter)=chapter else{
            self.error("Create a chapter first.");
            return;
        };
        let a=self.clone();self.navigate(move||a.create_and_open_scene(chapter,title,||{}));
    }
    fn create_and_open_scene(self:&Rc<Self>,chapter:String,title:String,done:impl FnOnce()+'static){
        let a=self.clone();let pid=self.pid.borrow().clone();let generation=self.desk.generation.get();
        self.client.rpc("create_scene",json!({
            "chapterId":chapter,"scene":{
                "schemaVersion":1,"title":title,"document":{
                    "type":"doc","content":[{
                        "type":"paragraph"
                    }]
                }
            }
        }),move|reply|{
            if !a.desk.is_closed() && *a.pid.borrow()==pid{match reply{
                Ok(record)=>{
                    a.load_index();
                    if a.desk.generation.get()==generation{if let Some(id)=record["id"].as_str(){a.open_scene(id,false);}}
                },Err(e)=>a.error(&e)
            }}
            done();
        });
    }
    fn rename(self:&Rc<Self>){
        let Some((id,kind))=self.selected()else{
            return;
        };
        if kind=="chapter"{self.edit_chapter();return;}
        let title=self.index.borrow()["scenes"].as_array().and_then(|v|v.iter().find(|s|s["id"]==id)).and_then(|s|s["title"].as_str()).unwrap_or("").to_owned();
        let Some(title)=prompt(&self.window,"Rename",&title)else{
            return;
        };
        let a=self.clone();
        if let Some(p)=self.panes.borrow().get(&id).cloned(){
            *p.title.borrow_mut()=title;
            p.dirty.set(true);
            a.status.set_text("Name changed · Save to keep this scene name");
        }else{
            let a=self.clone();
            self.client.rpc("load_scene",json!({
                "sceneId":id
            }),move|reply|match reply{
                Ok(s)=>{
                    let a_=a.clone();
                    a.client.rpc("rename_scene",json!({
                        "sceneId":id,"title":title,"parent":s["revisionId"]
                    }),move|r|match r{
                        Ok(_)=>a_.load_index(),Err(e)=>a_.error(&e)
                    });
                },Err(e)=>a.error(&e)
            });
        }
    }
    fn detach(self:&Rc<Self>){self.detach_current();}
    fn find(&self){
        self.desk.area.set_active_id(Some("Write"));
        if let Some(expander)=self.desk.find.borrow().as_ref(){expander.set_expanded(true);}
        if let Some(entry)=self.desk.find_entry.borrow().as_ref(){entry.grab_focus();}
    }
    fn format(&self){
        self.desk.area.set_active_id(Some("Write"));
        if let Some(expander)=self.desk.format.borrow().as_ref(){expander.set_expanded(true);}
    }
    fn simple_rpc(self:&Rc<Self>,op:&str,args:Value,title:&str){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.simple_rpc(op,args,title);return;}
        let weak=Rc::downgrade(self);
        let title=title.to_owned();
        self.client.rpc(op,args,move|r|if let Some(a)=weak.upgrade(){
            match r{
                Ok(value)=>{
                    text_dialog(&a.window,&title,&describe(&value));
                },Err(e)=>a.error(&e)
            }
        });
    }
    fn history(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.history();return;}
        let Some(p)=self.context_pane()else{
            return;
        };
        let a=self.clone();
        self.client.rpc("history",json!({
            "sceneId":p.scene_id
        }),move|reply|match reply{
            Ok(revisions)=>{
                let dialog=a.panel("Scene history");
                let list=gtk::Box::new(gtk::Orientation::Vertical,8);
                for revision in revisions.as_array().into_iter().flatten(){
                    let row=gtk::Box::new(gtk::Orientation::Horizontal,4);
                    row.pack_start(&gtk::Label::new(revision["createdAt"].as_str()),true,true,4);
                    let revision=revision.clone();
                    let revision_=revision.clone();
                    let a_=a.clone();
                    let id=p.scene_id.clone();
                    button(&row,"Preview",move||{
                        let a=desk::effective(&a_);
                        a_.client.rpc("load_revision",json!({
                            "sceneId":id,"revisionId":revision_["id"]
                        }),move|r|match r{
                            Ok(s)=>{
                                text_dialog(&a.window,"Saved scene",&novel_storage::plain_text(&s["scene"]["document"]));
                            },Err(e)=>a.error(&e)
                        });
                    });
                    let a_=a.clone();
                    let p_=p.clone();
                    button(&row,"Restore as new revision",move||{
                        let a=desk::effective(&a_);
                        let p=p_.clone();
                        let revision=revision.clone();
                        a_.drain(move|r|{
                            if let Err(e)=r{
                                a.error(&e);
                                return;
                            }let a_=a.clone();
                            a.client.rpc("restore_scene",json!({
                                "sceneId":p.scene_id,"revisionId":revision["id"],"parent":p.parent.borrow().clone()
                            }),move|r|match r{
                                Ok(_)=>a_.reload_scene(&p.scene_id),Err(e)=>a_.error(&e)
                            });
                        });
                    });
                    list.pack_start(&row,false,false,0);
                }
                dialog.content_area().pack_start(&scroll(&list),true,true,0);
                
                dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn reload_scene(self:&Rc<Self>,id:&str){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.reload_scene(id);return;}
        let protected=self.panes.borrow().get(id).is_some_and(|p|p.dirty.get()||p.pending.get()!=0||p.saving.get()||p.composing.get())
            ||self.sequence_host.panes.borrow().iter().any(|b|b.runtime.busy());
        if protected{self.error("Saved writing changed. Your draft, recording and Undo are retained against their original version. Finish recording and choose Save or Discard before reopening this scene.");return;}
        if let Some(p)=self.panes.borrow_mut().remove(id){
            if let Some(parent)=p.widget.parent(){
                if let Ok(c)=parent.downcast::<gtk::Container>(){
                    c.remove(&p.widget);
                }
            }if self.active.borrow().as_ref().is_some_and(|a|a.scene_id==id){
                *self.active.borrow_mut()=None;
            }p.process.stop();
        }self.open_scene(id,false);
    }
    fn snapshot(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.snapshot();return;}
        let Some(p)=self.context_pane()else{
            return;
        };
        let a=self.clone();
        self.drain(move|r|match r{
            Ok(())=>a.simple_rpc("snapshot_scene",json!({
                "sceneId":p.scene_id,"parent":p.parent.borrow().clone()
            }),"Snapshot saved"),Err(e)=>a.error(&e)
        });
    }
    fn export(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.export();return;}
        let Some(format)=choose_option(&self.window,crate::ui_text::text("export.title"),&[("docx",crate::ui_text::text("export.docx")),("text",crate::ui_text::text("export.text")),("markdown",crate::ui_text::text("export.markdown")),("html",crate::ui_text::text("export.html")),("project",crate::ui_text::text("export.project"))],"docx")else{return;};
        if format=="docx"{let page=self.page("exportProfile");let a=self.clone();self.when_loaded(page,move||a.export_saved(format));}else{self.export_saved(format);}
    }
    fn export_saved(self:&Rc<Self>,format:String){
        let a=self.clone();
        self.drain(move|r|{
            if let Err(e)=r{
                a.error(&e);
                return;
            }let op=if format=="docx"{
                "word_export_snapshot"
            }else if format=="project"{
                "export_archive"
            }else{
                "manuscript_export_snapshot"
            };
            let a_=a.clone();
            a.client.rpc(op,json!({
                "projectId":a.pid.borrow().clone(),"profileRevisionId":a.page("exportProfile").parent.borrow().clone()
            }),move|r|{
                let value=match r{
                    Ok(v)=>v,Err(e)=>{
                        a_.error(&e);
                        return;
                    }
                };
                let suffix=match format.as_str(){
                    "docx"=>".docx","text"=>".txt","markdown"=>".md","html"=>".html",_=>".novel.json"
                };
                let patterns=vec![format!("*{suffix}")];
                let Some(path)=files::choose(&a_.window,true,&format!("manuscript{suffix}"),&patterns.iter().map(String::as_str).collect::<Vec<_>>())else{
                    return;
                };
                let result=if format=="docx"{
                    serde_json::from_value::<novel_storage::WordExportSnapshot>(value).map_err(|e|e.to_string()).and_then(|snapshot|novel_docx::export_manuscript_with_profile(&snapshot.bundle,&snapshot.profile))
                }else if format=="project"{
                    Ok(value.as_str().unwrap_or("").as_bytes().to_vec())
                }else{
                    serde_json::from_value::<novel_storage::ProjectBundle>(value).map_err(|e|e.to_string()).map(|bundle|crate::export::manuscript(&bundle,&format).into_bytes())
                };
                match result.and_then(|bytes|files::write(&path,&bytes)){
                    Ok(())=>a_.status.set_text(&format!("Exported {}",path.display())),Err(e)=>a_.error(&e)
                }
            });
        });
    }
    fn import(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.import();return;}
        let Some(path)=files::choose(&self.window,false,"Complete project archive",&["*.novel.json","*.json"])else{
            return;
        };
        let archive=match files::read(&path){
            Ok(v)=>v,Err(e)=>{
                self.error(&e);
                return;
            }
        };
        let a=self.clone();
        self.drain(move|r|{
            if let Err(e)=r{
                a.error(&e);
                return;
            }let a_=a.clone();
            a.client.rpc("import_archive",json!({
                "archive":archive
            }),move|r|match r{
                Ok(p)=>a_.refresh_projects(p["project"]["id"].as_str().map(str::to_owned)),Err(e)=>a_.error(&e)
            });
        });
    }
    fn new_branch(self:&Rc<Self>){
        let Some(title)=prompt(&self.window,"New what-if branch","What if…")else{return;};
        if title.trim().is_empty(){return;}
        let a=self.clone();self.drain(move|r|{
            if let Err(e)=r{a.error(&e);return;}
            let a_=a.clone();a.client.rpc("create_project_branch",json!({"sourceProjectId":a.pid.borrow().clone(),"title":title}),move|r|match r{Ok(p)=>a_.refresh_projects(p["projectId"].as_str().map(str::to_owned)),Err(e)=>a_.error(&e)});
        });
    }
    fn branches(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.branches();return;}
        let a=self.clone();
        self.client.rpc("list_project_branches",json!({
            "sourceProjectId":self.pid.borrow().clone()
        }),move|reply|match reply{
            Ok(branches)=>{
                let dialog=a.panel("What-if branches");
                let list=gtk::Box::new(gtk::Orientation::Vertical,6);
                for branch in branches.as_array().into_iter().flatten(){
                    let row=gtk::Box::new(gtk::Orientation::Horizontal,4);
                    row.pack_start(&gtk::Label::new(branch["title"].as_str()),true,true,0);
                    let a_=a.clone();
                    let branch=branch.clone();
                    button(&row,"Compare",move||a_.compare_branch(branch.clone()));
                    list.pack_start(&row,false,false,0);
                }dialog.content_area().pack_start(&list,true,true,6);
                
                dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn compare_branch(self:&Rc<Self>,branch:Value){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.compare_branch(branch);return;}
        let a=self.clone();
        self.client.rpc("compare_project_branch",json!({
            "sourceProjectId":self.pid.borrow().clone(),"branchProjectId":branch["projectId"],"offset":0,"limit":100
        }),move|reply|match reply{
            Ok(comparison)=>{
                let dialog=a.panel("Review branch changes");
                let list=gtk::Box::new(gtk::Orientation::Vertical,8);
                for piece in comparison["pieces"].as_array().into_iter().flatten(){
                    let row=gtk::Box::new(gtk::Orientation::Horizontal,4);
                    row.pack_start(&gtk::Label::new(Some(&format!("{} · changed: {}",piece["title"].as_str().unwrap_or("Story data"),piece["differs"]))),true,true,0);
                    let preview=piece.clone();
                    let preview_branch=branch.clone();
                    let a_=a.clone();
                    button(&row,"Preview",move||a_.simple_rpc("load_entity_revision",json!({
                        "projectId":preview_branch["projectId"],"entityId":preview["branchEntityId"],"entityType":preview["entityType"],"revisionId":preview["branchHeadIds"][0]
                    }),"Branch version"));
                    let enabled=piece["canBringBack"]==true;
                    let piece=piece.clone();
                    let branch=comparison["branch"].clone();
                    let a_=a.clone();
                    let apply=button(&row,"Bring back",move||{
                        let a=desk::effective(&a_);
                        let piece=piece.clone();
                        let branch=branch.clone();
                        a_.drain(move|r|{
                            if let Err(e)=r{
                                a.error(&e);
                                return;
                            }let a_=a.clone();
                            a.client.rpc("bring_back_branch_piece",json!({
                                "sourceProjectId":a.pid.borrow().clone(),"branchProjectId":branch["projectId"],"lineageRevisionId":branch["lineageRevisionId"],"entityId":piece["branchEntityId"],"entityType":piece["entityType"],"revisionId":piece["branchHeadIds"][0],"expectedSourceHeads":piece["sourceHeadIds"],"expectedBranchHeads":piece["branchHeadIds"]
                            }),move|r|match r{
                                Ok(_)=>a_.load_index(),Err(e)=>a_.error(&e)
                            });
                        });
                    });
                    apply.set_sensitive(enabled);
                    list.pack_start(&row,false,false,0);
                }dialog.content_area().pack_start(&scroll(&list),true,true,0);
                
                dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn time_machine(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.time_machine();return;}
        let a=self.clone();
        self.client.rpc("project_time_points",json!({
            "projectId":self.pid.borrow().clone(),"beforeSequence":null,"limit":100
        }),move|reply|match reply{
            Ok(points)=>{
                let dialog=a.panel("Project time machine");
                let list=gtk::Box::new(gtk::Orientation::Vertical,6);
                for point in points["points"].as_array().into_iter().flatten(){
                    let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                    row.pack_start(&gtk::Label::new(Some(&format!("{} · {}",point["createdAt"].as_str().unwrap_or("Saved point"),point["operation"].as_str().unwrap_or("Edit")))),true,true,0);
                    let point=point.clone();
                    let epoch=points["epoch"].clone();
                    let a_=a.clone();
                    button(&row,"Preview this point",move||a_.preview_time(epoch.clone(),point.clone()));
                    list.pack_start(&row,false,false,0);
                }dialog.content_area().pack_start(&scroll(&list),true,true,0);
                
                dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn preview_time(self:&Rc<Self>,epoch:Value,point:Value){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.preview_time(epoch,point);return;}
        let a=self.clone();
        self.client.rpc("project_time_preview",json!({
            "projectId":self.pid.borrow().clone(),"epoch":epoch,"sequence":point["sequence"],"pointRevisionId":point["revisionId"]
        }),move|reply|match reply{
            Ok(preview)=>{
                let dialog=a.panel("Historical project versions");
                let list=gtk::Box::new(gtk::Orientation::Vertical,6);
                for object in preview["objects"].as_array().into_iter().flatten(){
                    for head in object["heads"].as_array().into_iter().flatten(){
                        let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                        row.pack_start(&gtk::Label::new(head["title"].as_str().or_else(||head["entityType"].as_str())),true,true,0);
                        let obj=object.clone();
                        let head_=head.clone();
                        let a_=a.clone();
                        button(&row,"Preview",move||a_.simple_rpc("load_entity_revision",json!({
                            "projectId":a_.pid.borrow().clone(),"entityId":obj["entityId"],"entityType":head_["entityType"],"revisionId":head_["revisionId"]
                        }),"Historical version"));
                        let obj=object.clone();
                        let head=head.clone();
                        let preview=preview.clone();
                        let a_=a.clone();
                        let recoverable=object["entityType"]=="scene" || object["entityId"]==format!("{}:planning",a.pid.borrow());
                        let apply=button(&row,"Copy as new revision",move||{
                            let a=desk::effective(&a_);
                            let obj=obj.clone();
                            let head=head.clone();
                            let preview=preview.clone();
                            a_.drain(move|r|{
                                if let Err(e)=r{
                                    a.error(&e);
                                    return;
                                }let a_=a.clone();
                                a.client.rpc("recover_time_piece",json!({
                                    "projectId":a.pid.borrow().clone(),"epoch":preview["epoch"],"sequence":preview["sequence"],"pointRevisionId":preview["pointRevisionId"],"entityId":obj["entityId"],"entityType":obj["entityType"],"revisionId":head["revisionId"],"expectedHeads":obj["currentHeadIds"]
                                }),move|r|match r{
                                    Ok(_)=>{
                                        if obj["entityType"]=="scene"{
                                            a_.reload_scene(obj["entityId"].as_str().unwrap());
                                        }a_.load_index();
                                    },Err(e)=>a_.error(&e)
                                });
                            });
                        });
                        apply.set_sensitive(recoverable);
                        list.pack_start(&row,false,false,0);
                    }
                }
                dialog.content_area().pack_start(&scroll(&list),true,true,0);
                
                dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn deleted(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.deleted();return;}
        let a=self.clone();
        self.client.rpc("deleted_scenes",json!({
            "projectId":self.pid.borrow().clone()
        }),move|reply|match reply{
            Ok(scenes)=>{
                let dialog=a.panel("Recover deleted scenes");
                let list=gtk::Box::new(gtk::Orientation::Vertical,6);
                for scene in scenes.as_array().into_iter().flatten(){
                    let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                    row.pack_start(&gtk::Label::new(scene["scene"]["title"].as_str()),true,true,0);
                    let scene=scene.clone();
                    let a_=a.clone();
                    button(&row,"Preview",move||{
                        let dialog=text_dialog(&a_.window,"Deleted scene",&novel_storage::plain_text(&scene["scene"]["document"]));
                        let a=desk::effective(&a_);
                        let scene_=scene.clone();
                        a_.client.rpc("scene_heads",json!({
                            "sceneId":scene["id"]
                        }),move|r|match r{
                            Ok(heads)=>{
                                let row=gtk::Box::new(gtk::Orientation::Horizontal,4);
                                let a_=a.clone();
                                button(&row,"Restore as a new revision",move||{
                                    let a=desk::effective(&a_);
                                    a_.client.rpc("restore_deleted_scene",json!({
                                        "sceneId":scene_["id"],"scene":scene_["scene"],"heads":heads["revisionIds"]
                                    }),move|r|match r{
                                        Ok(_)=>a.load_index(),Err(e)=>a.error(&e)
                                    });
                                });
                                dialog.content_area().pack_start(&row,false,false,6);
                                dialog.show_all();
                            },Err(e)=>a.error(&e)
                        });
                    });
                    list.pack_start(&row,false,false,0);
                }dialog.content_area().pack_start(&scroll(&list),true,true,0);
                
                dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn conflicts(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.conflicts();return;}
        if let Some(p)=self.context_pane(){
            let a=self.clone();
            let p_=p.clone();
            p.snapshot(move|r|{
                let local_document=match r{Ok(v)=>v,Err(e)=>{a.error(&e);return;}};
                let p=p_;
                let a_=a.clone();
                let id=p.scene_id.clone();
                a.client.rpc("load_scene",json!({"sceneId":id}),move|r|{
                    let current=match r{Ok(v)=>v,Err(e)=>{a_.error(&e);return;}};
                    let a=desk::effective(&a_);
                    a_.client.rpc("conflicts",json!({"sceneId":id}),move|r|match r{
                        Ok(conflicts)=>{
                            let dialog=a.panel("Choose a competing scene version");
                            let list=gtk::Box::new(gtk::Orientation::Vertical,6);
                            let ids=conflicts.as_array().into_iter().flatten().filter_map(|r|r["id"].as_str().map(str::to_owned)).collect::<Vec<_>>();
                            if ids.is_empty() && !p.dirty.get(){list.pack_start(&gtk::Label::new(Some("This scene has no competing versions.")),false,false,8);}
                            let mut candidates=vec![("Current saved draft".to_owned(),current["scene"].clone())];
                            if p.dirty.get(){
                                candidates.insert(0,("Retained local draft · save as a new revision".to_owned(),json!({"schemaVersion":1,"title":p.title.borrow().clone(),"document":local_document})));
                                list.pack_start(&gtk::Label::new(Some("Save the retained local draft first to keep it in history. You can then choose another saved version.")),false,false,8);
                            }
                            for revision in conflicts.as_array().into_iter().flatten(){
                                if revision["operation"]!="delete"{
                                    candidates.push((format!("Competing version · {}",revision["createdAt"].as_str().unwrap_or("Saved")),revision["payload"].clone()));
                                }
                            }
                            for (title,scene) in candidates{
                                let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                                row.pack_start(&gtk::Label::new(Some(&title)),true,true,6);
                                let a_=a.clone();let scene_=scene.clone();let ids=ids.clone();let current=current.clone();let id=id.clone();let p=p.clone();let local_document=local_document.clone();
                                button(&row,"Preview and choose",move||{
                                    let preview=text_dialog(&a_.window,&title,&novel_storage::plain_text(&scene_["document"]));
                                    let row=gtk::Box::new(gtk::Orientation::Horizontal,4);
                                    let a=desk::effective(&a_);let id=id.clone();let ids=ids.clone();let current=current.clone();let scene=scene_.clone();let preview_=preview.clone();let p=p.clone();let local_document=local_document.clone();
                                    let can_apply=(!p.dirty.get() && !ids.is_empty()) || (p.dirty.get() && scene["document"]==local_document);
                                    let apply=button(&row,"Use this version; keep all history",move||{
                                        if p.saving.get() || p.composing.get() || p.pending.get()!=0 || p.frame.borrow()["document"]!=local_document{
                                            a.error("The draft changed or is busy. Open a fresh conflict preview.");return;
                                        }
                                        if p.dirty.get() && scene["document"]!=local_document{
                                            a.error("Save the retained local draft before choosing another version.");return;
                                        }
                                        p.saving.set(true);p.widget.set_sensitive(false);
                                        let op=if ids.is_empty(){"save_scene"}else{"resolve_scene"};
                                        let args=if ids.is_empty(){json!({"sceneId":id,"scene":scene.clone(),"parent":current["revisionId"],"requestId":uuid::Uuid::new_v4().to_string()})}else{json!({"sceneId":id,"scene":scene.clone(),"parent":current["revisionId"],"conflictIds":ids})};
                                        let a_=a.clone();let id=id.clone();let preview=preview_.clone();let p=p.clone();
                                        let scene=scene.clone();let local_document=local_document.clone();
                                        a.client.rpc(op,args,move|r|{
                                            p.saving.set(false);p.widget.set_sensitive(true);
                                            match r{
                                                Ok(saved)=>{
                                                    if scene["document"]==local_document{
                                                        *p.saved_document.borrow_mut()=local_document.clone();*p.saved_title.borrow_mut()=p.title.borrow().clone();
                                                        if let Some(revision)=saved["revisionId"].as_str(){*p.parent.borrow_mut()=revision.into();}
                                                        p.dirty.set(false);
                                                    }
                                                    preview.close();a_.reload_scene(&id);a_.load_index();
                                                },Err(e)=>a_.error(&e)
                                            }
                                        });
                                    });
                                    apply.set_sensitive(can_apply);
                                    preview.content_area().pack_start(&row,false,false,6);preview.show_all();
                                });
                                list.pack_start(&row,false,false,0);
                            }
                            dialog.content_area().pack_start(&scroll(&list),true,true,0);dialog.show_all();
                        },Err(e)=>a.error(&e)
                    });
                });
            });
        }
    }
    fn review(self:&Rc<Self>){self.run_local_review();}
    fn details(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.details();return;}
        let Some(p)=self.context_pane()else{self.error("Open a scene to edit its details.");return;};
        let page=self.data_for_view("sceneMetadata");let planning=self.page("planning");let a=self.clone();
        self.when_loaded(page.clone(),move||{
            let app=a.clone();let page_=page.clone();
            a.navigate(move||{
                let mut value=page_.value.borrow().clone();
                if !value["scenes"].as_array().unwrap().iter().any(|s|s["sceneId"]==p.scene_id){
                    value["scenes"].as_array_mut().unwrap().push(json!({"sceneId":p.scene_id,"synopsis":"","status":"draft","viewpointId":null,"tags":[],"eventIds":[]}));page_.update(value);
                }
                let mut refs=app.references();for key in ["records","events"]{for item in planning.value.borrow()[key].as_array().into_iter().flatten(){if let Some(id)=item["id"].as_str(){refs.push((id.into(),item["name"].as_str().or_else(||item["title"].as_str()).unwrap_or("Record").into()));}}}
                page_.render(refs);app.install(&page_.widget,"Scene details",true);
            });
        });
    }
    fn new_translation(self:&Rc<Self>){self.translation_preview();}
    fn library(self:&Rc<Self>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.library();return;}
        let a=self.clone();
        self.client.rpc("list_projects",json!({}),move|r|match r{
            Ok(projects)=>{
                let dialog=a.panel("Novel and universe library");
                let list=gtk::Box::new(gtk::Orientation::Vertical,6);
                let app=a.clone();button(&list,"New novel",move||app.new_project());
                let a_=a.clone();
                button(&list,"New shared universe",move||{
                    let Some(title)=prompt(&a_.window,"Name the shared universe","")else{return;};
                    let a=desk::effective(&a_);
                    a_.client.rpc("create_universe",json!({"title":title}),move|r|match r{
                        Ok(project)=>a.refresh_projects(project["id"].as_str().map(str::to_owned)),Err(e)=>a.error(&e)
                    });
                });
                for project in projects.as_array().into_iter().flatten(){
                    let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                    row.pack_start(&gtk::Label::new(project["title"].as_str()),true,true,4);
                    let id=project["id"].as_str().unwrap_or("").to_owned();let a_=a.clone();let id_=id.clone();let d=dialog.clone();
                    button(&row,"Open",move||{a_.switch_project(id_.clone());let _=&d;});
                    let a_=a.clone();let projects=projects.clone();let title=project["title"].as_str().unwrap_or("Universe").to_owned();
                    button(&row,"Universe / series",move||a_.universe_library(id.clone(),title.clone(),projects.clone()));
                    list.pack_start(&row,false,false,0);
                }
                dialog.content_area().pack_start(&scroll(&list),true,true,0);dialog.show_all();
            },Err(e)=>a.error(&e)
        });
    }
    fn universe_library(self:&Rc<Self>,id:String,title:String,projects:Value){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.universe_library(id,title,projects);return;}
        let a=self.clone();
        self.client.rpc("load_project_data",json!({"projectId":id,"kind":"universeCatalogue"}),move|r|match r{
            Ok(snapshot) if !snapshot.is_null()=>{
                let dialog=a.panel(&format!("Shared universe · {title}"));
                let content=dialog.content_area();
                let series=gtk::ComboBoxText::new();series.append(Some(""),"Novels outside a series");
                for s in snapshot["payload"]["series"].as_array().into_iter().flatten(){series.append(s["id"].as_str(),s["title"].as_str().unwrap_or("Series"));}
                series.set_active(Some(0));content.pack_start(&series,false,false,4);
                let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                let a_=a.clone();let id_=id.clone();let snap=snapshot.clone();let d=dialog.clone();let projects_=projects.clone();let title_=title.clone();
                button(&row,"Add series",move||{
                    let Some(name)=prompt(&a_.window,"New series name","")else{return;};
                    let mut payload=snap["payload"].clone();payload["series"].as_array_mut().unwrap().push(json!({"id":uuid::Uuid::new_v4().to_string(),"title":name}));
                    a_.save_catalogue(id_.clone(),title_.clone(),projects_.clone(),payload,snap["revisionId"].clone(),d.clone());
                });
                let a_=a.clone();let id_=id.clone();let snap=snapshot.clone();let series_=series.clone();let d=dialog.clone();
                button(&row,"New novel in selected series",move||{
                    let Some(name)=prompt(&a_.window,"New novel name","")else{return;};
                    let sid=series_.active_id().filter(|s|!s.is_empty()).map(|s|s.to_string());let a=desk::effective(&a_);let d=d.clone();
                    a_.client.rpc("create_universe_novel",json!({"universeId":id_,"seriesId":sid,"title":name,"parent":snap["revisionId"]}),move|r|match r{
                        Ok(created)=>{d.close();a.refresh_projects(created["project"]["id"].as_str().map(str::to_owned));},Err(e)=>a.error(&e)
                    });
                });
                content.pack_start(&row,false,false,4);
                let existing=gtk::ComboBoxText::new();
                for p in projects.as_array().into_iter().flatten().filter(|p|p["id"]!=id){existing.append(p["id"].as_str(),p["title"].as_str().unwrap_or("Novel"));}
                existing.set_active(Some(0));content.pack_start(&existing,false,false,4);
                let row=gtk::Box::new(gtk::Orientation::Horizontal,6);
                let a_=a.clone();let id_=id.clone();let snap=snapshot.clone();let d=dialog.clone();let projects_=projects.clone();let title_=title.clone();let series_=series.clone();
                button(&row,"Add existing novel to selected series",move||{
                    let Some(novel)=existing.active_id()else{return;};
                    let sid=series_.active_id().filter(|s|!s.is_empty()).map(|s|s.to_string());let mut payload=snap["payload"].clone();
                    let novels=payload["novels"].as_array_mut().unwrap();
                    if let Some(member)=novels.iter_mut().find(|n|n["projectId"]==novel.as_str()){member["seriesId"]=json!(sid);}else{novels.push(json!({"projectId":novel.as_str(),"seriesId":sid}));}
                    a_.save_catalogue(id_.clone(),title_.clone(),projects_.clone(),payload,snap["revisionId"].clone(),d.clone());
                });
                let a_=a.clone();let id_=id.clone();
                button(&row,"Shared world notes",move||{
                    if a_.extra_data.borrow().iter().any(|p|p.dirty.get()){
                        a_.error("Save the open shared notes before opening another world-notes form.");return;
                    }
                    let page=DataPage::new(a_.client.clone(),"planning");
                    page.load(&id_,Vec::new());a_.extra_data.borrow_mut().push(page.clone());
                    a_.install(&page.widget,"Shared world notes",true);
                });
                content.pack_start(&row,false,false,4);
                let list=gtk::Box::new(gtk::Orientation::Vertical,4);
                for member in snapshot["payload"]["novels"].as_array().into_iter().flatten(){
                    let name=projects.as_array().into_iter().flatten().find(|p|p["id"]==member["projectId"]).and_then(|p|p["title"].as_str()).unwrap_or("Novel unavailable on this computer");
                    let series_name=snapshot["payload"]["series"].as_array().into_iter().flatten().find(|s|s["id"]==member["seriesId"]).and_then(|s|s["title"].as_str()).unwrap_or("Outside a series");
                    list.pack_start(&gtk::Label::new(Some(&format!("{series_name} · {name}"))),false,false,4);
                }
                content.pack_start(&scroll(&list),true,true,0);dialog.show_all();
            },Ok(_)=>a.error("This is a novel. Choose a shared universe, or create one in the Library."),Err(e)=>a.error(&e)
        });
    }
    fn save_catalogue(self:&Rc<Self>,id:String,title:String,projects:Value,payload:Value,parent:Value,dialog:Rc<Panel>){
        let target=desk::effective(self);if !Rc::ptr_eq(&target,self){target.save_catalogue(id,title,projects,payload,parent,dialog);return;}
        let a=self.clone();
        self.client.rpc("save_project_data",json!({"projectId":id,"kind":"universeCatalogue","payload":payload,"parent":parent}),move|r|match r{
            Ok(_)=>{dialog.close();a.universe_library(id,title,projects);},Err(e)=>a.error(&e)
        });
    }
}
