//! Board viewport state stays with this widget; only explicit card movement
//! changes the shared layout. Filtering never removes cards or their story links.
use super::{add_connection,board_index,selected_board,draw_edge_caption,EDGE_KINDS,prompt};
use crate::tools::DataPage;
use gtk::{gdk,prelude::*};
use serde_json::{json,Value};
use std::{cell::{Cell,RefCell},collections::{BTreeMap,HashMap,HashSet},rc::Rc};
pub(super) const CARD_WIDTH:f64=220.;
pub(super) const CARD_HEIGHT:f64=110.;
const MAX_CARDS:usize=20_000;
const MAX_PAINTED:usize=2_000;
const MAX_EXTENT:f64=20_000.;
#[derive(Clone,Debug,PartialEq)]
pub(super) struct ViewState {pub zoom:f64,pub query:String,pub group:String,pub generation:u64}
impl Default for ViewState {fn default()->Self{Self{zoom:1.,query:String::new(),group:String::new(),generation:0}}}
impl ViewState {
    fn set_zoom(&mut self,value:f64){let value=if value.is_finite(){value.clamp(0.4,1.5)}else{1.};if self.zoom!=value{self.zoom=value;self.generation+=1;}}
    fn set_query(&mut self,value:&str){if self.query!=value{self.query=value.into();self.generation+=1;}}
    fn set_group(&mut self,value:&str){if self.group!=value{self.group=value.into();self.generation+=1;}}
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub(super) struct Rect {pub x:f64,pub y:f64,pub width:f64,pub height:f64}
fn coordinate(value:&Value)->f64{value.as_f64().filter(|n|n.is_finite()).unwrap_or(0.)}
fn card_rect(card:&Value)->Rect{Rect{x:coordinate(&card["x"]),y:coordinate(&card["y"]),width:CARD_WIDTH,height:CARD_HEIGHT}}
fn intersects(a:Rect,b:Rect)->bool{a.x<b.x+b.width&&a.x+a.width>b.x&&a.y<b.y+b.height&&a.y+a.height>b.y}
fn to_world(x:f64,y:f64,zoom:f64)->Option<(f64,f64)>{(x.is_finite()&&y.is_finite()&&zoom.is_finite()&&(0.4..=1.5).contains(&zoom)).then_some((x/zoom,y/zoom))}
fn bounds(cards:&[Value])->(f64,f64){cards.iter().take(MAX_CARDS).fold((2800_f64,1800_f64),|(w,h),card|{let r=card_rect(card);(w.max(r.x+r.width+60.).min(MAX_EXTENT),h.max(r.y+r.height+60.).min(MAX_EXTENT))})}
fn group_names(board:&Value)->HashMap<String,String>{board["groups"].as_array().into_iter().flatten().take(MAX_CARDS).filter_map(|group|Some((group["id"].as_str()?.into(),group["name"].as_str().unwrap_or("Group").into()))).collect()}
fn card_metadata(card:&Value,groups:&HashMap<String,String>)->(String,String){
    let kind=card["kind"].as_str().unwrap_or("Card");
    let group=card["groupId"].as_str().map(|id|groups.get(id).map(String::as_str).unwrap_or("Missing group"));
    let caption=group.map_or_else(||kind.to_owned(),|name|format!("{kind} · {name}"));
    let labels=card["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" · ");(caption,labels)
}
fn matches(card:&Value,view:&ViewState,groups:&HashMap<String,String>)->bool{
    if !view.group.is_empty()&&card["groupId"].as_str()!=Some(&view.group){return false;}
    if view.query.is_empty(){return true;}
    let (caption,labels)=card_metadata(card,groups);
    format!("{} {} {caption} {labels}",card["title"].as_str().unwrap_or(""),card["body"].as_str().unwrap_or("")).to_lowercase().contains(&view.query.to_lowercase())
}
fn group_rectangles(cards:&[&Value],groups:&HashMap<String,String>)->Vec<(String,Rect)>{
    let mut rectangles:BTreeMap<String,Rect>=BTreeMap::new();
    for card in cards{let Some(id)=card["groupId"].as_str().filter(|id|groups.contains_key(*id))else{continue;};let r=card_rect(card);
        rectangles.entry(id.into()).and_modify(|old|{let right=(old.x+old.width).max(r.x+r.width);let bottom=(old.y+old.height).max(r.y+r.height);old.x=old.x.min(r.x);old.y=old.y.min(r.y);old.width=right-old.x;old.height=bottom-old.y;}).or_insert(r);
    }
    rectangles.into_iter().map(|(id,r)|(groups[&id].clone(),Rect{x:r.x-8.,y:r.y-24.,width:r.width+16.,height:r.height+32.})).collect()
}
#[derive(Clone,Debug)]
struct Drag {board:String,card:String,offset:(f64,f64),zoom:f64,view_generation:u64,data_epoch:u64}
impl Drag {
    fn position(&self,x:f64,y:f64,extent:(f64,f64))->Option<(f64,f64)>{let(x,y)=to_world(x,y,self.zoom)?;Some(((x-self.offset.0).clamp(0.,(extent.0-CARD_WIDTH).max(0.)),(y-self.offset.1).clamp(0.,(extent.1-CARD_HEIGHT).max(0.))))}
    fn valid(&self,view:&ViewState,epoch:u64,board:&str)->bool{self.view_generation==view.generation&&self.data_epoch==epoch&&self.board==board}
}
fn draw_text(area:&gtk::DrawingArea,context:&cairo::Context,value:&str,x:f64,y:f64,lines:i32){
    use glib::translate::ToGlibPtr;
    let excerpt=value.chars().take(2_000).collect::<String>();let layout=area.create_pango_layout(Some(&excerpt));layout.set_width(200*gtk::pango::SCALE);layout.set_wrap(gtk::pango::WrapMode::WordChar);layout.set_height(-lines);layout.set_ellipsize(gtk::pango::EllipsizeMode::End);context.move_to(x,y);
    unsafe{super::pango_cairo_show_layout(context.to_raw_none(),layout.to_glib_none().0);}
}
fn pan(adjustment:&gtk::Adjustment,value:f64){adjustment.set_value(value.clamp(adjustment.lower(),(adjustment.upper()-adjustment.page_size()).max(adjustment.lower())));}


fn board_mut<'a>(data:&'a mut Value,id:&str)->Result<&'a mut Value,String>{
    let boards=data["boards"].as_array_mut().ok_or("Invalid board collection.")?;
    if boards.iter().filter(|b|b["id"].as_str()==Some(id)).count()!=1{return Err("The selected board is missing or ambiguous. Refresh boards.".into());}
    Ok(boards.iter_mut().find(|b|b["id"].as_str()==Some(id)).unwrap())
}
fn edit_card(data:&mut Value,board:&str,expected:&Value,group:Option<&str>,labels:Vec<String>)->Result<bool,String>{
    let board=board_mut(data,board)?;
    if let Some(id)=group{if expected["groupId"].as_str()!=Some(id)&&board["groups"].as_array().into_iter().flatten().filter(|g|g["id"].as_str()==Some(id)).count()!=1{return Err("The selected group is missing or ambiguous. Refresh choices.".into());}}
    let id=expected["id"].as_str().ok_or("Choose an available card.")?;
    let cards=board["cards"].as_array_mut().ok_or("Invalid card collection.")?;
    if cards.iter().filter(|c|c["id"].as_str()==Some(id)).count()!=1{return Err("The selected card is missing or ambiguous. Refresh choices.".into());}
    let card=cards.iter_mut().find(|c|c["id"].as_str()==Some(id)).unwrap();
    if card!=expected{return Err("The selected card changed. Refresh choices before editing its labels or group.".into());}
    let mut next=card.clone();next["groupId"]=group.map_or(Value::Null,|id|json!(id));next["labels"]=json!(labels);let changed=next!=*card;*card=next;Ok(changed)
}
fn change_group(data:&mut Value,board:&str,expected:&Value,id:&str,name:Option<&str>)->Result<bool,String>{
    if id.is_empty()||id.len()>128{return Err("Invalid group identity.".into());}
    if name.is_some_and(|name|name.trim().is_empty()||name.chars().count()>2_000){return Err("Enter a group name of 1–2,000 characters.".into());}
    let board=board_mut(data,board)?;
    if board!=expected{return Err("The board changed. Refresh group choices before changing groups.".into());}
    let mut next=board.clone();let groups=next["groups"].as_array_mut().ok_or("Invalid group collection.")?;
    let count=groups.iter().filter(|g|g["id"].as_str()==Some(id)).count();
    if count>1{return Err("This group identity is ambiguous.".into());}
    if let Some(name)=name{
        if count==0{if groups.len()>=MAX_CARDS{return Err("This board reached its group limit.".into());}groups.push(json!({"id":id,"name":name}));}
        else{groups.iter_mut().find(|g|g["id"].as_str()==Some(id)).unwrap()["name"]=json!(name);}
    }else{
        if count!=1{return Err("Choose an available group to ungroup.".into());}groups.retain(|g|g["id"].as_str()!=Some(id));
        for card in next["cards"].as_array_mut().ok_or("Invalid card collection.")?{if card["groupId"].as_str()==Some(id){card["groupId"]=Value::Null;}}
    }
    let changed=next!=*board;*board=next;Ok(changed)
}
fn labels_from_text(text:&str)->Vec<String>{if text.is_empty(){Vec::new()}else{text.split('\n').map(str::to_owned).collect()}}
fn author_controls(page:Rc<DataPage>,board:gtk::ComboBoxText,area:gtk::DrawingArea,controls:&gtk::Box,refresh_groups:Rc<dyn Fn()>)->Rc<dyn Fn()>{
    let expander=gtk::Expander::new(Some("Labels and groups"));let box_=gtk::Box::new(gtk::Orientation::Vertical,6);expander.add(&box_);controls.pack_start(&expander,false,false,0);
    let cards=gtk::ComboBoxText::new();cards.set_tooltip_text(Some("Card labels and group"));let member=gtk::ComboBoxText::new();member.set_tooltip_text(Some("Selected card group"));
    let labels=gtk::TextView::new();labels.set_wrap_mode(gtk::WrapMode::WordChar);labels.set_tooltip_text(Some("Selected card labels, one per line"));labels.set_size_request(-1,72);let buffer=labels.buffer().unwrap();
    let groups=gtk::ComboBoxText::new();groups.set_tooltip_text(Some("Group to rename or ungroup"));let name=gtk::Entry::new();name.set_placeholder_text(Some("Group name"));name.set_max_length(2000);
    let create=gtk::Button::with_label("Add group to draft");let rename=gtk::Button::with_label("Rename group in draft");let remove=gtk::Button::with_label("Ungroup cards and remove group");let feedback=gtk::Label::new(Some("Labels and groups stay in the planning draft until Save."));feedback.set_line_wrap(true);feedback.set_xalign(0.);
    for widget in [cards.clone().upcast::<gtk::Widget>(),member.clone().upcast(),gtk::Label::new(Some("Labels — one per line")).upcast(),labels.upcast(),groups.clone().upcast(),name.clone().upcast(),create.clone().upcast(),rename.clone().upcast(),remove.clone().upcast(),feedback.clone().upcast()]{box_.pack_start(&widget,false,false,0);}
    let updating=Rc::new(Cell::new(false));let captured=Rc::new(RefCell::new(None::<(String,Value)>));let group_capture=Rc::new(RefCell::new(None::<(String,Value)>));
    let p=page.clone();let b=board.clone();let card_select=cards.clone();let m=member.clone();let text=buffer.clone();let guard=updating.clone();let capture=captured.clone();
    let load_card:Rc<dyn Fn()>=Rc::new(move||{guard.set(true);let data=p.value.borrow();let current=selected_board(&data,&b).and_then(|i|{let board=&data["boards"][i];let id=card_select.active_id()?;let card=board["cards"].as_array()?.iter().find(|c|c["id"].as_str()==Some(id.as_str()))?;Some((board["id"].as_str()?.to_owned(),card.clone()))});
        if let Some((_,card))=&current{m.set_active_id(Some(card["groupId"].as_str().unwrap_or("")));text.set_text(&card["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"));}else{m.set_active(Some(0));text.set_text("");}*capture.borrow_mut()=current;guard.set(false);});
    let load=load_card.clone();let guard=updating.clone();cards.connect_changed(move|_|{if !guard.get(){load();}});
    let p=page.clone();let b=board.clone();let cs=cards.clone();let m=member.clone();let gs=groups.clone();let guard=updating.clone();let gc=group_capture.clone();let load=load_card.clone();
    let refresh:Rc<dyn Fn()>=Rc::new(move||{guard.set(true);let old=cs.active_id();let old_group=gs.active_id();cs.remove_all();m.remove_all();gs.remove_all();cs.append(Some(""),"Choose card");m.append(Some(""),"Ungrouped");gs.append(Some(""),"Choose group");let data=p.value.borrow();*gc.borrow_mut()=None;if let Some(i)=selected_board(&data,&b){let value=&data["boards"][i];*gc.borrow_mut()=Some((value["id"].as_str().unwrap_or("").into(),value.clone()));for card in value["cards"].as_array().into_iter().flatten().take(MAX_CARDS){if let Some(id)=card["id"].as_str(){cs.append(Some(id),card["title"].as_str().unwrap_or("Card"));}}
        for g in value["groups"].as_array().into_iter().flatten().take(MAX_CARDS){if let Some(id)=g["id"].as_str(){let name=g["name"].as_str().unwrap_or("Group");m.append(Some(id),name);gs.append(Some(id),name);}}
        let known=group_names(value);for card in value["cards"].as_array().into_iter().flatten().take(MAX_CARDS){if let Some(id)=card["groupId"].as_str().filter(|id|!known.contains_key(*id)){m.append(Some(id),"Missing group — retained");}}
        }if !old.as_deref().is_some_and(|id|cs.set_active_id(Some(id))){cs.set_active(Some(0));}if !old_group.as_deref().is_some_and(|id|gs.set_active_id(Some(id))){gs.set_active(Some(0));}drop(data);guard.set(false);load();});
    let gs=groups.clone();let gc=group_capture.clone();let entry=name.clone();gs.connect_changed(move|combo|{if let Some((_,board))=gc.borrow().as_ref(){let id=combo.active_id().unwrap_or_default();if let Some(group)=board["groups"].as_array().into_iter().flatten().find(|g|g["id"].as_str()==Some(id.as_str())){entry.set_text(group["name"].as_str().unwrap_or(""));}}});
    let p=page.clone();let cap=captured.clone();let m=member.clone();let text=buffer.clone();let guard=updating.clone();let a=area.clone();let notice=feedback.clone();let gc=group_capture.clone();let reload=load_card.clone();
    let apply:Rc<dyn Fn()>=Rc::new(move||{if guard.get()||!p.loaded.get(){return;}let Some((board,card))=cap.borrow().clone()else{notice.set_text("Choose a card first.");return;};let text=text.text(&text.start_iter(),&text.end_iter(),true).unwrap_or_default();let group=m.active_id().unwrap_or_default();let original=card["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>();let labels=if original.join("\n")==text.as_str(){original}else{labels_from_text(text.as_str())};let next_labels=labels.clone();let result=edit_card(&mut p.value.borrow_mut(),&board,&card,(!group.is_empty()).then_some(group.as_str()),labels);match result{Ok(changed)=>{if changed{p.dirty.set(true);p.epoch.set(p.epoch.get()+1);let mut next=card;next["groupId"]=if group.is_empty(){Value::Null}else{json!(group.as_str())};next["labels"]=json!(next_labels);*cap.borrow_mut()=Some((board.clone(),next));if let Some(i)=board_index(&p.value.borrow(),&board){*gc.borrow_mut()=Some((board,p.value.borrow()["boards"][i].clone()));}a.queue_draw();}notice.set_text("Labels and groups are in the draft. Save keeps them; Discard restores saved data.");},Err(error)=>{reload();notice.set_text(&format!("Change was refused: {error}"));}}});
    let change=apply.clone();member.connect_changed(move|_|change());let change=apply;buffer.connect_changed(move|_|change());
    for (button,action) in [(create,0),(rename,1),(remove,2)]{let p=page.clone();let cap=group_capture.clone();let gs=groups.clone();let name=name.clone();let refresh=refresh.clone();let filter_groups=refresh_groups.clone();let a=area.clone();let notice=feedback.clone();button.connect_clicked(move|_|{if !p.loaded.get(){return;}let Some((id,expected))=cap.borrow().clone()else{notice.set_text("Refresh board choices first.");return;};let identity=if action==0{uuid::Uuid::new_v4().to_string()}else{gs.active_id().unwrap_or_default().to_string()};let name=name.text();if action!=0&&identity.is_empty(){notice.set_text("Choose a group first.");return;}let result=change_group(&mut p.value.borrow_mut(),&id,&expected,&identity,(action!=2).then_some(name.as_str()));match result{Ok(changed)=>{if changed{p.dirty.set(true);p.epoch.set(p.epoch.get()+1);}refresh();filter_groups();a.queue_draw();notice.set_text("Group change is in the draft. Story objects and card positions are retained.");},Err(error)=>notice.set_text(&error)}});}
    refresh
}
pub(super) fn build(page:Rc<DataPage>,parent:gtk::Window)->gtk::Box{
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
    let view=Rc::new(RefCell::new(ViewState::default()));
    let filter=gtk::Entry::new();filter.set_placeholder_text(Some("Filter cards or labels…"));filter.set_max_length(1000);filter.set_tooltip_text(Some("Filter board cards"));
    let group=gtk::ComboBoxText::new();group.set_tooltip_text(Some("Filter board group"));group.append(Some(""),"All groups");group.set_active(Some(0));
    let zoom=gtk::SpinButton::with_range(40.,150.,10.);zoom.set_numeric(true);zoom.set_value(100.);zoom.set_tooltip_text(Some("Board zoom percent"));
    let zoom_row=gtk::Box::new(gtk::Orientation::Horizontal,6);zoom_row.pack_start(&gtk::Label::new(Some("Zoom %")),false,false,0);zoom_row.pack_start(&zoom,true,true,0);let reset=gtk::Button::with_label("Reset view");zoom_row.pack_start(&reset,false,false,0);
    let pans=gtk::Box::new(gtk::Orientation::Horizontal,4);let pan_left=gtk::Button::with_label("←");pan_left.set_tooltip_text(Some("Pan left"));let pan_right=gtk::Button::with_label("→");pan_right.set_tooltip_text(Some("Pan right"));let pan_up=gtk::Button::with_label("↑");pan_up.set_tooltip_text(Some("Pan up"));let pan_down=gtk::Button::with_label("↓");pan_down.set_tooltip_text(Some("Pan down"));for button in [&pan_left,&pan_right,&pan_up,&pan_down]{pans.pack_start(button,true,true,0);}
    let count=gtk::Label::new(None);count.set_xalign(0.);count.set_line_wrap(true);count.set_tooltip_text(Some("Scroll or middle-drag to pan. Zoom and filters stay in this window."));
    controls.pack_start(&filter,false,false,0);controls.pack_start(&group,false,false,0);controls.pack_start(&zoom_row,false,false,0);controls.pack_start(&pans,false,false,0);controls.pack_start(&count,false,false,0);
    root.pack_start(&controls,false,false,0);
    let area=gtk::DrawingArea::new();area.set_size_request(2800,1800);area.add_events(gdk::EventMask::BUTTON_PRESS_MASK|gdk::EventMask::BUTTON_RELEASE_MASK|gdk::EventMask::POINTER_MOTION_MASK);
    let scroll=gtk::ScrolledWindow::new(None::<&gtk::Adjustment>,None::<&gtk::Adjustment>);scroll.add(&area);root.pack_start(&scroll,true,true,0);
    let h=scroll.hadjustment();let v=scroll.vadjustment();
    for (button,axis,delta) in [(pan_left,h.clone(),-120.),(pan_right,h.clone(),120.),(pan_up,v.clone(),-120.),(pan_down,v.clone(),120.)]{button.connect_clicked(move|_|pan(&axis,axis.value()+delta));}
    for axis in [&h,&v]{let a=area.clone();axis.connect_value_changed(move|_|a.queue_draw());}
    let a=area.clone();let p=page.clone();let c=combo.clone();let state=view.clone();
    let resize:Rc<dyn Fn()>=Rc::new(move||{let data=p.value.borrow();let extent=selected_board(&data,&c).and_then(|i|data["boards"][i]["cards"].as_array()).map(|cards|bounds(cards)).unwrap_or((2800.,1800.));let z=state.borrow().zoom;a.set_size_request((extent.0*z).ceil() as i32,(extent.1*z).ceil() as i32);});
    let state=view.clone();let a=area.clone();filter.connect_changed(move|entry|{state.borrow_mut().set_query(entry.text().as_str());a.queue_draw();});
    let state=view.clone();let a=area.clone();group.connect_changed(move|combo|{state.borrow_mut().set_group(combo.active_id().as_deref().unwrap_or(""));a.queue_draw();});
    let state=view.clone();let a=area.clone();let scale=resize.clone();let hx=h.clone();let vy=v.clone();zoom.connect_value_changed(move|spin|{let old=state.borrow().zoom;let center=((hx.value()+hx.page_size()/2.)/old,(vy.value()+vy.page_size()/2.)/old);state.borrow_mut().set_zoom(spin.value()/100.);scale();a.queue_draw();let state=state.clone();let generation=state.borrow().generation;let hx=hx.clone();let vy=vy.clone();glib::idle_add_local_once(move||{let state=state.borrow();if state.generation==generation{pan(&hx,center.0*state.zoom-hx.page_size()/2.);pan(&vy,center.1*state.zoom-vy.page_size()/2.);}});});
    let search=filter.clone();let groups=group.clone();let z=zoom.clone();let state=view.clone();let hx=h.clone();let vy=v.clone();reset.connect_clicked(move|_|{search.set_text("");groups.set_active_id(Some(""));z.set_value(100.);state.borrow_mut().generation+=1;pan(&hx,0.);pan(&vy,0.);});
    let p=page.clone();let c=combo.clone();let g=group.clone();let scale=resize.clone();
    let refresh_groups:Rc<dyn Fn()>=Rc::new(move||{let previous=g.active_id();g.remove_all();g.append(Some(""),"All groups");let data=p.value.borrow();if let Some(i)=selected_board(&data,&c){for group in data["boards"][i]["groups"].as_array().into_iter().flatten().take(MAX_CARDS){if let Some(id)=group["id"].as_str(){g.append(Some(id),group["name"].as_str().unwrap_or("Group"));}}}if !previous.as_deref().is_some_and(|id|g.set_active_id(Some(id))){g.set_active(Some(0));}scale();});
    let refresh_authors=author_controls(page.clone(),combo.clone(),area.clone(),&controls,refresh_groups.clone());
    let capture=Rc::new(RefCell::new(None::<(String,Value)>));
    let p=page.clone();let c=combo.clone();let s=source.clone();let t=target.clone();let captured=capture.clone();let feedback=status.clone();
    let scale=resize.clone();let refresh_view_groups=refresh_groups.clone();
    let choices:Rc<dyn Fn()>=Rc::new(move||{
        scale();refresh_view_groups();refresh_authors();
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
    let choose=choices.clone();let a=area.clone();let search=filter.clone();let g=group.clone();let hx=h.clone();let vy=v.clone();combo.connect_changed(move|_|{search.set_text("");g.set_active_id(Some(""));pan(&hx,0.);pan(&vy,0.);choose();a.queue_draw();});
    let p=page.clone();let s=source;let t=target;let k=kind;let l=label;let captured=capture;let feedback=status;let a=area.clone();
    connect.connect_clicked(move|_|{
        if !p.loaded.get()||p.saving.get(){feedback.set_text("Wait for the planning save to finish before connecting cards.");return;}
        let Some((board_id,cards))=captured.borrow().clone()else{feedback.set_text("Refresh board and card choices first.");return;};
        let from=s.active_id().unwrap_or_default();let to=t.active_id().unwrap_or_default();let kind=k.active_id().unwrap_or_default();let label=l.text();
        let result=add_connection(&mut p.value.borrow_mut(),&board_id,&cards,&from,&to,&kind,&label,&uuid::Uuid::new_v4().to_string());
        match result{Ok(())=>{p.dirty.set(true);p.epoch.set(p.epoch.get()+1);feedback.set_text("Connection added to draft. Ctrl+S saves; leaving offers Save / Discard / Cancel.");a.queue_draw();},Err(error)=>feedback.set_text(&error)}
    });
    let p=page.clone();let c=combo.clone();let state=view.clone();let hx=h.clone();let vy=v.clone();let notice=count.clone();
    area.connect_draw(move|area,context|{
        let data=p.value.borrow();context.set_source_rgb(1.,1.,1.);let _=context.paint();let Some(index)=selected_board(&data,&c)else{return glib::Propagation::Proceed;};let board=&data["boards"][index];
        let cards=board["cards"].as_array().map(Vec::as_slice).unwrap_or(&[]);let state=state.borrow();let groups=group_names(board);let filtered=cards.iter().take(MAX_CARDS).filter(|card|matches(card,&state,&groups)).collect::<Vec<_>>();
        let viewport=Rect{x:hx.value()/state.zoom,y:vy.value()/state.zoom,width:hx.page_size().max(1.)/state.zoom,height:vy.page_size().max(1.)/state.zoom};
        let painted=filtered.iter().copied().filter(|card|intersects(card_rect(card),viewport)).take(MAX_PAINTED).collect::<Vec<_>>();
        let outside=filtered.iter().filter(|card|{let r=card_rect(card);r.x>=MAX_EXTENT||r.y>=MAX_EXTENT||r.x+r.width<=0.||r.y+r.height<=0.}).count();
        let message=format!("{} visible · {} matching cards{}{}",painted.len(),filtered.len(),if painted.len()==MAX_PAINTED||cards.len()>MAX_CARDS{" · display limit reached; stored cards retained"}else{""},if outside>0{" · some coordinates are outside the bounded view; stored cards retained"}else{""});if notice.text().as_str()!=message{notice.set_text(&message);}
        let _=context.save();context.scale(state.zoom,state.zoom);
        for (name,r) in group_rectangles(&filtered,&groups){if !intersects(r,viewport){continue;}context.set_source_rgb(0.94,0.96,0.98);context.rectangle(r.x,r.y,r.width,r.height);let _=context.fill();context.set_source_rgb(0.45,0.5,0.55);context.rectangle(r.x,r.y,r.width,r.height);let _=context.stroke();draw_text(area,context,&name,r.x+4.,r.y+2.,1);}
        let allowed=filtered.iter().filter_map(|card|card["id"].as_str()).collect::<HashSet<_>>();let visible=painted.iter().filter_map(|card|card["id"].as_str()).collect::<HashSet<_>>();let by_id=cards.iter().take(MAX_CARDS).filter_map(|card|Some((card["id"].as_str()?,card))).collect::<HashMap<_,_>>();let mut captions=Vec::new();
        for edge in board["edges"].as_array().into_iter().flatten().take(MAX_CARDS){let (Some(from_id),Some(to_id))=(edge["fromId"].as_str(),edge["toId"].as_str())else{continue;};if !allowed.contains(from_id)||!allowed.contains(to_id)||!visible.contains(from_id)&&!visible.contains(to_id){continue;}
            if let(Some(from),Some(to))=(by_id.get(from_id),by_id.get(to_id)){let a=card_rect(from);let b=card_rect(to);let(ax,ay,bx,by)=(a.x+110.,a.y+55.,b.x+110.,b.y+55.);context.set_source_rgb(0.6,0.65,0.7);context.move_to(ax,ay);context.line_to(bx,by);let _=context.stroke();captions.push((super::edge_caption(edge),(ax+bx)/2.,(ay+by)/2.));}
        }
        for card in painted{let r=card_rect(card);context.set_source_rgb(0.98,0.98,0.98);context.rectangle(r.x,r.y,r.width,r.height);let _=context.fill();context.set_source_rgb(0.75,0.75,0.75);context.rectangle(r.x,r.y,r.width,r.height);let _=context.stroke();context.set_source_rgb(0.12,0.15,0.13);
            draw_text(area,context,card["title"].as_str().unwrap_or("Card"),r.x+10.,r.y+6.,1);draw_text(area,context,&card["body"].as_str().unwrap_or("").chars().take(110).collect::<String>(),r.x+10.,r.y+28.,2);let(caption,labels)=card_metadata(card,&groups);draw_text(area,context,&caption,r.x+10.,r.y+72.,1);draw_text(area,context,&labels,r.x+10.,r.y+90.,1);
        }
        for(caption,x,y)in captions{draw_edge_caption(area,context,&caption,x,y);}let _=context.restore();glib::Propagation::Proceed
    });
    let dragging=Rc::new(RefCell::new(None::<Drag>));let panning=Rc::new(RefCell::new(None::<(f64,f64,f64,f64)>));let p=page.clone();let c=combo.clone();let d=dragging.clone();let moving=panning.clone();let state=view.clone();let hx=h.clone();let vy=v.clone();
    area.connect_button_press_event(move|area,event|{
        if event.button()==2{let(x,y)=event.root();*moving.borrow_mut()=Some((x,y,hx.value(),vy.value()));*d.borrow_mut()=None;return glib::Propagation::Stop;}
        *moving.borrow_mut()=None;if event.button()!=1||!p.loaded.get()||p.saving.get(){*d.borrow_mut()=None;return glib::Propagation::Stop;}
        let(x,y)=event.position();let state=state.borrow();let Some((x,y))=to_world(x,y,state.zoom)else{return glib::Propagation::Stop;};let data=p.value.borrow();let Some(index)=selected_board(&data,&c)else{return glib::Propagation::Stop;};let board=&data["boards"][index];let groups=group_names(board);
        let viewport=Rect{x:hx.value()/state.zoom,y:vy.value()/state.zoom,width:hx.page_size().max(1.)/state.zoom,height:vy.page_size().max(1.)/state.zoom};
        let painted=board["cards"].as_array().into_iter().flatten().take(MAX_CARDS).filter(|card|matches(card,&state,&groups)&&intersects(card_rect(card),viewport)).take(MAX_PAINTED).collect::<Vec<_>>();
        *d.borrow_mut()=None;if let Some(card)=painted.into_iter().rev().find(|card|{let r=card_rect(card);x>=r.x&&x<=r.x+r.width&&y>=r.y&&y<=r.y+r.height}){if let(Some(board),Some(id))=(board["id"].as_str(),card["id"].as_str()){let r=card_rect(card);*d.borrow_mut()=Some(Drag{board:board.into(),card:id.into(),offset:(x-r.x,y-r.y),zoom:state.zoom,view_generation:state.generation,data_epoch:p.epoch.get()});let(caption,labels)=card_metadata(card,&groups);area.set_tooltip_text(Some(&format!("{} · {caption} · {labels}",card["title"].as_str().unwrap_or("Card"))));}}glib::Propagation::Stop
    });
    let p=page.clone();let c=combo.clone();let d=dragging.clone();let moving=panning.clone();let state=view.clone();let hx=h.clone();let vy=v.clone();
    area.connect_motion_notify_event(move|area,event|{
        if let Some((x,y,left,top))=*moving.borrow(){if event.state().contains(gdk::ModifierType::BUTTON2_MASK){let(rx,ry)=event.root();pan(&hx,left+x-rx);pan(&vy,top+y-ry);}return glib::Propagation::Stop;}
        if !p.loaded.get()||p.saving.get(){*d.borrow_mut()=None;return glib::Propagation::Stop;}
        let drag=d.borrow().clone();if let Some(mut drag)=drag{if event.state().contains(gdk::ModifierType::BUTTON1_MASK){if !drag.valid(&state.borrow(),p.epoch.get(),c.active_id().as_deref().unwrap_or("")){*d.borrow_mut()=None;return glib::Propagation::Stop;}let(x,y)=event.position();let mut data=p.value.borrow_mut();if let Some(board)=board_index(&data,&drag.board){let extent=data["boards"][board]["cards"].as_array().map(|cards|bounds(cards)).unwrap_or((2800.,1800.));let Some((x,y))=drag.position(x,y,extent)else{return glib::Propagation::Stop;};if let Some(card)=data["boards"][board]["cards"].as_array_mut().and_then(|cards|cards.iter_mut().find(|card|card["id"]==drag.card)){if coordinate(&card["x"])!=x||coordinate(&card["y"])!=y{card["x"]=json!(x);card["y"]=json!(y);p.dirty.set(true);p.epoch.set(p.epoch.get()+1);drag.data_epoch=p.epoch.get();*d.borrow_mut()=Some(drag);area.queue_draw();}}}}}glib::Propagation::Stop
    });
    area.connect_button_release_event(move|_,_|{*dragging.borrow_mut()=None;*panning.borrow_mut()=None;glib::Propagation::Stop});
    let p=page;let a=area;let choose=choices;
    add.connect_clicked(move|button|{
        if !p.loaded.get()||p.saving.get(){return;}let parent=button.toplevel().and_then(|w|w.downcast::<gtk::Window>().ok()).unwrap_or_else(||parent.clone());let Some(title)=prompt(&parent,"New plot card","New note")else{return;};
        let mut data=p.value.borrow_mut();let Some(index)=selected_board(&data,&combo)else{return;};if let Some(cards)=data["boards"][index]["cards"].as_array_mut(){if cards.len()>=MAX_CARDS{return;}let i=cards.len();cards.push(json!({"id":uuid::Uuid::new_v4().to_string(),"title":title,"body":"","kind":"note","reference":null,"x":20+(i%8)*250,"y":20+(i/8)*135,"groupId":null,"labels":[]}));p.dirty.set(true);p.epoch.set(p.epoch.get()+1);a.queue_draw();}drop(data);choose();
    });
    refresh.clicked();root
}

#[cfg(test)]
#[path="board_view_tests.rs"]
pub(crate) mod tests;
