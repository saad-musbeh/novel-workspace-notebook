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
use std::{cell::Cell,collections::{BTreeMap,HashMap,HashSet,VecDeque}};
const CHRONOLOGY_WORK:usize=20_000;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub(crate) enum EventOrder{Before,After,Same,Unknown,Unordered,Conflict}
struct TemporalEdge{from:usize,to:usize,days:f64,strict:bool,label:String}
struct Lower{day:f64,error:f64,strict:bool,trace:Option<usize>}
struct Trace{edge:usize,parent:Option<usize>}
/// Read-only feasibility of the existing authored range/order model. Bounds are
/// never tightened in the displayed data or written back as inferred dates.
pub(crate) struct ChronologyAnalysis<'a>{
    ids:Vec<&'a str>,index:HashMap<&'a str,usize>,ranges:Vec<Option<(f64,f64)>>,errors:Vec<f64>,
    edges:Vec<TemporalEdge>,outgoing:Vec<Vec<usize>>,component:Vec<usize>,
    conflicts:BTreeMap<usize,Vec<String>>,unsupported:HashSet<usize>,work:Cell<usize>,limited:Cell<bool>,
}
// Track accumulated numerical uncertainty. A nonzero authored offset remains
// provably positive on its own; cancellation of large/decimal paths may not.
fn temporal_add(day:f64,error:f64,offset:f64)->Option<(f64,f64)>{
    let next=day+offset;if !next.is_finite(){return None;}
    let exact_integer=[day,offset,next].iter().all(|n|n.fract()==0.0&&n.abs()<=9_007_199_254_740_992.0);
    let extra=if offset==0.0||exact_integer{0.0}else{day.abs().max(offset.abs()).max(next.abs())*(4.0*f64::EPSILON)};
    let error=error+extra;(error.is_finite()).then_some((next,error))
}
fn temporal_improves(day:f64,error:f64,old:f64,old_error:f64)->bool{day-old>error+old_error}
fn temporal_rows<'a>(doc:&'a Value,key:&str)->&'a [Value]{doc[key].as_array().map(Vec::as_slice).unwrap_or(&[])}
fn temporal_root(parents:&mut[usize],mut id:usize)->usize{while parents[id]!=id{parents[id]=parents[parents[id]];id=parents[id];}id}
fn temporal_join(parents:&mut[usize],a:usize,b:usize){let a=temporal_root(parents,a);let b=temporal_root(parents,b);if a!=b{parents[b]=a;}}
impl<'a> ChronologyAnalysis<'a>{
    fn visit(&self)->bool{let next=self.work.get().saturating_add(1);self.work.set(next);if next>CHRONOLOGY_WORK{self.limited.set(true);false}else{true}}
    pub(crate) fn limited(&self)->bool{self.limited.get()}
    pub(crate) fn new(doc:&'a Value)->Self{
        let input_limited=temporal_rows(doc,"events").len().saturating_add(temporal_rows(doc,"constraints").len())>CHRONOLOGY_WORK/4;
        let mut raw=HashMap::new();let mut duplicate=HashSet::new();
        for event in temporal_rows(doc,"events").iter().take(CHRONOLOGY_WORK/4){if let Some(id)=event["id"].as_str().filter(|id|!id.is_empty()&&id.len()<=200){if raw.insert(id,event).is_some(){duplicate.insert(id);}}}
        let mut ids=raw.keys().copied().collect::<Vec<_>>();ids.sort_unstable();let index=ids.iter().enumerate().map(|(i,id)|(*id,i)).collect::<HashMap<_,_>>();let count=ids.len();
        let mut analysis=Self{ids,index,ranges:vec![None;count],errors:vec![0.0;count],edges:Vec::new(),outgoing:vec![Vec::new();count],component:(0..count).collect(),conflicts:BTreeMap::new(),unsupported:HashSet::new(),work:Cell::new(0),limited:Cell::new(input_limited)};
        let ambiguous=duplicate.into_iter().filter_map(|id|analysis.index.get(id).copied()).collect::<HashSet<_>>();let mut invalid=ambiguous.clone();let mut known=vec![false;count];for i in &invalid{known[*i]=true;}let mut parents=(0..count).collect::<Vec<_>>();
        for i in 0..count {
            if !analysis.visit(){break;}
            let event=raw[analysis.ids[i]];
            if event["time"]["kind"]=="relative" {
                let anchor=event["time"]["eventId"].as_str().and_then(|id|analysis.index.get(id).copied());
                let offset=event["time"]["offsetDays"].as_f64().filter(|n|n.is_finite());
                if let Some(anchor)=anchor {temporal_join(&mut parents,i,anchor);}
                if let(Some(anchor),Some(offset))=(anchor.filter(|anchor|*anchor!=i&&!ambiguous.contains(anchor)&&!ambiguous.contains(&i)),offset) {
                    let label=format!("relative event {} ({offset} days from {})",analysis.ids[i],analysis.ids[anchor]);
                    analysis.push_edge(anchor,i,offset,false,label.clone());
                    analysis.push_edge(i,anchor,-offset,false,label);
                } else {invalid.insert(i);}
            }
            analysis.resolve(i,&raw,&mut known,&mut invalid);
        }
        let mut seen=HashSet::new();
        for (row,edge) in temporal_rows(doc,"constraints").iter().take(CHRONOLOGY_WORK/4).enumerate(){
            if !analysis.visit(){break;}if edge["confirmed"]!=true{continue;}
            let from=edge["beforeId"].as_str().and_then(|id|analysis.index.get(id).copied());let to=edge["afterId"].as_str().and_then(|id|analysis.index.get(id).copied());
            if let(Some(a),Some(b))=(from,to){temporal_join(&mut parents,a,b);}
            let days=edge["minimumDays"].as_f64().filter(|n|n.is_finite()&&*n>=0.);let strict=edge["strict"].as_bool();
            let label=edge["id"].as_str().filter(|id|!id.is_empty()&&id.len()<=200).map(str::to_owned).unwrap_or_else(||format!("constraint row {} (missing ID)",row+1));
            let unique=edge["id"].as_str().is_none_or(|id|seen.insert(id));
            match(from,to,days,strict){(Some(from),Some(to),Some(days),Some(strict))if unique&&!ambiguous.contains(&from)&&!ambiguous.contains(&to)=>{analysis.push_edge(from,to,days,strict,label);},_=>{invalid.extend(from);invalid.extend(to);}}
        }
        analysis.component=(0..count).map(|i|temporal_root(&mut parents,i)).collect();analysis.unsupported=invalid.into_iter().map(|i|analysis.component[i]).collect();
        analysis.check_cycles();
        analysis.propagate();analysis
    }
    fn push_edge(&mut self,from:usize,to:usize,days:f64,strict:bool,label:String){let i=self.edges.len();self.outgoing[from].push(i);self.edges.push(TemporalEdge{from,to,days,strict,label});}
    // Signed relative equalities require total cycle weight, not the presence
    // of one positive edge. Strict zero-total cycles are also impossible.
    fn check_cycles(&mut self){
        let n=self.ids.len();let mut lower=vec![(0.0f64,0usize);n];let mut previous:Vec<Option<usize>>=vec![None;n];let mut errors=vec![0.0;n];
        for pass in 0..n {
            let mut updated=Vec::new();
            for (i,e) in self.edges.iter().enumerate(){
                if !self.visit(){return;}let component=self.component[e.from];if self.conflicts.contains_key(&component){continue;}
                let Some((day,error))=temporal_add(lower[e.from].0,errors[e.from],e.days)else{self.unsupported.insert(component);continue;};
                let candidate=(day,lower[e.from].1.saturating_add(usize::from(e.strict)));
                if temporal_improves(day,error,lower[e.to].0,errors[e.to])||day==lower[e.to].0&&candidate.1>lower[e.to].1{lower[e.to]=candidate;errors[e.to]=error;previous[e.to]=Some(i);updated.push(e.to);}
                else if (day>lower[e.to].0||day==lower[e.to].0&&error>errors[e.to]&&lower[e.from].0!=0.0)&&!previous[e.from].is_some_and(|j|self.edges[j].label==e.label&&self.edges[j].from==e.to){self.unsupported.insert(component);}
            }
            if updated.is_empty(){break;}
            if pass+1==n {
                for mut vertex in updated {
                    for _ in 0..n{if !self.visit(){return;}let Some(edge)=previous[vertex]else{break;};vertex=self.edges[edge].from;}
                    let start=vertex;let mut cycle=Vec::new();let mut seen=HashSet::new();
                    while seen.insert(vertex){if !self.visit(){return;}let Some(edge)=previous[vertex]else{break;};cycle.push(edge);vertex=self.edges[edge].from;}
                    if vertex!=start{continue;}
                    let mut total=0.0;let mut error=0.0;let mut balanced=BTreeMap::<u64,i64>::new();for i in &cycle{let days=self.edges[*i].days;if days!=0.0{*balanced.entry(days.abs().to_bits()).or_default()+=if days>0.0{1}else{-1};}if let Some((next,uncertainty))=temporal_add(total,error,days){total=next;error=uncertainty;}else{error=f64::INFINITY;}}
                    let exact_zero=balanced.values().all(|count|*count==0);let strict=cycle.iter().any(|i|self.edges[*i].strict);
                    if total>error||exact_zero&&strict{let mut labels=cycle.iter().map(|i|self.edges[*i].label.clone()).collect::<Vec<_>>();labels.sort();labels.dedup();self.conflicts.insert(self.component[start],labels);}else{self.unsupported.insert(self.component[start]);}
                }
            }
        }
    }
    fn resolve(&mut self,start:usize,raw:&HashMap<&str,&Value>,known:&mut[bool],invalid:&mut HashSet<usize>){
        if known[start]{return;}let mut current=start;let mut path=Vec::new();let mut seen=HashSet::new();let mut result=None;let mut error=0.0;
        loop{
            if !self.visit(){break;}if known[current]{result=self.ranges[current];error=self.errors[current];break;}
            if !seen.insert(current){invalid.extend(path.iter().map(|(i,_)|*i));invalid.insert(current);break;}
            let time=&raw[self.ids[current]]["time"];
            match time["kind"].as_str(){
                Some("fixed")=>{result=time["day"].as_f64().filter(|n|n.is_finite()).map(|day|(day,day));if result.is_none(){invalid.insert(current);}known[current]=true;self.ranges[current]=result;break;},
                Some("interval"|"bounds")=>{result=time["start"].as_f64().zip(time["end"].as_f64()).filter(|(a,b)|a.is_finite()&&b.is_finite()&&a<=b);if result.is_none(){invalid.insert(current);}known[current]=true;self.ranges[current]=result;break;},
                Some("unknown")=>{known[current]=true;break;},
                Some("relative")=>{let offset=time["offsetDays"].as_f64().filter(|n|n.is_finite());let anchor=time["eventId"].as_str().and_then(|id|self.index.get(id).copied());if let(Some(offset),Some(anchor))=(offset,anchor){path.push((current,offset));current=anchor;}else{invalid.insert(current);known[current]=true;break;}},
                _=>{invalid.insert(current);known[current]=true;break;}
            }
        }
        for(i,offset)in path.into_iter().rev(){result=result.and_then(|(a,b)|{let(a,ea)=temporal_add(a,error,offset)?;let(b,eb)=temporal_add(b,error,offset)?;error=ea.max(eb);Some((a,b))});self.errors[i]=error;if result.is_none()&&known[current]&&self.ranges[current].is_some(){invalid.insert(i);}self.ranges[i]=result;known[i]=true;}
    }
    fn propagate(&mut self){
        let mut lower=self.ranges.iter().enumerate().map(|(i,range)|range.map(|(day,_)|Lower{day,error:self.errors[i],strict:false,trace:None})).collect::<Vec<_>>();let mut queue=(0..self.ids.len()).filter(|i|lower[*i].is_some()).collect::<VecDeque<_>>();let mut traces:Vec<Trace>=Vec::new();
        while let Some(from)=queue.pop_front(){if !self.visit(){return;}let component=self.component[from];if self.conflicts.contains_key(&component){continue;}let base=lower[from].as_ref().unwrap();let(day,error,strict,parent)=(base.day,base.error,base.strict,base.trace);
            for edge in &self.outgoing[from]{if !self.visit(){return;}let e=&self.edges[*edge];let Some((candidate,error))=temporal_add(day,error,e.days)else{self.unsupported.insert(component);continue;};let strict=strict||e.strict;
                if self.ranges[e.to].is_some_and(|(_,end)|candidate-end>error+self.errors[e.to]||candidate==end&&strict&&error+self.errors[e.to]==0.0){let mut path=vec![e.label.clone()];let mut trace=parent;while let Some(i)=trace{if !self.visit(){return;}path.push(self.edges[traces[i].edge].label.clone());trace=traces[i].parent;}path.reverse();self.conflicts.insert(component,path);break;}
                if lower[e.to].as_ref().is_none_or(|old|temporal_improves(candidate,error,old.day,old.error)||candidate==old.day&&error<=old.error&&strict&&!old.strict){let trace=traces.len();traces.push(Trace{edge:*edge,parent});lower[e.to]=Some(Lower{day:candidate,error,strict,trace:Some(trace)});queue.push_back(e.to);}
            }
        }
    }
    pub(crate) fn range(&self,id:&str)->Option<(f64,f64)>{self.index.get(id).and_then(|i|self.ranges[*i])}
    pub(crate) fn range_uncertainty(&self,id:&str)->Option<f64>{let i=*self.index.get(id)?;self.ranges[i].map(|_|self.errors[i])}
    fn strict_path(&self,start:usize,target:usize)->Option<bool>{
        let mut lower=vec![None::<(f64,f64,bool)>;self.ids.len()];lower[start]=Some((0.0,0.0,false));let mut queue=VecDeque::from([start]);
        while let Some(from)=queue.pop_front(){
            if !self.visit(){return None;}let(day,error,strict)=lower[from]?;
            for i in &self.outgoing[from]{if !self.visit(){return None;}let e=&self.edges[*i];let(next,error)=temporal_add(day,error,e.days)?;let candidate=(next,error,strict||e.strict);
                if e.to==target&&(candidate.0>candidate.1||candidate.0==0.0&&candidate.1==0.0&&candidate.2){return Some(true);}
                if lower[e.to].is_none_or(|old|temporal_improves(candidate.0,candidate.1,old.0,old.1)||candidate.0==old.0&&candidate.1<=old.1&&candidate.2&&!old.2){lower[e.to]=Some(candidate);queue.push_back(e.to);}
            }
        }Some(false)
    }
    pub(crate) fn order(&self,left:&str,right:&str)->EventOrder{
        let(Some(a),Some(b))=(self.index.get(left).copied(),self.index.get(right).copied())else{return EventOrder::Unknown;};
        if self.conflicts.contains_key(&self.component[a])||self.conflicts.contains_key(&self.component[b]){return EventOrder::Conflict;}
        if self.limited()||self.unsupported.contains(&self.component[a])||self.unsupported.contains(&self.component[b]){return EventOrder::Unknown;}
        let(Some(forward),Some(backward))=(self.strict_path(a,b),self.strict_path(b,a))else{return EventOrder::Unknown;};
        if forward&&backward{EventOrder::Conflict}else if left==right{EventOrder::Same}else if forward||self.ranges[a].zip(self.ranges[b]).is_some_and(|((_,end),(start,_))|start-end>self.errors[a]+self.errors[b]){EventOrder::Before}else if backward||self.ranges[a].zip(self.ranges[b]).is_some_and(|((start,_),(_,end))|start-end>self.errors[a]+self.errors[b]){EventOrder::After}else if self.ranges[a].is_some()&&self.ranges[b].is_some(){EventOrder::Unordered}else{EventOrder::Unknown}
    }
    pub(crate) fn contributors(&self,id:&str)->Option<&[String]>{let i=*self.index.get(id)?;self.conflicts.get(&self.component[i]).map(Vec::as_slice)}
}
fn constraint_comparison(analysis:&ChronologyAnalysis<'_>,row:&Value)->&'static str{
    let Some((aid,bid))=row["beforeId"].as_str().zip(row["afterId"].as_str())else{return "Unresolved";};
    let Some((a,b))=analysis.range(aid).zip(analysis.range(bid))else{return "Unresolved";};
    let Some(gap)=row["minimumDays"].as_f64().filter(|n|n.is_finite()&&*n>=0.)else{return "Unresolved";};let strict=row["strict"]==true;
    let Some((minimum,error_min))=temporal_add(a.0,analysis.errors[analysis.index[aid]],gap)else{return "Unresolved";};
    let Some((maximum,error_max))=temporal_add(a.1,analysis.errors[analysis.index[aid]],gap)else{return "Unresolved";};let error_b=analysis.errors[analysis.index[bid]];
    if minimum-b.1>error_min+error_b||minimum==b.1&&strict&&error_min+error_b==0.0{"Contradiction"}else if b.0-maximum>error_max+error_b||b.0==maximum&&!strict&&error_max+error_b==0.0{"Satisfied"}else{"Uncertain"}
}

pub fn chronology(document:&Value)->String{
    let analysis=ChronologyAnalysis::new(document);let mut output=String::new();
    for event in temporal_rows(document,"events").iter().take(CHRONOLOGY_WORK/4){let range=event["id"].as_str().and_then(|id|analysis.range(id));output+=&format!("{}: {}\n",event["title"].as_str().unwrap_or("Event").chars().take(1000).collect::<String>(),range.map(|(a,b)|if a==b{format!("Day {a}")}else{format!("Days {a}–{b}")}).unwrap_or("Unknown or cyclic time (incomplete evidence remains unresolved)".into()));}
    for row in temporal_rows(document,"constraints").iter().take(CHRONOLOGY_WORK/4){
        let confirmed=row["confirmed"]==true;let component=row["beforeId"].as_str().zip(row["afterId"].as_str()).map(|(a,b)|analysis.order(a,b));
        let status=if confirmed&&component==Some(EventOrder::Conflict){"Contradiction"}else if confirmed&&component==Some(EventOrder::Unknown){"Unresolved"}else{constraint_comparison(&analysis,row)};
        output+=&format!("Constraint: {status}{} · {}\n",if confirmed{""}else{" (unconfirmed note; not enforced)"},row["notes"].as_str().unwrap_or("").chars().take(1000).collect::<String>());
    }
    for contributors in analysis.conflicts.values(){output+=&format!("Confirmed chronology contradiction · Contributing constraints: {}\n",contributors.join(", "));}
    if !analysis.unsupported.is_empty(){output+="Incomplete or unsupported chronology evidence remains unresolved.\n";}if analysis.limited(){output+="Chronology work limit reached; remaining feasibility and scope are unresolved.\n";}output
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
    fn temporal_fixture()->Value{json!({"events":[{"id":"a","title":"A","time":{"kind":"fixed","day":1}},{"id":"middle","title":"Middle","time":{"kind":"unknown"}},{"id":"b","title":"B","time":{"kind":"fixed","day":4}}],"constraints":[]})}
    fn temporal_edge(id:&str,from:&str,to:&str,days:f64,strict:bool)->Value{json!({"id":id,"beforeId":from,"afterId":to,"minimumDays":days,"strict":strict,"confirmed":true,"notes":"Authored constraint"})}
    #[test]fn confirmed_strict_same_day_order_is_contradiction_with_source_id(){
        let mut doc=temporal_fixture();doc["events"][2]["time"]["day"]=json!(1);doc["constraints"]=json!([temporal_edge("strict","a","b",0.,true)]);let before=doc.clone();let report=chronology(&doc);assert!(report.contains("Constraint: Contradiction"),"{report}");assert!(report.contains("Contributing constraints: strict"));assert_eq!(doc,before);
    }
    #[test]fn positive_and_strict_cycles_conflict_in_the_connected_component_only(){
        for (days,strict)in[(1.,false),(0.,true)]{let mut doc=temporal_fixture();doc["events"][0]["time"]=json!({"kind":"unknown"});doc["events"][2]["time"]=json!({"kind":"unknown"});doc["events"].as_array_mut().unwrap().extend([json!({"id":"outside1","time":{"kind":"fixed","day":1}}),json!({"id":"outside2","time":{"kind":"fixed","day":2}})]);doc["constraints"]=json!([temporal_edge("positive","a","middle",days,strict),temporal_edge("return","middle","a",0.,false),temporal_edge("linked","middle","b",0.,false)]);let index=ChronologyAnalysis::new(&doc);assert_eq!(index.order("a","b"),EventOrder::Conflict);let ids=index.contributors("b").unwrap();assert!(ids.iter().any(|id|id=="positive")&&ids.iter().any(|id|id=="return"));assert_eq!(index.order("outside1","outside2"),EventOrder::Before);}
    }
    #[test]fn zero_nonstrict_cycles_can_remain_feasible_and_unconfirmed_edges_are_not_enforced(){
        let mut doc=temporal_fixture();doc["events"][2]["time"]["day"]=json!(1);doc["constraints"]=json!([temporal_edge("out","a","b",0.,false),temporal_edge("back","b","a",0.,false)]);assert_eq!(ChronologyAnalysis::new(&doc).order("a","b"),EventOrder::Unordered);
        for confirmed in [Value::Bool(false),Value::Null]{doc["constraints"]=json!([temporal_edge("ignored","a","b",5.,true)]);if confirmed.is_null(){doc["constraints"][0].as_object_mut().unwrap().remove("confirmed");}else{doc["constraints"][0]["confirmed"]=confirmed;}let index=ChronologyAnalysis::new(&doc);assert_eq!(index.order("a","b"),EventOrder::Unordered);assert!(index.contributors("a").is_none());let report=chronology(&doc);assert!(report.contains("unconfirmed note; not enforced"));assert!(!report.contains("Confirmed chronology contradiction"));}
    }
    #[test]fn unknown_intermediary_propagates_feasibility_without_fabricating_a_date(){
        let mut doc=temporal_fixture();doc["constraints"]=json!([temporal_edge("first","a","middle",2.,false),temporal_edge("second","middle","b",2.,false)]);let original=doc.clone();let index=ChronologyAnalysis::new(&doc);assert_eq!(index.order("a","b"),EventOrder::Conflict);assert_eq!(index.contributors("b").unwrap(),["first","second"]);assert_eq!(index.range("middle"),None);assert_eq!(doc,original);doc["constraints"][1]["minimumDays"]=json!(1);let index=ChronologyAnalysis::new(&doc);assert_eq!(index.order("a","middle"),EventOrder::Before);assert_eq!(index.range("middle"),None);
    }
    #[test]fn malformed_times_and_confirmed_constraints_fail_closed_without_changing_legacy_data(){
        for time in [json!({}),json!({"kind":"fixed","day":null}),json!({"kind":"interval","start":10,"end":1}),json!({"kind":"relative","eventId":"missing","offsetDays":1}),json!({"kind":"relative","eventId":"middle","offsetDays":0})]{let mut doc=temporal_fixture();doc["events"][1]["time"]=time;let before=doc.clone();let index=ChronologyAnalysis::new(&doc);assert_eq!(index.order("a","middle"),EventOrder::Unknown);assert_eq!(index.range("middle"),None);assert_eq!(doc,before);}
        for value in [Value::Null,json!(-1),json!("many")]{let mut doc=temporal_fixture();doc["constraints"]=json!([temporal_edge("broken","a","b",0.,false)]);doc["constraints"][0]["minimumDays"]=value;assert_eq!(ChronologyAnalysis::new(&doc).order("a","b"),EventOrder::Unknown);assert!(chronology(&doc).contains("Constraint: Unresolved"));}
        let mut doc=temporal_fixture();doc["constraints"]=json!([temporal_edge("partial","a","b",0.,false)]);doc["constraints"][0].as_object_mut().unwrap().remove("strict");assert_eq!(ChronologyAnalysis::new(&doc).order("a","b"),EventOrder::Unknown);
    }
    #[test]fn finite_negative_fractional_relative_dates_and_interval_uncertainty_are_preserved(){
        let mut doc=temporal_fixture();doc["events"][0]["time"]=json!({"kind":"fixed","day":-0.5});doc["events"][1]["time"]=json!({"kind":"relative","eventId":"a","offsetDays":0.25});let index=ChronologyAnalysis::new(&doc);assert_eq!(index.range("middle"),Some((-0.25,-0.25)));doc["events"][0]["time"]=json!({"kind":"bounds","start":0,"end":10});doc["events"][2]["time"]=json!({"kind":"interval","start":5,"end":15});doc["constraints"]=json!([temporal_edge("ambiguous","a","b",0.,false)]);assert!(chronology(&doc).contains("Constraint: Uncertain"));let original=doc.clone();let index=ChronologyAnalysis::new(&doc);assert_eq!(index.range("a"),Some((0.,10.)));assert_eq!(index.range("b"),Some((5.,15.)));assert_eq!(doc,original);
        doc["constraints"]=json!([]);doc["events"][0]["time"]=json!({"kind":"fixed","day":1e308});doc["events"][1]["time"]=json!({"kind":"relative","eventId":"a","offsetDays":1e308});let index=ChronologyAnalysis::new(&doc);assert_eq!(index.range("middle"),None);assert_eq!(index.order("a","middle"),EventOrder::Unknown);assert!(!chronology(&doc).contains("Day inf"));
    }
    #[test]fn bounded_feasibility_labels_remaining_work_unknown(){let doc=json!({"events":(0..1000).map(|i|json!({"id":format!("e{i}"),"time":{"kind":"unknown"}})).collect::<Vec<_>>(),"constraints":(0..999).rev().map(|i|temporal_edge(&format!("c{i}"),&format!("e{i}"),&format!("e{}",i+1),1.,false)).collect::<Vec<_>>()});let index=ChronologyAnalysis::new(&doc);assert!(index.limited());assert_eq!(index.order("e0","e999"),EventOrder::Unknown);assert!(chronology(&doc).contains("work limit reached"));}

    #[test]fn unknown_relative_equalities_cannot_be_erased_by_confirmed_order(){
        for offset in [1.0,0.0]{let doc=json!({"events":[{"id":"u","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"u","offsetDays":offset}}],"constraints":[temporal_edge("reverse","v","u",0.0,true)]});let before=doc.clone();let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.order("v","u"),EventOrder::Conflict);let ids=analysis.contributors("u").unwrap();assert!(ids.iter().any(|id|id=="reverse"));assert!(ids.iter().any(|id|id.starts_with("relative event v")));assert_eq!(analysis.range("u"),None);assert_eq!(doc,before);}
    }
    #[test]fn signed_relative_paths_use_total_offset_and_balanced_cycles_stay_feasible(){
        let mut doc=json!({"events":[{"id":"u","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"u","offsetDays":10}},{"id":"w","time":{"kind":"relative","eventId":"v","offsetDays":-20}}],"constraints":[]});let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.order("u","v"),EventOrder::Before);assert_eq!(analysis.order("u","w"),EventOrder::After);assert!(analysis.contributors("v").is_none());assert_eq!(analysis.range("v"),None);
        doc["events"][1]["time"]["offsetDays"]=json!(0);let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.order("u","v"),EventOrder::Unknown);assert!(analysis.contributors("u").is_none());
    }
    #[test]fn duplicate_event_identity_and_oversized_input_fail_closed(){
        let mut doc=temporal_fixture();let duplicate=doc["events"][0].clone();doc["events"].as_array_mut().unwrap().push(duplicate);let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.range("a"),None);assert_eq!(analysis.order("a","b"),EventOrder::Unknown);doc["events"][3]["time"]=json!({"kind":"relative","eventId":"b","offsetDays":100});doc["constraints"]=json!([temporal_edge("ambiguous-order","a","b",1.,true)]);let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.order("a","b"),EventOrder::Unknown);assert!(analysis.contributors("a").is_none());
        let doc=json!({"events":(0..6000).map(|i|json!({"id":format!("e{i}"),"time":{"kind":"unknown"}})).collect::<Vec<_>>()});let analysis=ChronologyAnalysis::new(&doc);assert!(analysis.limited());assert_eq!(analysis.order("e0","e5999"),EventOrder::Unknown);assert!(chronology(&doc).contains("work limit reached"));
    }

    #[test]fn fractional_relative_roundtrip_does_not_invent_a_contradiction(){let doc=json!({"events":[{"id":"a","time":{"kind":"fixed","day":0.1}},{"id":"b","time":{"kind":"relative","eventId":"a","offsetDays":0.2}}],"constraints":[]});let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.order("a","b"),EventOrder::Before);assert!(analysis.contributors("a").is_none());}

    #[test]fn cancellation_uncertainty_does_not_prove_order_and_small_direct_offsets_still_do(){let mut doc=json!({"events":[{"id":"u","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"u","offsetDays":0.1}},{"id":"w","time":{"kind":"relative","eventId":"v","offsetDays":0.2}},{"id":"x","time":{"kind":"relative","eventId":"w","offsetDays":-0.3}}],"constraints":[]});let analysis=ChronologyAnalysis::new(&doc);assert!(!matches!(analysis.order("u","x"),EventOrder::Before|EventOrder::After));doc["events"][0]["time"]=json!({"kind":"fixed","day":0});let analysis=ChronologyAnalysis::new(&doc);assert!(!matches!(analysis.order("u","x"),EventOrder::Before|EventOrder::After));let doc=json!({"events":[{"id":"u","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"u","offsetDays":1e-100}}]});assert_eq!(ChronologyAnalysis::new(&doc).order("u","v"),EventOrder::Before);}
    #[test]fn numerically_uncertain_positive_cycle_with_large_offsets_never_applies_order(){let doc=json!({"events":[{"id":"u","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"u","offsetDays":1e12}}],"constraints":[temporal_edge("almost-return","v","u",0.0,true)]});let analysis=ChronologyAnalysis::new(&doc);assert_eq!(analysis.order("u","v"),EventOrder::Conflict);let mut doc=json!({"events":[{"id":"u","time":{"kind":"unknown"}},{"id":"v","time":{"kind":"relative","eventId":"u","offsetDays":1e12}},{"id":"w","time":{"kind":"relative","eventId":"v","offsetDays":-1e12}}],"constraints":[temporal_edge("tiny-return","w","u",1e-100,false)]});let analysis=ChronologyAnalysis::new(&doc);assert!(!matches!(analysis.order("u","w"),EventOrder::Before|EventOrder::After));doc["constraints"][0]["minimumDays"]=json!(1);assert_eq!(ChronologyAnalysis::new(&doc).order("u","w"),EventOrder::Conflict);}

    #[test]fn decimal_boundary_stays_uncertain_but_exact_integer_strict_gap_conflicts(){let mut doc=json!({"events":[{"id":"a","time":{"kind":"fixed","day":0.1}},{"id":"b","time":{"kind":"fixed","day":0.3}}],"constraints":[temporal_edge("gap","a","b",0.2,false)]});assert!(chronology(&doc).contains("Constraint: Uncertain"));assert!(ChronologyAnalysis::new(&doc).contributors("a").is_none());doc["events"][0]["time"]["day"]=json!(1);doc["events"][1]["time"]["day"]=json!(2);doc["constraints"][0]["minimumDays"]=json!(1);doc["constraints"][0]["strict"]=json!(true);assert_eq!(ChronologyAnalysis::new(&doc).order("a","b"),EventOrder::Conflict);}

}
