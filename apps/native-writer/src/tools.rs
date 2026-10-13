//! Native, structured project forms. Drafts retain their exact parent revision.
use crate::process::Process;
use gtk::prelude::*;
use serde_json::{
    json,Value
};
use std::{
    cell::{
        Cell,RefCell
    },rc::Rc
};
pub struct DataPage {
    pub widget:gtk::Box,body:gtk::Box,status:gtk::Label,pub kind:String,     pub project_id:RefCell<String>,pub value:Rc<RefCell<Value>>,pub parent:RefCell<Value>,     pub dirty:Rc<Cell<bool>>,pub epoch:Rc<Cell<u64>>,pub saving:Cell<bool>,pub loaded:Cell<bool>,saved_value:RefCell<Value>,client:Process,
}
fn id()->String{
    uuid::Uuid::new_v4().to_string()
}
pub fn initial(kind:&str)->Value{
    match kind{
        "planning"=>json!({
            "schemaVersion":1,"fields":[],"records":[],"plotPoints":[],"facts":[],"boards":[{
                "id":id(),"name":"Story board","cards":[],"edges":[],"groups":[]
            }],"lanes":[{
                "id":id(),"name":"Main plot","kind":"main"
            }],"events":[],"constraints":[],"issues":[],"readerReveals":[],"knowledge":[]
        }),     "writingSystems"=>json!({
            "schemaVersion":1,"systems":[],"assignments":[]
        }),     "sceneTranslations"=>json!({
            "schemaVersion":1,"drafts":[]
        }),     "sceneMetadata"|"paragraphAlternatives"=>json!({
            "schemaVersion":1,"scenes":[]
        }),     "languagePacks"=>json!({
            "schemaVersion":1,"packs":[]
        }),     "assistanceJobs"=>json!({
            "schemaVersion":1,"jobs":[]
        }),     "settings"=>json!({
            "schemaVersion":1,"assistance":{
                "schemaVersion":1,"providers":[{
                    "kind":"languagetool","endpoint":"http://127.0.0.1:8081"
                },{
                    "kind":"llm","endpoint":"http://127.0.0.1:8080"
                }],"exceptions":[],"suggestions":[]
            }
        }),     "exportProfile"=>serde_json::to_value(novel_storage::ExportProfile::default()).unwrap(),     _=>json!({
            "schemaVersion":1
        }),
    }
}
fn template(key:&str)->Option<Value>{
    let identity=id();
    Some(match key{
        "records"=>json!({
            "id":identity,"kind":"character","name":"New character","aliases":[],"notes":"","values":{
            },"links":[]
        }),     "fields"=>json!({
            "id":identity,"label":"New field","kind":"text","recordKind":"character","hidden":false,"options":[]
        }),     "facts"=>json!({
            "id":identity,"subjectId":"","property":"","value":"","status":"proposal","confirmed":false,"observerId":null,"source":"","eventId":null,"untilEventId":null
        }),     "events"=>json!({
            "id":identity,"title":"New event","notes":"","laneIds":[],"sceneIds":[],"time":{
                "kind":"unknown"
            }
        }),     "plotPoints"=>json!({
            "id":identity,"title":"New plot point","notes":"","links":[],"requiresPayoff":false,"scopeClosed":false,"payoffId":null
        }),     "lanes"=>json!({
            "id":identity,"name":"New subplot","kind":"subplot"
        }),     "boards"=>json!({
            "id":identity,"name":"New board","cards":[],"edges":[],"groups":[]
        }),     "cards"=>json!({
            "id":identity,"title":"New card","body":"","kind":"note","reference":null,"x":0,"y":0,"groupId":null,"labels":[]
        }),     "groups"=>json!({
            "id":identity,"name":"New group"
        }),     "edges"=>json!({
            "id":identity,"fromId":"","toId":"","kind":"reference","label":"","confirmed":false
        }),     "constraints"=>json!({
            "id":identity,"beforeId":"","afterId":"","minimumDays":0,"strict":false,"confirmed":false,"notes":""
        }),     "issues"=>json!({
            "id":identity,"category":"Continuity","description":"","importance":"medium","links":[],"evidence":[],"status":"open","resolution":""
        }),     "readerReveals"=>json!({
            "id":identity,"eventId":null,"factId":null,"scene":{
                "kind":"scene","id":""
            },"position":0,"notes":""
        }),     "knowledge"=>json!({
            "id":identity,"characterId":"","factId":"","stance":"knows","factSnapshot":{
                "subjectId":"","property":"","value":""
            },"interpretation":"","source":"","scope":{
                "kind":"scenes","sceneIds":[]
            }
        }),     "systems"=>json!({
            "schemaVersion":1,"id":identity,"mappingVersion":1,"name":"New alphabet","scriptId":"custom","encoding":"private-use","direction":"ltr","mappings":[],"font":null,"glossary":[],"notes":""
        }),     "mappings"=>json!({
            "input":"","output":"","transliteration":""
        }),     "glossary"=>json!({
            "symbol":"","meaning":""
        }),     "assignments"=>json!({
            "scene":{
                "kind":"scene","id":""
            },"definitionId":"","mappingVersion":1
        }),     "exceptions"=>json!({
            "term":"","language":"en","reason":""
        }),     "links"=>json!({
            "kind":"scene","id":""
        }),     "evidence"=>json!({
            "id":identity,"link":null,"quote":"","sourceRevision":"","notes":""
        }),     _=>return None,
    })
}
#[derive(Clone)]enum Part{
    Key(String),Index(usize)
}
fn at<'a>(mut value:&'a Value,path:&[Part])->&'a Value{
    for p in path{
        value=match p{
            Part::Key(k)=>&value[k],Part::Index(i)=>&value[*i]
        };
    }value
}
fn set(value:&mut Value,path:&[Part],replacement:Value){
    if path.is_empty(){
        *value=replacement;
        return;
    }let mut parent=value;
    for p in &path[..path.len()-1]{
        parent=match p{
            Part::Key(k)=>&mut parent[k],Part::Index(i)=>&mut parent[*i]
        };
    }match path.last().unwrap(){
        Part::Key(k)=>parent[k]=replacement,Part::Index(i)=>parent[*i]=replacement
    }
}
fn label(key:&str)->String{
    let mut text=String::new();
    for (i,c) in key.chars().enumerate(){
        if c.is_uppercase() && i>0{
            text.push(' ');
        }text.push(if i==0{
            c.to_ascii_uppercase()
        }else{
            c.to_ascii_lowercase()
        });
    }text
}
fn map_kind_is_reference(value:&Value,path:&[Part])->bool{
    matches!(at(value,path)["kind"].as_str(),Some("scene"|"record"|"event"|"plotPoint"|"fact"))
}
fn enums(key:&str,current:&str)->Vec<&'static str>{
    match key{
        "kind" if ["text","long-text","number","date","single-choice","multiple-choice","links"].contains(&current)=>vec!["text","long-text","number","date","single-choice","multiple-choice","links"],
        "kind" if ["scene","record","event","plotPoint","fact"].contains(&current)=>vec!["scene","record","event","plotPoint","fact"],
        "lang"|"language"|"sourceLanguage"|"targetLanguage"=>vec!["en","ar","zh-Hans","zh-Hant","und"],     "dir"|"direction"=>vec!["auto","ltr","rtl"],"pageSize"=>vec!["letter","a4"],"lineSpacing"=>vec!["source","1","1.15","1.5","2"],     "stance"=>vec!["knows","believes","misunderstands"],"encoding"=>vec!["unicode","private-use","font-presentation"],"importance"=>vec!["low","medium","high"],     "status" if ["fact","belief","rumor","proposal"].contains(&current)=>vec!["fact","belief","rumor","proposal"],     "status" if ["draft","revising","complete"].contains(&current)=>vec!["draft","revising","complete"],     "status" if ["open","investigating","intentional","resolved","dismissed","stale"].contains(&current)=>vec!["open","investigating","intentional","resolved","dismissed","stale"],     "kind"|"recordKind" if ["character","location","item"].contains(&current)=>vec!["character","location","item"],     "kind" if ["main","subplot"].contains(&current)=>vec!["main","subplot"],     _=>vec![],
    }
}
#[derive(Clone)]struct Form{
    value:Rc<RefCell<Value>>,dirty:Rc<Cell<bool>>,epoch:Rc<Cell<u64>>,references:Rc<Vec<(String,String)>>
}
impl Form{
    fn change(&self,path:&[Part],value:Value){
        set(&mut self.value.borrow_mut(),path,value);
        self.dirty.set(true);
        self.epoch.set(self.epoch.get()+1);
    }
    fn render(&self,path:Vec<Part>,key:&str,depth:usize)->gtk::Widget{
        let value=at(&self.value.borrow(),&path).clone();
        let box_=gtk::Box::new(gtk::Orientation::Vertical,5);
        box_.set_margin_start(if depth>0{
            8
        }else{
            0
        });
        if depth>12{
            box_.pack_start(&gtk::Label::new(Some("This nested record can be read from its project archive.")),false,false,0);
            return box_.upcast();
        }
        match value{
            Value::Object(map)=>{
                if key=="time" && map.contains_key("kind"){
                    let kind=gtk::ComboBoxText::new();
                    for name in ["unknown","fixed","interval","bounds","relative"]{kind.append(Some(name),name);}
                    kind.set_active_id(map["kind"].as_str());
                    box_.pack_start(&kind,false,false,0);
                    let body=gtk::Box::new(gtk::Orientation::Vertical,4);
                    for (name,_) in &map{
                        if name=="kind"{continue;}
                        let mut p=path.clone();p.push(Part::Key(name.clone()));
                        body.pack_start(&gtk::Label::new(Some(&label(name))),false,false,0);
                        body.pack_start(&self.render(p,name,depth+1),false,false,0);
                    }
                    let form=self.clone();let path_=path.clone();let body_=body.clone();
                    kind.connect_changed(move|c|if let Some(kind)=c.active_id(){
                        let time=match kind.as_str(){
                            "fixed"=>json!({"kind":"fixed","day":0,"label":""}),
                            "interval"|"bounds"=>json!({"kind":kind.as_str(),"start":0,"end":0,"label":""}),
                            "relative"=>json!({"kind":"relative","eventId":"","offsetDays":0}),
                            _=>json!({"kind":"unknown"})
                        };
                        form.change(&path_,time.clone());
                        for child in body_.children(){body_.remove(&child);}
                        for (name,_) in time.as_object().unwrap(){
                            if name=="kind"{continue;}
                            let mut p=path_.clone();p.push(Part::Key(name.clone()));
                            body_.pack_start(&gtk::Label::new(Some(&label(name))),false,false,0);
                            body_.pack_start(&form.render(p,name,depth+1),false,false,0);
                        }
                        body_.show_all();
                    });
                    box_.pack_start(&body,false,false,0);
                    return box_.upcast();
                }
                if key=="values"{
                    let mut record_path=path.clone();record_path.pop();
                    let kind=at(&self.value.borrow(),&record_path)["kind"].as_str().unwrap_or("character").to_owned();
                    let fields=self.value.borrow()["fields"].as_array().cloned().unwrap_or_default();
                    for field in fields.iter().filter(|field|field["recordKind"]==kind && field["hidden"]!=true){
                        let Some(id)=field["id"].as_str() else{continue;};
                        let mut p=path.clone();p.push(Part::Key(id.to_owned()));
                        if at(&self.value.borrow(),&p).is_null(){
                            let initial=match field["kind"].as_str(){Some("number")=>json!(0),Some("multiple-choice"|"links")=>json!([]),_=>json!("")};
                            let add=gtk::Button::with_label(&format!("Set {}",field["label"].as_str().unwrap_or("field")));
                            let form=self.clone();let p=p.clone();let holder=gtk::Box::new(gtk::Orientation::Vertical,2);let holder_=holder.clone();
                            add.connect_clicked(move|button|{
                                form.change(&p,initial.clone());button.hide();
                                holder_.pack_start(&form.render(p.clone(),"value",depth+1),false,false,0);holder_.show_all();
                            });
                            box_.pack_start(&add,false,false,0);box_.pack_start(&holder,false,false,0);
                        }else{
                            box_.pack_start(&gtk::Label::new(field["label"].as_str()),false,false,0);
                            box_.pack_start(&self.render(p,"value",depth+1),false,false,0);
                        }
                    }
                    return box_.upcast();
                }
                for (key,value) in map{
                    if key=="schemaVersion"{
                        continue;
                    }let mut p=path.clone();
                    p.push(Part::Key(key.clone()));
                    if (key=="id" && !(map_kind_is_reference(&self.value.borrow(),&path))) || key.ends_with("RevisionId") || key=="documentHash"{
                        continue;
                    }
                    let caption=gtk::Label::new(Some(&label(&key)));
                    caption.set_xalign(0.);
                    box_.pack_start(&caption,false,false,0);
                    if value.is_array() || value.is_object(){
                        let expander=gtk::Expander::new(Some(&format!("Open {}",label(&key))));
                        let form=self.clone();
                        let p=p.clone();
                        expander.connect_expanded_notify(move|e|if e.is_expanded() && e.child().is_none(){
                            e.add(&form.render(p.clone(),&key,depth+1));
                            e.show_all();
                        });
                        box_.pack_start(&expander,false,false,0);
                    }
                    else{
                        box_.pack_start(&self.render(p,&key,depth+1),false,false,0);
                    }
                }
            }
            Value::Array(array)=>{
                if array.iter().all(Value::is_string) && template(key).is_none(){
                    let view=gtk::TextView::new();
                    view.set_wrap_mode(gtk::WrapMode::WordChar);
                    let buffer=view.buffer().unwrap();
                    buffer.set_text(&array.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"));
                    view.set_size_request(-1,80);
                    let form=self.clone();
                    let p=path.clone();
                    buffer.connect_changed(move|b|{
                        let text=b.text(&b.start_iter(),&b.end_iter(),true).unwrap_or_default();
                        form.change(&p,json!(text.lines().filter(|s|!s.trim().is_empty()).collect::<Vec<_>>()));
                    });
                    box_.pack_start(&view,false,false,0);
                }
                else{
                    let combo=gtk::ComboBoxText::new();
                    for (i,item) in array.iter().enumerate(){
                        let name=item["name"].as_str().or_else(||item["title"].as_str()).or_else(||item["label"].as_str()).or_else(||item["property"].as_str()).unwrap_or("Record");
                        combo.append(Some(&i.to_string()),&format!("{} · {}",i+1,name));
                    }box_.pack_start(&combo,false,false,0);
                    let inspector=gtk::Box::new(gtk::Orientation::Vertical,3);
                    let form=self.clone();
                    let p=path.clone();
                    let inspector_=inspector.clone();
                    let key_=key.to_owned();
                    combo.connect_changed(move|c|{
                        for child in inspector_.children(){
                            inspector_.remove(&child);
                        }if let Some(i)=c.active_id().and_then(|s|s.parse::<usize>().ok()){
                            let mut p=p.clone();
                            p.push(Part::Index(i));
                            inspector_.pack_start(&form.render(p,&key_,depth+1),false,false,0);
                            inspector_.show_all();
                        }
                    });
                    if let Some(template)=template(key){
                        let add=gtk::Button::with_label("Add");
                        let form=self.clone();
                        let p=path.clone();
                        let combo_=combo.clone();
                        let key_=key.to_owned();
                        add.connect_clicked(move|_|{
                            let mut values=at(&form.value.borrow(),&p).as_array().cloned().unwrap_or_default();
                            let mut item=template.clone();
                            if item.get("id").is_some(){
                                item["id"]=json!(id());
                            }let name=item["name"].as_str().or_else(||item["title"].as_str()).unwrap_or(&key_).to_owned();
                            values.push(item);
                            let index=values.len()-1;
                            form.change(&p,json!(values));
                            combo_.append(Some(&index.to_string()),&format!("{} · {}",index+1,name));
                            combo_.set_active_id(Some(&index.to_string()));
                        });
                        box_.pack_start(&add,false,false,0);
                        let remove=gtk::Button::with_label("Remove selected record");
                        let form=self.clone();let p=path.clone();let combo_=combo.clone();
                        remove.connect_clicked(move|_|{
                            let Some(index)=combo_.active_id().and_then(|s|s.parse::<usize>().ok())else{return;};
                            let mut values=at(&form.value.borrow(),&p).as_array().cloned().unwrap_or_default();
                            if index>=values.len(){return;}
                            values.remove(index);form.change(&p,json!(values));combo_.remove_all();
                            for (i,item) in values.iter().enumerate(){
                                let name=item["name"].as_str().or_else(||item["title"].as_str()).or_else(||item["label"].as_str()).unwrap_or("Record");
                                combo_.append(Some(&i.to_string()),&format!("{} · {}",i+1,name));
                            }
                            if !values.is_empty(){combo_.set_active(Some(0));}
                        });
                        box_.pack_start(&remove,false,false,0);
                    }
                    box_.pack_start(&inspector,false,false,0);
                    if !array.is_empty(){
                        combo.set_active(Some(0));
                    }
                }
            }
            Value::Bool(value)=>{
                let check=gtk::CheckButton::with_label("Yes");
                check.set_active(value);
                let form=self.clone();
                check.connect_toggled(move|c|form.change(&path,json!(c.is_active())));
                box_.pack_start(&check,false,false,0);
            }
            Value::Number(number)=>{
                let spin=gtk::SpinButton::with_range(-1_000_000.,1_000_000.,if number.is_f64(){
                    0.5
                }else{
                    1.
                });
                spin.set_digits(if number.is_f64(){
                    2
                }else{
                    0
                });
                spin.set_value(number.as_f64().unwrap_or(0.));
                let form=self.clone();
                let integral=!number.is_f64();
                spin.connect_value_changed(move|s|form.change(&path,if integral{
                    json!(s.value() as i64)
                }else{
                    json!(s.value())
                }));
                box_.pack_start(&spin,false,false,0);
            }
            Value::String(text)=>{
                let options=enums(key,&text);
                let immutable=["sceneId","sourceText","sourceLanguage","targetLanguage"].contains(&key) && path.iter().any(|p|matches!(p,Part::Key(k) if k=="drafts"));
                if (key.ends_with("Id") || key=="id") && !immutable{
                    let combo=gtk::ComboBoxText::new();
                    combo.append(Some(""),"Unspecified");
                    for (id,name) in self.references.iter(){
                        combo.append(Some(id),name);
                    }if !text.is_empty() && !self.references.iter().any(|(i,_)|i==&text){
                        combo.append(Some(&text),&text);
                    }combo.set_active_id(Some(&text));
                    let form=self.clone();
                    combo.connect_changed(move|c|if let Some(id)=c.active_id(){
                        form.change(&path,json!(id.as_str()));
                    });
                    box_.pack_start(&combo,false,false,0);
                }
                else if !options.is_empty() && !immutable{
                    let combo=gtk::ComboBoxText::new();
                    for option in options{
                        combo.append(Some(option),option);
                    }combo.set_active_id(Some(&text));
                    let form=self.clone();
                    combo.connect_changed(move|c|if let Some(s)=c.active_id(){
                        form.change(&path,json!(s.as_str()));
                    });
                    box_.pack_start(&combo,false,false,0);
                }
                else if text.contains('\n') || ["notes","body","text","sourceText","description","synopsis","explanation","resolution"].contains(&key){
                    let view=gtk::TextView::new();
                    view.set_editable(!immutable);
                    view.set_wrap_mode(gtk::WrapMode::WordChar);
                    let buffer=view.buffer().unwrap();
                    buffer.set_text(&text);
                    view.set_size_request(-1,120);
                    let form=self.clone();
                    buffer.connect_changed(move|b|form.change(&path,json!(b.text(&b.start_iter(),&b.end_iter(),true).unwrap_or_default().as_str())));
                    box_.pack_start(&view,false,false,0);
                }
                else{
                    let entry=gtk::Entry::new();
                    entry.set_text(&text);
                    entry.set_editable(!immutable);
                    let form=self.clone();
                    entry.connect_changed(move|e|form.change(&path,json!(e.text().as_str())));
                    box_.pack_start(&entry,false,false,0);
                }
            }
            Value::Null=>{
                if key.ends_with("Id"){
                    let combo=gtk::ComboBoxText::new();
                    combo.append(Some(""),"Unspecified");
                    for(id,name)in self.references.iter(){
                        combo.append(Some(id),name);
                    }combo.set_active(Some(0));
                    let form=self.clone();
                    combo.connect_changed(move|c|if let Some(s)=c.active_id(){
                        form.change(&path,if s.is_empty(){
                            Value::Null
                        }else{
                            json!(s.as_str())
                        });
                    });
                    box_.pack_start(&combo,false,false,0);
                }else{
                    box_.pack_start(&gtk::Label::new(Some("Unspecified")),false,false,0);
                }
            }
        }
        box_.upcast()
    }
}
impl DataPage{
    pub fn new(client:Process,kind:&str)->Rc<Self>{
        let widget=gtk::Box::new(gtk::Orientation::Vertical,8);
        widget.set_margin_start(12);
        widget.set_margin_end(12);
        widget.set_margin_top(12);
        let controls=gtk::Box::new(gtk::Orientation::Horizontal,6);
        let status=gtk::Label::new(Some("Open a novel to begin."));
        status.set_xalign(0.);status.set_line_wrap(true);
        controls.pack_start(&status,true,true,0);widget.pack_start(&controls,false,false,0);
        let scroll=gtk::ScrolledWindow::new(None::<&gtk::Adjustment>,None::<&gtk::Adjustment>);
        let body=gtk::Box::new(gtk::Orientation::Vertical,8);
        scroll.add(&body);
        widget.pack_start(&scroll,true,true,0);
        let page=Rc::new(Self{
            widget,body,status,kind:kind.into(),project_id:RefCell::new(String::new()),value:Rc::new(RefCell::new(initial(kind))),parent:RefCell::new(Value::Null),dirty:Rc::new(Cell::new(false)),epoch:Rc::new(Cell::new(0)),saving:Cell::new(false),loaded:Cell::new(false),saved_value:RefCell::new(initial(kind)),client
        });
        page
    }
    pub fn load(self:&Rc<Self>,project_id:&str,references:Vec<(String,String)>){
        if self.dirty.get(){
            self.status.set_text("Save this draft before changing novels.");
            return;
        }*self.project_id.borrow_mut()=project_id.into();
        self.loaded.set(false);
        self.status.set_text("Opening…");
        let weak=Rc::downgrade(self);
        let pid=project_id.to_owned();
        let current=pid.clone();let epoch=self.epoch.get();
        self.client.rpc("load_project_data",json!({
            "projectId":pid,"kind":self.kind
        }),move|reply|if let Some(p)=weak.upgrade(){
            if *p.project_id.borrow()!=current || p.epoch.get()!=epoch || p.dirty.get(){
                return;
            }match reply{
                Ok(snapshot)=>{
                    *p.value.borrow_mut()=if snapshot.is_null(){
                        initial(&p.kind)
                    }else{
                        snapshot["payload"].clone()
                    };
                    *p.parent.borrow_mut()=snapshot["revisionId"].clone();
                    *p.saved_value.borrow_mut()=p.value.borrow().clone();
                    p.loaded.set(true);
                    p.status.set_text("Saved");
                    p.render(references);
                },Err(e)=>p.status.set_text(&e)
            }
        });
    }
    pub fn render(&self,mut references:Vec<(String,String)>){
        for child in self.body.children(){
            self.body.remove(&child);
        }for key in ["records","events","plotPoints","lanes","systems"]{
            for item in self.value.borrow()[key].as_array().into_iter().flatten(){
                if let Some(id)=item["id"].as_str(){
                    references.push((id.to_owned(),item["name"].as_str().or_else(||item["title"].as_str()).unwrap_or("Record").to_owned()));
                }
            }
        }let form=Form{
            value:self.value.clone(),dirty:self.dirty.clone(),epoch:self.epoch.clone(),references:Rc::new(references)
        };
        self.body.pack_start(&form.render(Vec::new(),"",0),false,false,0);
        self.body.show_all();
    }
    pub fn save(self:&Rc<Self>,callback:impl FnOnce(Result<(),String>)+'static){
        if !self.dirty.get(){
            callback(Ok(()));
            return;
        }if !self.loaded.get() || self.saving.get(){
            callback(Err("Project form is busy. Wait for its saved confirmation.".into()));
            return;
        }
        self.saving.set(true);
        self.status.set_text("Saving…");
        let epoch=self.epoch.get();
        let value=self.value.borrow().clone();
        let saved_value=value.clone();
        let weak=Rc::downgrade(self);
        self.client.rpc("save_project_data",json!({
            "projectId":self.project_id.borrow().clone(),"kind":self.kind,"payload":value,"parent":self.parent.borrow().clone()
        }),move|reply|{
            if let Some(p)=weak.upgrade(){
                p.saving.set(false);
                match reply{
                    Ok(snapshot)=>{
                        *p.saved_value.borrow_mut()=saved_value;
                        *p.parent.borrow_mut()=snapshot["revisionId"].clone();
                        p.dirty.set(p.epoch.get()!=epoch);
                        p.status.set_text(if p.dirty.get(){
                            "More changes to save"
                        }else{
                            "Saved"
                        });
                        callback(Ok(()));
                    },Err(e)=>{
                        p.status.set_text(&format!("Save failed; draft retained. {e}"));
                        callback(Err(e));
                    }
                }
            }else{
                callback(Err("The project form was closed.".into()));
            }
        });
    }
    pub fn discard(&self){
        *self.value.borrow_mut()=self.saved_value.borrow().clone();self.dirty.set(false);self.epoch.set(self.epoch.get()+1);self.render(Vec::new());self.status.set_text("Changes discarded");
    }
    pub fn update(&self,value:Value){
        *self.value.borrow_mut()=value;
        self.dirty.set(true);
        self.epoch.set(self.epoch.get()+1);
        self.render(Vec::new());
    }
}
