//! Interface layout preferences: a real bounded writing allocation, never image scaling.
use super::*;
use novel_storage::native_desk::{NativeDeskPreferences,NativeDeskDirection};

mod surface_imp {
    use super::*;
    use gtk::subclass::prelude::*;
    #[derive(Default)]
    pub struct AllocatedSurface {
        pub width:Cell<u32>,
        pub children:RefCell<Option<(gtk::DrawingArea,gtk::Scrollbar)>>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for AllocatedSurface {
        const NAME:&'static str="NovelAllocatedWritingSurface";
        type Type=super::AllocatedSurface;
        type ParentType=gtk::Fixed;
    }
    impl ObjectImpl for AllocatedSurface {}
    impl WidgetImpl for AllocatedSurface {
        fn preferred_width(&self)->(i32,i32){
            let bar=self.children.borrow().as_ref().map_or(1,|(_,bar)|bar.preferred_width().0.max(1));
            // A requested maximum is never a minimum; narrow windows can shrink.
            let minimum=240+bar;(minimum,minimum)
        }
        fn preferred_width_for_height(&self,_height:i32)->(i32,i32){self.preferred_width()}
        fn preferred_height(&self)->(i32,i32){(240,240)}
        fn preferred_height_for_width(&self,_width:i32)->(i32,i32){self.preferred_height()}
        fn size_allocate(&self,allocation:&gtk::Allocation){
            // Do not call GtkFixed's allocation: it first allocates its children
            // at natural 240px sizes, producing a second contradictory editor
            // refresh on every resize. Each child gets its final size only once.
            let object=self.obj();object.set_allocation(allocation);object.set_clip(allocation);
            if let Some((area,scrollbar))=self.children.borrow().as_ref(){
                let bar=scrollbar.preferred_width().0.max(1);let (left,width,bar_x)=surface_geometry(allocation.width(),bar,self.width.get());
                area.size_allocate(&gtk::Allocation::new(allocation.x()+left,allocation.y(),width,allocation.height()));
                scrollbar.size_allocate(&gtk::Allocation::new(allocation.x()+bar_x,allocation.y(),bar,allocation.height()));
            }
        }
    }
    impl ContainerImpl for AllocatedSurface {}
    impl FixedImpl for AllocatedSurface {}
}
glib::wrapper! {
    pub struct AllocatedSurface(ObjectSubclass<surface_imp::AllocatedSurface>)
        @extends gtk::Fixed,gtk::Container,gtk::Widget,
        @implements gtk::Buildable;
}
pub(crate) struct WritingSurface {
    pub widget:gtk::Fixed,
    allocated:AllocatedSurface,
    area:gtk::DrawingArea,
    scrollbar:gtk::Scrollbar,
}
/// The worker accepts 240–4096 pixels. This is also the actual area allocation.
fn surface_geometry(available:i32,scrollbar:i32,preference:u32)->(i32,i32,i32){
    let available=(available-scrollbar).max(240);let maximum=if preference==0{4096}else{preference as i32};let width=available.min(maximum).clamp(240,4096);
    ((available-width)/2,width,available)
}
impl WritingSurface {
    pub fn new(area:&gtk::DrawingArea,scrollbar:&gtk::Scrollbar)->Rc<Self>{
        use glib::subclass::prelude::ObjectSubclassIsExt;
        let allocated:AllocatedSurface=glib::Object::builder().build();
        let widget:gtk::Fixed=allocated.clone().upcast();widget.set_hexpand(true);widget.set_vexpand(true);widget.set_direction(gtk::TextDirection::Ltr);
        *allocated.imp().children.borrow_mut()=Some((area.clone(),scrollbar.clone()));
        widget.put(area,0,0);widget.put(scrollbar,0,0);
        Rc::new(Self{widget,allocated,area:area.clone(),scrollbar:scrollbar.clone()})
    }
    pub fn set_width(&self,width:u32){
        use glib::subclass::prelude::ObjectSubclassIsExt;
        if self.allocated.imp().width.replace(width)!=width{self.widget.queue_resize();}
    }
    pub fn apply(&self,preferences:&NativeDeskPreferences){self.set_width(preferences.writing_width);self.widget.set_direction(gtk::TextDirection::Ltr);self.area.set_direction(gtk::TextDirection::Ltr);self.scrollbar.set_direction(gtk::TextDirection::Ltr);}
}
pub(super) struct DesktopRuntime {
    value:RefCell<NativeDeskPreferences>,project_id:RefCell<String>,generation:Cell<u64>,pub loaded:Cell<bool>,
}
impl Default for DesktopRuntime {fn default()->Self{Self{value:RefCell::new(NativeDeskPreferences::default()),project_id:RefCell::new(String::new()),generation:Cell::new(0),loaded:Cell::new(false)}}}
fn direction(root:&gtk::Widget,value:gtk::TextDirection){root.set_direction(value);if let Ok(container)=root.clone().downcast::<gtk::Container>(){for child in container.children(){direction(&child,value);}}}
fn apply(owner:&Workspace,preferences:&NativeDeskPreferences){
    let value=match preferences.direction{NativeDeskDirection::Ltr=>gtk::TextDirection::Ltr,NativeDeskDirection::Rtl=>gtk::TextDirection::Rtl};direction(owner.window.upcast_ref(),value);
    for pane in owner.panes.borrow().values(){pane.surface.apply(preferences);}
}
fn publish(pid:&str,preferences:&NativeDeskPreferences){
    WINDOWS.with(|windows|for owner in windows.borrow().iter().filter(|a|*a.pid.borrow()==pid&&!a.desk.is_closed()){
        owner.desktop_preferences.generation.set(owner.desktop_preferences.generation.get()+1);*owner.desktop_preferences.project_id.borrow_mut()=pid.into();*owner.desktop_preferences.value.borrow_mut()=preferences.clone();owner.desktop_preferences.loaded.set(true);apply(owner,preferences);
    });
}
pub(super) struct DesktopPreferencesPage {
    pub widget:gtk::Box,pub direction:gtk::ComboBoxText,pub mode:gtk::ComboBoxText,pub width:gtk::SpinButton,
    pub dirty:Cell<bool>,pub saving:Cell<bool>,pub composing:Cell<bool>,pub loaded:Cell<bool>,pub status:gtk::Label,
    controls:gtk::Box,project_id:String,value:RefCell<Value>,parent:RefCell<Value>,confirmed:RefCell<NativeDeskPreferences>,updating:Cell<bool>,
}
impl DesktopPreferencesPage {
    fn new(project_id:String)->Rc<Self>{
        let widget=gtk::Box::new(gtk::Orientation::Vertical,12);widget.set_margin_start(32);widget.set_margin_end(32);widget.set_margin_top(26);widget.set_margin_bottom(20);
        let title=gtk::Label::new(Some("Writing view"));title.set_xalign(0.);title.style_context().add_class("page-title");widget.pack_start(&title,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Vertical,8);widget.pack_start(&controls,false,false,0);
        let direction=gtk::ComboBoxText::new();direction.append(Some("ltr"),"Left to right");direction.append(Some("rtl"),"Right to left");let label=gtk::Label::new(Some("Interface direction"));label.set_xalign(0.);label.set_mnemonic_widget(Some(&direction));controls.pack_start(&label,false,false,0);controls.pack_start(&direction,false,false,0);
        let mode=gtk::ComboBoxText::new();mode.append(Some("fill"),"Fill page");mode.append(Some("bounded"),"Limit writing width");let label=gtk::Label::new(Some("Writing width"));label.set_xalign(0.);label.set_mnemonic_widget(Some(&mode));controls.pack_start(&label,false,false,0);controls.pack_start(&mode,false,false,0);
        let width=gtk::SpinButton::with_range(360.,1200.,20.);width.set_numeric(true);width.set_update_policy(gtk::SpinButtonUpdatePolicy::IfValid);controls.pack_start(&width,false,false,0);
        let hint=gtk::Label::new(Some("A limited writing area is centered and shrinks with its window. Interface direction affects controls only; paragraph direction and original writing remain independent."));hint.set_xalign(0.);hint.set_line_wrap(true);controls.pack_start(&hint,false,false,0);
        let row=gtk::Box::new(gtk::Orientation::Horizontal,8);controls.pack_start(&row,false,false,0);let save=button(&row,"Save view",||{});let discard=button(&row,"Discard changes",||{});
        let status=gtk::Label::new(Some("Loading saved view…"));status.set_xalign(0.);status.set_line_wrap(true);status.set_selectable(true);widget.pack_start(&status,false,false,0);
        let p=Rc::new(Self{widget,controls,direction,mode,width,status,project_id,value:RefCell::new(json!({"schemaVersion":1})),parent:RefCell::new(Value::Null),confirmed:RefCell::new(NativeDeskPreferences::default()),updating:Cell::new(false),dirty:Cell::new(false),saving:Cell::new(false),composing:Cell::new(false),loaded:Cell::new(false)});
        let weak=Rc::downgrade(&p);p.direction.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.changed();});let weak=Rc::downgrade(&p);p.mode.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.width.set_sensitive(p.mode.active_id().as_deref()==Some("bounded"));p.changed();});
        let weak=Rc::downgrade(&p);p.width.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.changed();});let weak=Rc::downgrade(&p);p.width.connect_preedit_changed(move|_,text|if let Some(p)=weak.upgrade(){p.composing.set(!text.is_empty());});
        let weak=Rc::downgrade(&p);save.connect_clicked(move|_|if let Some(p)=weak.upgrade(){if let Some(owner)=p.owner(){p.save(&owner,|_|{});}});let weak=Rc::downgrade(&p);discard.connect_clicked(move|_|if let Some(p)=weak.upgrade(){if !p.saving.get()&&!p.composing.get(){p.discard();}});
        p.controls.set_sensitive(false);p
    }
    fn owner(&self)->Option<Rc<Workspace>>{desk::workspace_for_widget(self.widget.upcast_ref()).filter(|a|*a.pid.borrow()==self.project_id)}
    fn changed(&self){if self.loaded.get()&&!self.updating.get(){self.dirty.set(true);self.status.set_text("Unsaved view · Save to use it");}}
    fn restore(&self,value:&NativeDeskPreferences){self.updating.set(true);self.direction.set_active_id(Some(match value.direction{NativeDeskDirection::Ltr=>"ltr",NativeDeskDirection::Rtl=>"rtl"}));self.mode.set_active_id(Some(if value.writing_width==0{"fill"}else{"bounded"}));self.width.set_value(if value.writing_width==0{720.}else{value.writing_width as f64});self.width.set_sensitive(value.writing_width!=0);self.updating.set(false);self.dirty.set(false);}
    pub fn discard(&self){self.restore(&self.confirmed.borrow());self.status.set_text("View changes discarded.");}
    fn load(self:&Rc<Self>,owner:&Rc<Workspace>){let weak=Rc::downgrade(self);owner.client.rpc("load_project_data",json!({"projectId":self.project_id,"kind":"settings"}),move|reply|{
        let Some(p)=weak.upgrade()else{return;};if p.owner().is_none(){return;}
        match reply.and_then(|saved|{let value=if saved.is_null(){json!({"schemaVersion":1})}else{saved["payload"].clone()};let preferences=NativeDeskPreferences::from_project_settings(&value)?;Ok((saved["revisionId"].clone(),value,preferences))}){
            Ok((parent,value,preferences))=>{*p.parent.borrow_mut()=parent;*p.value.borrow_mut()=value;*p.confirmed.borrow_mut()=preferences.clone();p.restore(&preferences);p.loaded.set(true);p.controls.set_sensitive(true);p.width.set_sensitive(preferences.writing_width!=0);p.status.set_text("Saved locally · View applies to this novel's windows");},Err(e)=>p.status.set_text(&format!("Could not load view preferences. {e}")),
        }
    });}
    pub fn save(self:&Rc<Self>,owner:&Rc<Workspace>,callback:impl FnOnce(Result<(),String>)+'static){
        let result=(||{
            if self.saving.get(){return Err("View preferences are still saving.".to_owned());}if self.composing.get(){return Err("Finish entering the view preference before saving.".to_owned());}
            if !self.loaded.get(){return Err("Wait for the saved view to load.".to_owned());}if self.owner().is_none()||owner.desk.is_closed()||*owner.pid.borrow()!=self.project_id{return Err("The novel changed. View edits stay in this page.".to_owned());}
            let direction=match self.direction.active_id().as_deref(){Some("ltr")=>NativeDeskDirection::Ltr,Some("rtl")=>NativeDeskDirection::Rtl,_=>return Err("Choose an interface direction.".into())};
            let writing_width=match self.mode.active_id().as_deref(){Some("fill")=>0,Some("bounded")=>self.width.text().parse::<u32>().map_err(|_|"Enter a whole writing width.".to_owned())?,_=>return Err("Choose a writing width mode.".into())};
            let preferences=NativeDeskPreferences{schema_version:1,direction,writing_width};preferences.validate()?;Ok(preferences)
        })();
        let preferences=match result{Ok(p)=>p,Err(e)=>{self.status.set_text(&e);callback(Err(e));return;}};if !self.dirty.get(){callback(Ok(()));return;}
        let mut value=self.value.borrow().clone();value["nativeDesk"]=serde_json::to_value(&preferences).unwrap();self.saving.set(true);self.controls.set_sensitive(false);self.status.set_text("Saving view preferences…");let p=self.clone();
        owner.client.rpc("save_project_data_checked",json!({"projectId":self.project_id,"kind":"settings","payload":value,"parent":self.parent.borrow().clone(),"dependencies":[]}),move|reply|{
            p.saving.set(false);p.controls.set_sensitive(true);p.width.set_sensitive(preferences.writing_width!=0);match reply{
                Ok(saved)=>{*p.parent.borrow_mut()=saved["revisionId"].clone();*p.value.borrow_mut()=saved["payload"].clone();*p.confirmed.borrow_mut()=preferences.clone();p.dirty.set(false);publish(&p.project_id,&preferences);p.status.set_text("View preferences saved locally.");callback(Ok(()));},
                Err(e)=>{p.status.set_text(&format!("View not saved; changes retained. {e}"));callback(Err(e));}
            }
        });
    }
}
impl Workspace {
    pub(super) fn reload_desktop_preferences(self:&Rc<Self>){let pid=self.pid.borrow().clone();if pid.is_empty(){return;}let generation=self.desktop_preferences.generation.get()+1;self.desktop_preferences.generation.set(generation);self.desktop_preferences.loaded.set(false);*self.desktop_preferences.project_id.borrow_mut()=pid.clone();let weak=Rc::downgrade(self);
        self.client.rpc("load_project_data",json!({"projectId":pid,"kind":"settings"}),move|reply|{let Some(a)=weak.upgrade()else{return;};if a.desk.is_closed()||*a.pid.borrow()!=pid||a.desktop_preferences.generation.get()!=generation{return;}
            match reply.and_then(|saved|NativeDeskPreferences::from_project_settings(&saved["payload"])){Ok(preferences)=>{*a.desktop_preferences.value.borrow_mut()=preferences.clone();a.desktop_preferences.loaded.set(true);apply(&a,&preferences);},Err(e)=>a.error(&format!("Could not load writing view. {e}")),}
        });
    }
    pub(super) fn apply_desktop_to_pane(&self,pane:&EditorPane){
        let preferences=if *self.desktop_preferences.project_id.borrow()==*self.pid.borrow(){self.desktop_preferences.value.borrow().clone()}else{NativeDeskPreferences::default()};pane.surface.apply(&preferences);
    }
    pub(super) fn apply_desktop_to_widget(&self,widget:&gtk::Widget){
        let preferences=if *self.desktop_preferences.project_id.borrow()==*self.pid.borrow(){self.desktop_preferences.value.borrow().clone()}else{NativeDeskPreferences::default()};direction(widget,match preferences.direction{NativeDeskDirection::Ltr=>gtk::TextDirection::Ltr,NativeDeskDirection::Rtl=>gtk::TextDirection::Rtl});
        for pane in self.panes.borrow().values(){pane.surface.apply(&preferences);}
    }
    pub(super) fn writing_view(self:&Rc<Self>){let a=desk::effective(self);let out=a.clone();a.navigate(move||{let page=DesktopPreferencesPage::new(out.pid.borrow().clone());out.desktop_preference_pages.borrow_mut().push(page.clone());out.install(&page.widget,"Writing view",true);page.load(&out);});}
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn surface_allocates_true_pixel_width_without_enlarging_bitmap(){assert_eq!(surface_geometry(1000,16,0),(0,984,984));assert_eq!(surface_geometry(1000,16,720),(132,720,984));assert_eq!(surface_geometry(500,16,720),(0,484,484));assert_eq!(surface_geometry(256,16,360),(0,240,240));assert_eq!(surface_geometry(5016,16,0),(452,4096,5000));}
}
