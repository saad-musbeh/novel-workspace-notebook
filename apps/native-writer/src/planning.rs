//! Native plot board and deterministic chronology. The author supplies every link.
use crate::{
    app::prompt,tools::DataPage
};
use gtk::{
    gdk,prelude::*
};
use serde_json::{
    json,Value
};
use std::{
    cell::RefCell,rc::Rc
};
#[link(name="pangocairo-1.0")] extern "C"{
    fn pango_cairo_show_layout(context:*mut cairo::ffi::cairo_t,layout:*mut gtk::pango::ffi::PangoLayout);
}
fn text(area:&gtk::DrawingArea,context:&cairo::Context,value:&str,x:f64,y:f64,width:i32){
    use glib::translate::ToGlibPtr;
    let layout=area.create_pango_layout(Some(value));
    layout.set_width(width*gtk::pango::SCALE);
    layout.set_wrap(gtk::pango::WrapMode::WordChar);
    context.move_to(x,y);
    unsafe{
        pango_cairo_show_layout(context.to_raw_none(),layout.to_glib_none().0);
    }
}
const EDGE_KINDS:&[(&str,&str)]=&[("reference","Reference"),("causes","Causes"),("happens-before","Happens before"),("reveals","Reveals"),("depends-on","Depends on"),("setup/payoff","Setup/payoff"),("custom","Custom")];
fn edge_caption(edge:&Value)->String {
    let kind=edge["kind"].as_str().unwrap_or("reference");
    let kind=EDGE_KINDS.iter().find(|(id,_)|*id==kind).map(|(_,name)|*name).unwrap_or(kind);
    let label=edge["label"].as_str().unwrap_or("");
    if label.is_empty(){kind.into()}else{format!("{label} · {kind}")}
}
fn board_index(data:&Value,id:&str)->Option<usize>{data["boards"].as_array()?.iter().position(|b|b["id"].as_str()==Some(id))}
fn selected_board(data:&Value,combo:&gtk::ComboBoxText)->Option<usize>{board_index(data,combo.active_id()?.as_str())}
/// Validate against the exact card choices the author reviewed. No positions,
/// titles, reference objects, or other planning data are rewritten.
fn add_connection(data:&mut Value,board_id:&str,expected_cards:&Value,from:&str,to:&str,kind:&str,label:&str,identity:&str)->Result<(),String>{
    if from.is_empty()||to.is_empty()||from==to{return Err("Choose two different cards before connecting them.".into())}
    if !EDGE_KINDS.iter().any(|(id,_)|*id==kind){return Err("Choose a supported connection kind.".into())}
    if label.chars().count()>2_000{return Err("Connection labels are limited to 2,000 characters.".into())}
    if kind=="custom"&&label.trim().is_empty(){return Err("A custom connection needs an authored label.".into())}
    if identity.is_empty()||identity.len()>128{return Err("Invalid connection identity.".into())}
    let index=board_index(data,board_id).ok_or("The board changed. Refresh board and card choices.")?;
    let board=&mut data["boards"][index];
    if &board["cards"]!=expected_cards{return Err("The board cards changed. Refresh card choices before connecting them.".into())}
    let cards=board["cards"].as_array().ok_or("Invalid board card collection.")?;
    if [from,to].iter().any(|id|cards.iter().filter(|c|c["id"].as_str()==Some(*id)).count()!=1){return Err("A selected card is missing or ambiguous. Refresh card choices.".into())}
    let edges=board["edges"].as_array_mut().ok_or("Invalid board connection collection.")?;
    if edges.len()>=20_000{return Err("This board reached its connection limit.".into())}
    if edges.iter().any(|e|e["id"].as_str()==Some(identity)){return Err("This connection identity is already present.".into())}
    if edges.iter().any(|e|e["fromId"]==from&&e["toId"]==to&&e["kind"]==kind&&e["label"]==label){return Err("This authored connection already exists.".into())}
    edges.push(json!({"id":identity,"fromId":from,"toId":to,"kind":kind,"label":label,"confirmed":false}));Ok(())
}
fn draw_edge_caption(area:&gtk::DrawingArea,context:&cairo::Context,caption:&str,x:f64,y:f64){
    use glib::translate::ToGlibPtr;
    let layout=area.create_pango_layout(Some(caption));layout.set_width(240*gtk::pango::SCALE);layout.set_wrap(gtk::pango::WrapMode::WordChar);layout.set_height(-3);layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let(width,height)=layout.pixel_size();let x=(x-f64::from(width)/2.).clamp(4.,2552.);let y=(y-f64::from(height)/2.).clamp(4.,1696.);
    context.set_source_rgba(1.,1.,1.,0.96);context.rectangle(x-4.,y-3.,f64::from(width)+8.,f64::from(height)+6.);let _=context.fill();context.set_source_rgb(0.18,0.18,0.18);context.move_to(x,y);
    unsafe{pango_cairo_show_layout(context.to_raw_none(),layout.to_glib_none().0);}
}
pub fn board(page:Rc<DataPage>,parent:gtk::Window)->gtk::Box{
    let root=gtk::Box::new(gtk::Orientation::Vertical,6);
    // Desk mounts this first child in its existing Board tools sidebar.
    let controls=gtk::Box::new(gtk::Orientation::Vertical,6);
    let combo=gtk::ComboBoxText::new();combo.set_tooltip_text(Some("Board"));
    let refresh=gtk::Button::with_label("Refresh boards");let add=gtk::Button::with_label("Add note card");
    controls.pack_start(&combo,true,true,0);controls.pack_start(&refresh,false,false,0);controls.pack_start(&add,false,false,0);
    let source=gtk::ComboBoxText::new();source.set_tooltip_text(Some("Connection source"));
    let target=gtk::ComboBoxText::new();target.set_tooltip_text(Some("Connection target"));
    let kind=gtk::ComboBoxText::new();kind.set_tooltip_text(Some("Connection kind"));for (id,name) in EDGE_KINDS{kind.append(Some(id),name);}kind.set_active_id(Some("reference"));
    let label=gtk::Entry::new();label.set_placeholder_text(Some("Connection label"));label.set_max_length(2_000);
    let refresh_cards=gtk::Button::with_label("Refresh card choices");let connect=gtk::Button::with_label("Connect cards");let status=gtk::Label::new(Some("Connections stay in the draft until you Save."));status.set_xalign(0.);status.set_line_wrap(true);
    controls.pack_start(&source,false,false,0);controls.pack_start(&target,false,false,0);controls.pack_start(&kind,false,false,0);controls.pack_start(&label,false,false,0);controls.pack_start(&refresh_cards,false,false,0);controls.pack_start(&connect,false,false,0);controls.pack_start(&status,false,false,0);
    root.pack_start(&controls,false,false,0);
    let area=gtk::DrawingArea::new();area.set_size_request(2800,1800);area.add_events(gdk::EventMask::BUTTON_PRESS_MASK|gdk::EventMask::BUTTON_RELEASE_MASK|gdk::EventMask::POINTER_MOTION_MASK);
    let scroll=gtk::ScrolledWindow::new(None::<&gtk::Adjustment>,None::<&gtk::Adjustment>);scroll.add(&area);root.pack_start(&scroll,true,true,0);
    let capture=Rc::new(RefCell::new(None::<(String,Value)>));
    let p=page.clone();let c=combo.clone();let s=source.clone();let t=target.clone();let captured=capture.clone();let feedback=status.clone();
    let choices:Rc<dyn Fn()>=Rc::new(move||{
        let previous_s=s.active_id();let previous_t=t.active_id();s.remove_all();t.remove_all();s.append(Some(""),"Choose source card");t.append(Some(""),"Choose target card");
        let data=p.value.borrow();let Some(index)=selected_board(&data,&c)else{*captured.borrow_mut()=None;s.set_active(Some(0));t.set_active(Some(0));feedback.set_text("Choose an available board.");return;};
        let board=&data["boards"][index];*captured.borrow_mut()=Some((board["id"].as_str().unwrap_or("").into(),board["cards"].clone()));
        for card in board["cards"].as_array().into_iter().flatten().take(20_000){if let Some(id)=card["id"].as_str(){let title=card["title"].as_str().unwrap_or("Card");s.append(Some(id),title);t.append(Some(id),title);}}
        if !previous_s.as_deref().is_some_and(|id|s.set_active_id(Some(id))){s.set_active(Some(0));}if !previous_t.as_deref().is_some_and(|id|t.set_active_id(Some(id))){t.set_active(Some(0));}feedback.set_text("Choose two cards. Connecting adds an unsaved authored edge.");
    });
    let choose=choices.clone();refresh_cards.connect_clicked(move|_|choose());
    let p=page.clone();let c=combo.clone();let choose=choices.clone();let a=area.clone();
    refresh.connect_clicked(move|_|{
        let current=c.active_id();c.remove_all();for board in p.value.borrow()["boards"].as_array().into_iter().flatten(){c.append(board["id"].as_str(),board["name"].as_str().unwrap_or("Board"));}
        if !current.as_deref().is_some_and(|id|c.set_active_id(Some(id))){c.set_active(Some(0));}choose();a.queue_draw();
    });
    let choose=choices.clone();let a=area.clone();combo.connect_changed(move|_|{choose();a.queue_draw();});
    let p=page.clone();let s=source;let t=target;let k=kind;let l=label;let captured=capture;let feedback=status;let a=area.clone();
    connect.connect_clicked(move|_|{
        if !p.loaded.get()||p.saving.get(){feedback.set_text("Wait for the planning save to finish before connecting cards.");return;}
        let Some((board_id,cards))=captured.borrow().clone()else{feedback.set_text("Refresh board and card choices first.");return;};
        let from=s.active_id().unwrap_or_default();let to=t.active_id().unwrap_or_default();let kind=k.active_id().unwrap_or_default();let label=l.text();
        let result=add_connection(&mut p.value.borrow_mut(),&board_id,&cards,&from,&to,&kind,&label,&uuid::Uuid::new_v4().to_string());
        match result{Ok(())=>{p.dirty.set(true);p.epoch.set(p.epoch.get()+1);feedback.set_text("Connection added to draft. Ctrl+S saves; leaving offers Save / Discard / Cancel.");a.queue_draw();},Err(error)=>feedback.set_text(&error)}
    });
    let p=page.clone();let c=combo.clone();
    area.connect_draw(move|area,context|{
        let data=p.value.borrow();context.set_source_rgb(1.,1.,1.);let _=context.paint();let Some(index)=selected_board(&data,&c)else{return glib::Propagation::Proceed;};let board=&data["boards"][index];
        let cards=board["cards"].as_array().map(Vec::as_slice).unwrap_or(&[]);let mut captions=Vec::new();
        for edge in board["edges"].as_array().into_iter().flatten().take(20_000){
            let from=cards.iter().find(|c|c["id"]==edge["fromId"]);let to=cards.iter().find(|c|c["id"]==edge["toId"]);
            if let(Some(from),Some(to))=(from,to){
                let ax=from["x"].as_f64().unwrap_or(0.)+110.;let ay=from["y"].as_f64().unwrap_or(0.)+55.;let bx=to["x"].as_f64().unwrap_or(0.)+110.;let by=to["y"].as_f64().unwrap_or(0.)+55.;
                context.set_source_rgb(0.6,0.65,0.7);context.move_to(ax,ay);context.line_to(bx,by);let _=context.stroke();captions.push((edge_caption(edge),(ax+bx)/2.,(ay+by)/2.));
            }
        }
        for card in cards.iter().take(2000){
            let x=card["x"].as_f64().unwrap_or(20.);let y=card["y"].as_f64().unwrap_or(20.);context.set_source_rgb(0.98,0.98,0.98);context.rectangle(x,y,220.,110.);let _=context.fill();context.set_source_rgb(0.75,0.75,0.75);context.rectangle(x,y,220.,110.);let _=context.stroke();context.set_source_rgb(0.12,0.15,0.13);
            text(area,context,card["title"].as_str().unwrap_or("Card"),x+10.,y+8.,200);text(area,context,&card["body"].as_str().unwrap_or("").chars().take(110).collect::<String>(),x+10.,y+40.,200);
        }
        // Captions use full Pango bidi shaping and paint after cards so their
        // authored label/type cannot disappear behind a nearby card.
        for (caption,x,y) in captions{draw_edge_caption(area,context,&caption,x,y);}glib::Propagation::Proceed
    });
    let dragging=Rc::new(RefCell::new(None::<(String,String,f64,f64)>));let p=page.clone();let c=combo.clone();let d=dragging.clone();
    area.connect_button_press_event(move|_,event|{
        if !p.loaded.get()||p.saving.get(){return glib::Propagation::Stop;}let(x,y)=event.position();let data=p.value.borrow();let Some(index)=selected_board(&data,&c)else{return glib::Propagation::Stop;};let board=&data["boards"][index];
        if let Some(card)=board["cards"].as_array().into_iter().flatten().rev().find(|card|{let cx=card["x"].as_f64().unwrap_or(0.);let cy=card["y"].as_f64().unwrap_or(0.);x>=cx&&x<=cx+220.&&y>=cy&&y<=cy+110.}){if let(Some(board),Some(card_id))=(board["id"].as_str(),card["id"].as_str()){*d.borrow_mut()=Some((board.into(),card_id.into(),x-card["x"].as_f64().unwrap_or(0.),y-card["y"].as_f64().unwrap_or(0.)));}}glib::Propagation::Stop
    });
    let p=page.clone();let d=dragging.clone();
    area.connect_motion_notify_event(move|a,event|{
        if !p.loaded.get()||p.saving.get(){return glib::Propagation::Stop;}
        if let Some((board,id,dx,dy))=d.borrow().clone(){if event.state().contains(gdk::ModifierType::BUTTON1_MASK){let(x,y)=event.position();let mut data=p.value.borrow_mut();if let Some(board)=board_index(&data,&board){if let Some(card)=data["boards"][board]["cards"].as_array_mut().and_then(|cards|cards.iter_mut().find(|card|card["id"]==id)){card["x"]=json!((x-dx).clamp(0.,2580.));card["y"]=json!((y-dy).clamp(0.,1690.));p.dirty.set(true);p.epoch.set(p.epoch.get()+1);a.queue_draw();}}}}glib::Propagation::Stop
    });
    area.connect_button_release_event(move|_,_|{*dragging.borrow_mut()=None;glib::Propagation::Stop});
    let p=page;let a=area;let choose=choices;
    add.connect_clicked(move|button|{
        if !p.loaded.get()||p.saving.get(){return;}let parent=button.toplevel().and_then(|w|w.downcast::<gtk::Window>().ok()).unwrap_or_else(||parent.clone());let Some(title)=prompt(&parent,"New plot card","New note")else{return;};
        let mut data=p.value.borrow_mut();let Some(index)=selected_board(&data,&combo)else{return;};if let Some(cards)=data["boards"][index]["cards"].as_array_mut(){let i=cards.len();cards.push(json!({"id":uuid::Uuid::new_v4().to_string(),"title":title,"body":"","kind":"note","reference":null,"x":20+(i%8)*250,"y":20+(i/8)*135,"groupId":null,"labels":[]}));p.dirty.set(true);p.epoch.set(p.epoch.get()+1);a.queue_draw();}drop(data);choose();
    });
    refresh.clicked();root
}
pub fn chronology(document:&Value)->String{
    let events=document["events"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    fn range(event:&Value,events:&[Value],seen:&mut Vec<String>)->Option<(f64,f64)>{
        let id=event["id"].as_str()?.to_owned();
        if seen.contains(&id){
            return None;
        }seen.push(id);
        let time=&event["time"];
        match time["kind"].as_str()?{
            "fixed"=>{
                let day=time["day"].as_f64()?;
                Some((day,day))
            },"interval"|"bounds"=>Some((time["start"].as_f64()?,time["end"].as_f64()?)),"relative"=>{
                let linked=events.iter().find(|e|e["id"]==time["eventId"])?;
                let(a,b)=range(linked,events,seen)?;
                let offset=time["offsetDays"].as_f64()?;
                Some((a+offset,b+offset))
            },_=>None
        }
    }
    let mut output=String::new();
    for event in events{
        let resolved=range(event,events,&mut Vec::new());
        output+=&format!("{}: {}\n",event["title"].as_str().unwrap_or("Event"),resolved.map(|(a,b)|if a==b{
            format!("Day {a}")
        }else{
            format!("Days {a}–{b}")
        }).unwrap_or("Unknown or cyclic time".into()));
    }
    for constraint in document["constraints"].as_array().into_iter().flatten(){
        let a=events.iter().find(|e|e["id"]==constraint["beforeId"]);
        let b=events.iter().find(|e|e["id"]==constraint["afterId"]);
        let status=match a.zip(b).and_then(|(a,b)|Some((range(a,events,&mut Vec::new())?,range(b,events,&mut Vec::new())?))){
            Some(((a_start,a_end),(b_start,b_end)))=>{
                let gap=constraint["minimumDays"].as_f64().unwrap_or(0.);
                if b_end-a_start<gap{
                    "Contradiction"
                }else if b_start-a_end>=gap{
                    "Satisfied"
                }else{
                    "Uncertain"
                }
            },None=>"Unresolved"
        };
        output+=&format!("Constraint: {status} · {}\n",constraint["notes"].as_str().unwrap_or(""));
    }output
}
#[cfg(test)]mod tests{
    use super::*;
    fn fixture()->Value{json!({"boards":[{"id":"board","cards":[{"id":"a","title":"نور 明","x":20,"y":20,"reference":{"kind":"scene","id":"canonical-scene"}},{"id":"b","title":"Garden","x":320,"y":200}],"edges":[]}],"facts":[{"id":"untouched","value":"Exact مِ 中文"}],"events":[]})}
    #[test]fn authored_edge_captions_preserve_exact_multilingual_labels_and_kind(){
        let label="يَكشف 中文 e\u{301} \u{e000} 👩🏽‍🚀";let edge=json!({"kind":"reveals","label":label});assert_eq!(edge_caption(&edge),format!("{label} · Reveals"));assert_eq!(edge_caption(&json!({"kind":"setup/payoff","label":""})),"Setup/payoff");assert_eq!(edge["label"],label);
    }
    #[test]fn connecting_cards_changes_only_the_authored_edge_and_never_duplicates_or_infers(){
        let mut data=fixture();let original=data.clone();let cards=data["boards"][0]["cards"].clone();let label="يَكشف 中文 👩🏽‍🚀";add_connection(&mut data,"board",&cards,"a","b","reveals",label,"edge").unwrap();assert_eq!(data["boards"][0]["edges"][0],json!({"id":"edge","fromId":"a","toId":"b","kind":"reveals","label":label,"confirmed":false}));assert_eq!(data["boards"][0]["cards"],original["boards"][0]["cards"]);assert_eq!(data["facts"],original["facts"]);assert_eq!(data["events"],original["events"]);let saved=data.clone();assert!(add_connection(&mut data,"board",&cards,"a","b","reveals",label,"other-edge").is_err());assert_eq!(data,saved);
    }
    #[test]fn invalid_self_missing_and_stale_card_choices_refuse_without_mutation(){
        let mut data=fixture();let cards=data["boards"][0]["cards"].clone();let original=data.clone();for (from,to,kind,label) in [("a","a","reference",""),("a","missing","reference",""),("a","b","unsupported",""),("a","b","custom"," ")]{assert!(add_connection(&mut data,"board",&cards,from,to,kind,label,"edge").is_err());assert_eq!(data,original);}
        data["boards"][0]["cards"].as_array_mut().unwrap().remove(1);let removed=data.clone();assert!(add_connection(&mut data,"board",&cards,"a","b","reference","","edge").unwrap_err().contains("changed"));assert_eq!(data,removed);assert!(add_connection(&mut data,"removed-board",&cards,"a","b","reference","","edge").is_err());
    }
    #[test]fn time_cycles_and_interval_ambiguity_remain_unresolved(){
        let data=json!({
            "events":[{
                "id":"a","title":"A","time":{
                    "kind":"fixed","day":1
                }
            },{
                "id":"b","title":"B","time":{
                    "kind":"bounds","start":0,"end":2
                }
            },{
                "id":"c","title":"C","time":{
                    "kind":"relative","eventId":"c","offsetDays":1
                }
            }],"constraints":[{
                "beforeId":"a","afterId":"b","minimumDays":0,"notes":"Ambiguous"
            }]
        });
        let report=chronology(&data);
        assert!(report.contains("Uncertain"));
        assert!(report.contains("Unknown or cyclic"));
    }
}
