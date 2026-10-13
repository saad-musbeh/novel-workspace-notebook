//! Saved-manuscript reading and explicit, local revision reminders.
use super::*;

#[derive(Clone,Debug)]
struct Passage {
    scene_id:String,
    revision_id:String,
    start:usize,
    end:usize,
}

/// GTK TextIter offsets and our anchors both count Unicode scalar values.
fn selection_anchor(passages:&[Passage],text:&str,start:usize,end:usize)->Option<(Passage,String,usize)> {
    if start>=end{return None;}
    let passage=passages.iter().find(|p|start>=p.start && end<=p.end)?;
    let quote=text.chars().skip(start).take(end-start).collect::<String>();
    if quote.trim().is_empty(){return None;}
    Some((passage.clone(),quote,start-passage.start))
}
fn reading_copy(bundle:&Value)->(String,Vec<Passage>) {
    let mut text=String::new();let mut passages=Vec::new();let mut offset=0;
    let mut scenes_by_chapter:HashMap<&str,Vec<&Value>>=HashMap::new();
    for scene in bundle["scenes"].as_array().into_iter().flatten(){
        if let Some(chapter)=scene["chapterId"].as_str(){scenes_by_chapter.entry(chapter).or_default().push(scene);}
    }
    for chapter in bundle["chapters"].as_array().into_iter().flatten(){
        let title=chapter["title"].as_str().unwrap_or("Chapter");text.push_str(title);text.push_str("\n\n");offset+=title.chars().count()+2;
        for scene in scenes_by_chapter.get(chapter["id"].as_str().unwrap_or("")).into_iter().flatten(){
            let title=scene["scene"]["title"].as_str().unwrap_or("Scene");text.push_str(title);text.push_str("\n\n");offset+=title.chars().count()+2;
            let start=offset;let body=novel_storage::plain_text(&scene["scene"]["document"]);offset+=body.chars().count();text.push_str(&body);
            passages.push(Passage{scene_id:scene["id"].as_str().unwrap_or("").into(),revision_id:scene["revisionId"].as_str().unwrap_or("").into(),start,end:offset});
            text.push_str("\n\n\n");offset+=3;
        }
    }
    (text,passages)
}
fn label(text:&str)->gtk::Label {
    let label=gtk::Label::new(Some(text));label.set_xalign(0.);label.set_line_wrap(true);label.set_selectable(true);label
}
/// A page may have moved to another window while its RPC was running.
fn page_owner(origin:&Rc<Workspace>,widget:&gtk::Box,pid:&str)->Option<Rc<Workspace>> {
    let mut result=None;
    desk::with_button_owner(widget.upcast_ref(),||{
        let a=desk::effective(origin);
        if !a.desk.is_closed() && a.pid.borrow().as_str()==pid && a.pages.page_num(widget).is_some(){result=Some(a);}
    });result
}
fn add_record(payload:&mut Value,key:&str,record:&Value)->Result<(),String>{
    let rows=payload[key].as_array_mut().ok_or("The saved notebook tools have an unsupported format.")?;
    if !rows.iter().any(|row|row["id"]==record["id"]){rows.push(record.clone());}Ok(())
}
fn resolve_record(payload:&mut Value,key:&str,id:&str)->Result<(),String>{
    let row=payload[key].as_array_mut().and_then(|rows|rows.iter_mut().find(|row|row["id"].as_str()==Some(id))).ok_or("This reminder has changed. Reopen Passages to revisit to load it again.")?;
    row["resolved"]=json!(true);Ok(())
}
fn rewrite_count(history:&Value)->usize {
    let revisions=history.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let by_id=revisions.iter().filter_map(|r|Some((r["id"].as_str()?,r))).collect::<HashMap<_,_>>();
    revisions.iter().filter(|revision|{
        if revision["operation"]!="edit" || revision["entityType"]!="scene"{return false;}
        let document=&revision["payload"]["document"];
        revision["parents"].as_array().into_iter().flatten().any(|parent|{
            let Some(old)=parent.as_str().and_then(|id|by_id.get(id))else{return false;};
            let before=&old["payload"]["document"];
            if old["entityType"]!="scene" || !before.is_object() || !document.is_object(){return false;}
            // An insertion-only edit retains every old scalar in order. This
            // linear check also avoids treating the large-diff fallback as a cut.
            let previous=novel_storage::plain_text(before);let current=novel_storage::plain_text(document);
            let mut remaining=current.chars();
            if previous.chars().all(|c|remaining.by_ref().any(|next|next==c)){return false;}
            !novel_storage::notebook_tools::removed_passages(before,document).is_empty()
        })
    }).count()
}

impl Workspace {
    pub(super) fn reader(self:&Rc<Self>){
        let a=desk::effective(self);let pid=a.pid.borrow().clone();
        if pid.is_empty(){a.error("Choose a novel before opening its reading copy.");return;}
        let generation=a.desk.generation.get();let app=a.clone();
        a.confirm_project(move|accepted|{
            if !accepted || app.desk.is_closed() || app.pid.borrow().as_str()!=pid || app.desk.generation.get()!=generation{return;}
            let panel=app.panel("Read it like a stranger");let loading=label("Preparing the saved reading copy…");panel.content_area().pack_start(&loading,false,false,0);panel.show_all();
            let origin=app.clone();let request_pid=pid.clone();
            app.client.rpc("open_project",json!({"projectId":pid}),move|reply|{
                let Some(owner)=page_owner(&origin,&panel.widget,&request_pid)else{return;};
                match reply{
                    Ok(bundle)=>{panel.content_area().remove(&loading);owner.populate_reader(&panel,&request_pid,&bundle);panel.widget.show_all();},
                    Err(error)=>loading.set_text(&format!("Could not load the reading copy. {error}"))
                }
            });
        });
    }
    fn populate_reader(self:&Rc<Self>,panel:&Rc<Panel>,pid:&str,bundle:&Value){
        let (text,passages)=reading_copy(bundle);
        let hint=label("A saved reading copy. Select a passage, then mark your reaction. Marks are saved immediately.");hint.style_context().add_class("notebook-hint");panel.content_area().pack_start(&hint,false,false,0);
        let row=gtk::Box::new(gtk::Orientation::Horizontal,8);panel.content_area().pack_start(&row,false,false,0);
        let feedback=label("");panel.content_area().pack_start(&feedback,false,false,0);
        let view=gtk::TextView::new();view.set_editable(false);view.set_cursor_visible(false);view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_left_margin(32);view.set_right_margin(32);view.set_top_margin(18);view.set_bottom_margin(28);view.set_pixels_above_lines(5);view.set_pixels_below_lines(8);
        let buffer=view.buffer().unwrap();buffer.set_text(&text);
        let tag=gtk::TextTag::builder().name("reader-body").family("serif").size_points(17.0).build();buffer.tag_table().unwrap().add(&tag);buffer.apply_tag(&tag,&buffer.start_iter(),&buffer.end_iter());
        panel.content_area().pack_start(&scroll(&view),true,true,0);
        let text=Rc::new(text);let passages=Rc::new(passages);let pending=Rc::new(Cell::new(false));
        for (kind,title) in [("bored","Bored"),("confused","Confused"),("curious","Curious")]{
            let origin=self.clone();let widget=panel.widget.clone();let pid=pid.to_owned();let buffer=buffer.clone();let passages=passages.clone();let text=text.clone();let feedback=feedback.clone();let pending=pending.clone();
            button(&row,title,move||{
                let Some(a)=page_owner(&origin,&widget,&pid)else{return;};
                if pending.get(){return;}
                let Some((start,end))=buffer.selection_bounds()else{feedback.set_text("Select a passage first.");return;};
                let Some((passage,quote,start))=selection_anchor(&passages,&text,start.offset() as usize,end.offset() as usize)else{feedback.set_text("Select text within one scene, without its heading.");return;};
                let record=json!({"id":uuid::Uuid::new_v4().to_string(),"sceneId":passage.scene_id,"revisionId":passage.revision_id,"quote":quote,"start":start,"kind":kind,"resolved":false});
                pending.set(true);feedback.set_text("Saving reaction…");
                let feedback=feedback.clone();let pending=pending.clone();let origin=a.clone();let widget=widget.clone();let pid=pid.clone();
                a.update_notebook_tools(move|value|add_record(value,"reactions",&record),move|reply|{
                    pending.set(false);if page_owner(&origin,&widget,&pid).is_none(){return;}
                    match reply{Ok(())=>feedback.set_text(&format!("Saved: {title}. Find this passage in Passages to revisit.")),Err(error)=>feedback.set_text(&format!("Reaction was not saved. Your selection is still here. {error}"))}
                });
            });
        }
    }
    pub(super) fn attention(self:&Rc<Self>){
        let a=desk::effective(self);let pid=a.pid.borrow().clone();
        if pid.is_empty(){a.error("Choose a novel before opening Passages to revisit.");return;}
        let next=a.clone();a.navigate(move||next.open_attention(pid));
    }
    fn open_attention(self:&Rc<Self>,pid:String){
        if self.desk.is_closed() || self.pid.borrow().as_str()!=pid{return;}
        let panel=self.panel("Passages to revisit");let loading=label("Loading your reminders…");panel.content_area().pack_start(&loading,false,false,0);panel.show_all();
        let origin=self.clone();self.load_notebook_tools(move|reply|{
            let Some(owner)=page_owner(&origin,&panel.widget,&pid)else{return;};
            match reply{
                Ok(data)=>{panel.content_area().remove(&loading);owner.populate_attention(&panel,&pid,&data);panel.widget.show_all();},
                Err(error)=>loading.set_text(&format!("Could not load reminders. {error}"))
            }
        });
    }
    fn populate_attention(self:&Rc<Self>,panel:&Rc<Panel>,pid:&str,data:&Value){
        let hint=label("Your unresolved reading reactions and notes, followed by scenes with at least two saved rewrites of existing text. These counts are a memory aid, not a judgement of your writing.");hint.style_context().add_class("notebook-hint");panel.content_area().pack_start(&hint,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Horizontal,8);panel.content_area().pack_start(&controls,false,false,0);
        let scenes=gtk::ComboBoxText::new();let references=self.references();for (id,title) in &references{scenes.append(Some(id),title);}scenes.set_active(Some(0));
        if let Some(p)=self.context_pane(){scenes.set_active_id(Some(&p.scene_id));}
        controls.pack_start(&scenes,true,true,0);
        let feedback=label("");panel.content_area().pack_start(&feedback,false,false,0);
        let list=gtk::Box::new(gtk::Orientation::Vertical,18);panel.content_area().pack_start(&scroll(&list),true,true,0);
        let count=data["reactions"].as_array().into_iter().flatten().chain(data["notes"].as_array().into_iter().flatten()).filter(|r|r["resolved"]!=true).count();
        let reminders=gtk::Box::new(gtk::Orientation::Vertical,18);list.pack_start(&reminders,false,false,0);
        let empty=label("No unresolved reactions or notes.");if count==0{reminders.pack_start(&empty,false,false,0);}
        for key in ["reactions","notes"]{
            for record in data[key].as_array().into_iter().flatten().filter(|r|r["resolved"]!=true){
                self.attention_card(&reminders,panel,pid,key,record,&references);
            }
        }
        let origin=self.clone();let widget=panel.widget.clone();let pid_=pid.to_owned();let pending=Rc::new(Cell::new(false));let list_=reminders.clone();let panel_=panel.clone();let references_=references.clone();
        button(&controls,"Add note",move||{
            let Some(a)=page_owner(&origin,&widget,&pid_)else{return;};if pending.get(){return;}
            let Some(scene)=scenes.active_id()else{feedback.set_text("Create a scene before adding a note.");return;};
            let Some(text)=prompt_with_action(&a.window,"Note for this scene","","Save note")else{return;};if text.trim().is_empty(){return;}
            let record=json!({"id":uuid::Uuid::new_v4().to_string(),"sceneId":scene.as_str(),"text":text,"resolved":false});let saved=record.clone();
            pending.set(true);feedback.set_text("Saving note…");let pending=pending.clone();let feedback=feedback.clone();let origin=a.clone();let widget=widget.clone();let pid=pid_.clone();let list=list_.clone();let panel=panel_.clone();let references=references_.clone();let empty=empty.clone();
            a.update_notebook_tools(move|value|add_record(value,"notes",&record),move|reply|{
                pending.set(false);let Some(owner)=page_owner(&origin,&widget,&pid)else{return;};
                match reply{
                    Ok(())=>{feedback.set_text("Note saved.");if empty.parent().is_some(){list.remove(&empty);}owner.attention_card(&list,&panel,&pid,"notes",&saved,&references);list.show_all();},
                    Err(error)=>feedback.set_text(&format!("Note was not saved: {error}\nYour note: {text}"))
                }
            });
        });
        let heading=label("Repeated saved rewrites");heading.style_context().add_class("page-title");list.pack_start(&heading,false,false,0);
        let history=gtk::Box::new(gtk::Orientation::Vertical,8);let progress=label("Counting saved rewrites…");history.pack_start(&progress,false,false,0);list.pack_start(&history,false,false,0);
        self.attention_histories(panel.widget.clone(),pid.to_owned(),references,0,history,progress,0);
    }
    fn attention_card(self:&Rc<Self>,list:&gtk::Box,panel:&Rc<Panel>,pid:&str,key:&str,record:&Value,references:&[(String,String)]){
        let card=gtk::Box::new(gtk::Orientation::Vertical,6);let scene=record["sceneId"].as_str().unwrap_or("");
        let title=references.iter().find(|(id,_)|id==scene).map(|(_,title)|title.as_str()).unwrap_or("Scene no longer in this novel");
        let kind=match record["kind"].as_str(){Some("bored")=>"Bored",Some("confused")=>"Confused",Some("curious")=>"Curious",_=>"Note"};
        card.pack_start(&label(&format!("{title} · {kind}")),false,false,0);card.pack_start(&label(record[if key=="reactions"{"quote"}else{"text"}].as_str().unwrap_or("")),false,false,0);
        let row=gtk::Box::new(gtk::Orientation::Horizontal,8);card.pack_start(&row,false,false,0);let feedback=label("");card.pack_start(&feedback,false,false,0);
        let origin=self.clone();let widget=panel.widget.clone();let pid_=pid.to_owned();let scene=scene.to_owned();let record_=record.clone();let key_=key.to_owned();
        button(&row,if key=="reactions"{"Open passage"}else{"Open scene"},move||{
            let Some(a)=page_owner(&origin,&widget,&pid_)else{return;};
            if key_=="reactions"{a.jump_to_passage(scene.clone(),record_["revisionId"].as_str().unwrap_or("").into(),record_["quote"].as_str().unwrap_or("").into(),record_["start"].as_u64().unwrap_or(0) as usize);}else{a.open_scene(&scene,false);}
        });
        let origin=self.clone();let widget=panel.widget.clone();let pid=pid.to_owned();let id=record["id"].as_str().unwrap_or("").to_owned();let key=key.to_owned();let card_=card.clone();let pending=Rc::new(Cell::new(false));
        button(&row,"Mark resolved",move||{
            let Some(a)=page_owner(&origin,&widget,&pid)else{return;};if pending.replace(true){return;}
            feedback.set_text("Saving…");let id=id.clone();let key=key.clone();let pending=pending.clone();let feedback=feedback.clone();let card=card_.clone();let origin=a.clone();let widget=widget.clone();let pid=pid.clone();
            a.update_notebook_tools(move|value|resolve_record(value,&key,&id),move|reply|{
                pending.set(false);if page_owner(&origin,&widget,&pid).is_none(){return;}
                match reply{Ok(())=>{feedback.set_text("Resolved.");card.set_sensitive(false);},Err(error)=>feedback.set_text(&format!("Could not resolve this reminder. {error}"))}
            });
        });list.pack_start(&card,false,false,0);
    }
    fn attention_histories(self:&Rc<Self>,widget:gtk::Box,pid:String,scenes:Vec<(String,String)>,index:usize,list:gtk::Box,progress:gtk::Label,shown:usize){
        if index>=scenes.len(){progress.set_text(if shown==0{"No scenes have two saved rewrites yet."}else{"Counts include saved edits that remove or replace existing text; additions, formatting, renames and restores are excluded."});return;}
        let origin=self.clone();let scene=scenes[index].0.clone();self.client.rpc("history",json!({"sceneId":scene}),move|reply|{
            let Some(owner)=page_owner(&origin,&widget,&pid)else{return;};
            let count=match reply{Ok(history)=>rewrite_count(&history),Err(error)=>{progress.set_text(&format!("Some history could not be counted. {error}"));return;}};
            let mut shown=shown;
            if count>=2{
                shown+=1;let row=gtk::Box::new(gtk::Orientation::Horizontal,8);row.pack_start(&label(&format!("{} · {count} saved rewrites",scenes[index].1)),true,true,0);
                let a=owner.clone();let widget_=widget.clone();let pid_=pid.clone();button(&row,"Open scene",move||if let Some(a)=page_owner(&a,&widget_,&pid_){a.open_scene(&scene,false);});list.pack_start(&row,false,false,0);row.show_all();
            }
            owner.attention_histories(widget,pid,scenes,index+1,list,progress,shown);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reading_selection_uses_scalar_offsets_and_rejects_headings_or_cross_scene_ranges(){
        let bundle=json!({"chapters":[{"id":"chapter","title":"Chapter"}],"scenes":[{"id":"scene","chapterId":"chapter","revisionId":"revision","scene":{"title":"Scene","document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"مرحبا 🌿 café"}]}]}}}]});
        let(text,passages)=reading_copy(&bundle);let p=&passages[0];
        let(anchor,quote,start)=selection_anchor(&passages,&text,p.start+6,p.start+7).unwrap();assert_eq!(quote,"🌿");assert_eq!(start,6);assert_eq!(anchor.revision_id,"revision");
        assert!(selection_anchor(&passages,&text,p.start-1,p.start+1).is_none());assert!(selection_anchor(&passages,&text,p.start,p.end+1).is_none());assert!(selection_anchor(&passages,&text,p.start,p.start).is_none());
    }
    #[test]
    fn reading_offsets_accumulate_across_multilingual_chapters_and_scenes(){
        fn scene(id:&str,chapter:&str,title:&str,text:&str)->Value{json!({"id":id,"chapterId":chapter,"revisionId":format!("rev-{id}"),"scene":{"title":title,"document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]}}})}
        let bundle=json!({"chapters":[{"id":"a","title":"العنوان 🌿"},{"id":"b","title":"第二章"}],"scenes":[scene("first","a","الفجر","مَرْحَبًا"),scene("other","b","出発","世界 🦊"),scene("second","a","Evening","e\u{301} café")]});
        let (text,passages)=reading_copy(&bundle);
        assert_eq!(passages.iter().map(|p|p.scene_id.as_str()).collect::<Vec<_>>(),vec!["first","second","other"]);
        for (passage,expected) in passages.iter().zip(["مَرْحَبًا","e\u{301} café","世界 🦊"]){
            let (_,quote,offset)=selection_anchor(&passages,&text,passage.start,passage.end).unwrap();assert_eq!(quote,expected);assert_eq!(offset,0);
        }
        assert_eq!(passages.last().unwrap().end+3,text.chars().count());
    }
    #[test]
    fn history_counts_rewrites_against_actual_parents_and_reminders_update_by_identity(){
        fn revision(id:&str,parent:Option<&str>,operation:&str,text:&str)->Value{
            json!({"id":id,"entityType":"scene","operation":operation,"parents":parent.into_iter().collect::<Vec<_>>(),"payload":{"document":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]}}})
        }
        let mut formatting=revision("format",Some("add"),"edit","A 界 B C");formatting["payload"]["document"]["content"][0]["content"][0]["marks"]=json!([{"type":"bold"}]);
        // Out-of-order history and a branch must use explicit parents, not neighbors.
        let history=json!([
            revision("rewrite2",Some("rewrite1"),"edit","A 狐 C"),
            revision("root",None,"create","A 界 B"),
            revision("add",Some("root"),"edit","A 界 B C"),formatting,
            revision("rewrite1",Some("format"),"edit","A 界 C"),
            revision("branch-add",Some("root"),"edit","A extra 界 B"),
            revision("rename",Some("rewrite2"),"rename","changed"),
            revision("restore",Some("rename"),"restore","A"),
            revision("missing",Some("absent"),"edit","unknown")
        ]);
        assert_eq!(rewrite_count(&history),2);
        let old="界".repeat(1000);let inserted=format!("{}م{}م{}",&old[..900],&old[900..1800],&old[1800..]);
        assert_eq!(rewrite_count(&json!([revision("large",None,"create",&old),revision("inserted",Some("large"),"edit",&inserted)])),0);
        let mut data=json!({"notes":[]});let note=json!({"id":"one","text":"Follow up","resolved":false});add_record(&mut data,"notes",&note).unwrap();add_record(&mut data,"notes",&note).unwrap();assert_eq!(data["notes"].as_array().unwrap().len(),1);resolve_record(&mut data,"notes","one").unwrap();assert_eq!(data["notes"][0]["resolved"],true);assert!(resolve_record(&mut data,"notes","missing").is_err());
    }
}
