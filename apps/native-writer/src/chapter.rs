//! A revision-bound chapter form hosted by the ordinary notebook draft lifecycle.
use super::*;
use novel_storage::ChapterEditSnapshot;

pub(super) struct ChapterPage {
    pub widget:gtk::Box,
    pub title:gtk::Entry,
    pub number:gtk::SpinButton,
    pub dirty:Cell<bool>,
    pub saving:Cell<bool>,
    pub composing:Cell<bool>,
    pub loaded:Cell<bool>,
    pub status:gtk::Label,
    project_id:String,
    chapter_id:String,
    controls:gtk::Box,
    snapshot:RefCell<Option<ChapterEditSnapshot>>,
    updating:Cell<bool>,
}
impl ChapterPage {
    fn new(project_id:String,chapter_id:String)->Rc<Self>{
        let widget=gtk::Box::new(gtk::Orientation::Vertical,12);
        widget.set_margin_start(32);widget.set_margin_end(32);widget.set_margin_top(26);widget.set_margin_bottom(20);
        let heading=gtk::Label::new(Some("Edit chapter"));heading.set_xalign(0.);heading.style_context().add_class("page-title");widget.pack_start(&heading,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Vertical,8);widget.pack_start(&controls,false,false,0);
        let title=gtk::Entry::new();let title_label=gtk::Label::new(Some("Title"));title_label.set_xalign(0.);title_label.set_mnemonic_widget(Some(&title));
        controls.pack_start(&title_label,false,false,0);controls.pack_start(&title,false,false,0);
        let number=gtk::SpinButton::with_range(0.,1.,1.);number.set_numeric(true);number.set_update_policy(gtk::SpinButtonUpdatePolicy::IfValid);
        let number_label=gtk::Label::new(Some("Chapter number"));number_label.set_xalign(0.);number_label.set_mnemonic_widget(Some(&number));
        controls.pack_start(&number_label,false,false,0);controls.pack_start(&number,false,false,0);
        let hint=gtk::Label::new(Some("Changing the number moves this chapter and renumbers the others.\nUse 0 for a prologue. Set the first chapter to 1 to start numbering at 1 again."));hint.set_xalign(0.);hint.set_line_wrap(true);controls.pack_start(&hint,false,false,0);
        let status=gtk::Label::new(Some("Loading saved chapter…"));status.set_xalign(0.);status.set_line_wrap(true);status.set_selectable(true);widget.pack_start(&status,false,false,0);
        let page=Rc::new(Self{widget,title,number,status,controls,project_id,chapter_id,dirty:Cell::new(false),saving:Cell::new(false),composing:Cell::new(false),loaded:Cell::new(false),snapshot:RefCell::new(None),updating:Cell::new(false)});
        let weak=Rc::downgrade(&page);page.title.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.changed();});
        let weak=Rc::downgrade(&page);page.number.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.changed();});
        let weak=Rc::downgrade(&page);page.title.connect_preedit_changed(move|_,text|if let Some(p)=weak.upgrade(){p.composing.set(!text.is_empty());});
        let row=gtk::Box::new(gtk::Orientation::Horizontal,8);page.controls.pack_start(&row,false,false,0);
        let weak=Rc::downgrade(&page);button(&row,"Save chapter",move||if let Some(p)=weak.upgrade(){
            if let Some(owner)=desk::workspace_for_widget(p.widget.upcast_ref()){p.save(&owner,|_|{});}
        });
        let weak=Rc::downgrade(&page);button(&row,"Discard changes",move||if let Some(p)=weak.upgrade(){if !p.saving.get() && !p.composing.get(){p.discard();}});
        page.controls.set_sensitive(false);page
    }
    fn changed(&self){
        if !self.updating.get() && self.loaded.get(){self.dirty.set(true);self.status.set_text("Unsaved chapter changes · Save to keep them");}
    }
    fn restore(&self,snapshot:&ChapterEditSnapshot){
        self.updating.set(true);self.title.set_text(&snapshot.chapter.title);
        self.number.set_range(0.,snapshot.expected_order.len() as f64);
        self.number.set_value((snapshot.chapter.position+snapshot.chapter_number_start) as f64);
        self.updating.set(false);self.dirty.set(false);
    }
    pub fn discard(&self){
        if let Some(snapshot)=self.snapshot.borrow().as_ref(){self.restore(snapshot);self.status.set_text("Chapter changes discarded.");}
    }
    fn load(self:&Rc<Self>,owner:&Rc<Workspace>,done:impl FnOnce()+'static){
        let weak=Rc::downgrade(self);
        owner.client.rpc("chapter_edit_snapshot",json!({"projectId":self.project_id,"chapterId":self.chapter_id}),move|reply|{
            let Some(p)=weak.upgrade()else{done();return;};
            if desk::workspace_for_widget(p.widget.upcast_ref()).is_none(){done();return;}
            match reply.and_then(|v|serde_json::from_value::<ChapterEditSnapshot>(v).map_err(|_|"Unsupported saved chapter.".into())){
                Ok(snapshot)=>{p.restore(&snapshot);*p.snapshot.borrow_mut()=Some(snapshot);p.loaded.set(true);p.controls.set_sensitive(true);p.status.set_text("Saved locally · Manual saving");},
                Err(e)=>{p.loaded.set(false);p.controls.set_sensitive(false);p.status.set_text(&format!("Could not load chapter. Reopen Edit chapter. {e}"));}
            }
            done();
        });
    }
    pub fn save(self:&Rc<Self>,owner:&Rc<Workspace>,callback:impl FnOnce(Result<(),String>)+'static){
        let result=(||{
            if self.saving.get(){return Err("The chapter is still saving.".to_owned());}
            if self.composing.get(){return Err("Finish entering the chapter title before saving.".to_owned());}
            if !self.dirty.get(){return Ok(None);}
            if owner.desk.is_closed() || *owner.pid.borrow()!=self.project_id{return Err("The novel changed. Your chapter changes stay in this page.".to_owned());}
            let capture=self.snapshot.borrow().clone().ok_or("Wait for the saved chapter to load.")?;
            let title=self.title.text().to_string();
            if title.trim().is_empty() || title.chars().count()>500{return Err("Enter a chapter title of 1–500 characters.".to_owned());}
            let number=self.number.text().parse::<i64>().map_err(|_|"Enter a whole chapter number.".to_owned())?;
            if number<0 || number>capture.expected_order.len() as i64{return Err("Chapter number is outside this manuscript's range.".to_owned());}
            Ok(Some(json!({"chapterId":self.chapter_id,"title":title,"number":number,"expectedChapterHead":capture.expected_chapter_head,"expectedProjectHead":capture.expected_project_head,"expectedOrder":capture.expected_order,"numberingParent":capture.numbering_parent})))
        })();
        let args=match result{Ok(Some(args))=>args,Ok(None)=>{callback(Ok(()));return;},Err(e)=>{self.status.set_text(&e);callback(Err(e));return;}};
        self.saving.set(true);self.controls.set_sensitive(false);self.status.set_text("Saving chapter…");
        let p=self.clone();
        owner.client.rpc("edit_chapter",args,move|reply|match reply{
            Ok(_)=>{
                p.dirty.set(false);p.loaded.set(false);
                if let Some(a)=desk::workspace_for_widget(p.widget.upcast_ref()){
                    a.load_index();let finished=p.clone();p.load(&a,move||{finished.saving.set(false);callback(Ok(()));});
                }else{p.saving.set(false);callback(Ok(()));}
            },
            Err(e)=>{p.saving.set(false);p.controls.set_sensitive(true);p.status.set_text(&format!("Chapter not saved; changes retained. {e}"));callback(Err(e));}
        });
    }
}
impl Workspace {
    pub(super) fn edit_chapter(self:&Rc<Self>){
        let a=desk::effective(self);
        let chapter=a.selected().and_then(|(id,kind)|if kind=="chapter"{Some(id)}else{
            a.index.borrow()["scenes"].as_array()?.iter().find(|s|s["id"]==id)?["chapterId"].as_str().map(str::to_owned)
        });
        let Some(chapter)=chapter else{a.error("Select a chapter or one of its scenes first.");return;};
        let out=a.clone();a.navigate(move||{
            let page=ChapterPage::new(out.pid.borrow().clone(),chapter);
            out.chapter_pages.borrow_mut().push(page.clone());out.install(&page.widget,"Edit chapter",true);page.load(&out,||{});
        });
    }
}
