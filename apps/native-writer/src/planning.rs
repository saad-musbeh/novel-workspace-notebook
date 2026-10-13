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
pub fn board(page:Rc<DataPage>,parent:gtk::Window)->gtk::Box{
    let root=gtk::Box::new(gtk::Orientation::Vertical,6);
    let controls=gtk::Box::new(gtk::Orientation::Vertical,6);
    let combo=gtk::ComboBoxText::new();
    let refresh=gtk::Button::with_label("Refresh boards");
    let add=gtk::Button::with_label("Add note card");
    controls.pack_start(&combo,true,true,0);
    controls.pack_start(&refresh,false,false,0);
    controls.pack_start(&add,false,false,0);
    root.pack_start(&controls,false,false,0);
    let area=gtk::DrawingArea::new();
    area.set_size_request(2800,1800);
    area.add_events(gdk::EventMask::BUTTON_PRESS_MASK|gdk::EventMask::BUTTON_RELEASE_MASK|gdk::EventMask::POINTER_MOTION_MASK);
    let scroll=gtk::ScrolledWindow::new(None::<&gtk::Adjustment>,None::<&gtk::Adjustment>);
    scroll.add(&area);
    root.pack_start(&scroll,true,true,0);
    let p=page.clone();
    let c=combo.clone();
    refresh.connect_clicked(move|_|{
        c.remove_all();
        for (i,board) in p.value.borrow()["boards"].as_array().into_iter().flatten().enumerate(){
            c.append(Some(&i.to_string()),board["name"].as_str().unwrap_or("Board"));
        }c.set_active(Some(0));
    });
    let a=area.clone();
    combo.connect_changed(move|_|a.queue_draw());
    let p=page.clone();
    let c=combo.clone();
    area.connect_draw(move|area,context|{
        let data=p.value.borrow();
        let index=c.active_id().and_then(|s|s.parse::<usize>().ok()).unwrap_or(0);
        let board=&data["boards"][index];
        context.set_source_rgb(1.,1.,1.);
        let _=context.paint();
        let cards=board["cards"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        for edge in board["edges"].as_array().into_iter().flatten(){
            let from=cards.iter().find(|c|c["id"]==edge["fromId"]);
            let to=cards.iter().find(|c|c["id"]==edge["toId"]);
            if let(Some(from),Some(to))=(from,to){
                context.set_source_rgb(0.6,0.65,0.7);
                context.move_to(from["x"].as_f64().unwrap_or(0.)+110.,from["y"].as_f64().unwrap_or(0.)+55.);
                context.line_to(to["x"].as_f64().unwrap_or(0.)+110.,to["y"].as_f64().unwrap_or(0.)+55.);
                let _=context.stroke();
            }
        }
        for card in cards.iter().take(2000){
            let x=card["x"].as_f64().unwrap_or(20.);
            let y=card["y"].as_f64().unwrap_or(20.);
            context.set_source_rgb(0.98,0.98,0.98);
            context.rectangle(x,y,220.,110.);
            let _=context.fill();
            context.set_source_rgb(0.75,0.75,0.75);context.rectangle(x,y,220.,110.);let _=context.stroke();
            context.set_source_rgb(0.12,0.15,0.13);
            text(area,context,card["title"].as_str().unwrap_or("Card"),x+10.,y+8.,200);
            text(area,context,&card["body"].as_str().unwrap_or("").chars().take(110).collect::<String>(),x+10.,y+40.,200);
        }
        glib::Propagation::Proceed
    });
    let dragging=Rc::new(RefCell::new(None::<(usize,usize,f64,f64)>));
    let p=page.clone();
    let c=combo.clone();
    let d=dragging.clone();
    area.connect_button_press_event(move|_,event|{
        let(x,y)=event.position();
        let index=c.active_id().and_then(|s|s.parse::<usize>().ok()).unwrap_or(0);
        let data=p.value.borrow();
        if let Some((i,card))=data["boards"][index]["cards"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().enumerate().rev().find(|(_,card)|{
            let cx=card["x"].as_f64().unwrap_or(0.);
            let cy=card["y"].as_f64().unwrap_or(0.);
            x>=cx&&x<=cx+220.&&y>=cy&&y<=cy+110.
        }){
            *d.borrow_mut()=Some((index,i,x-card["x"].as_f64().unwrap_or(0.),y-card["y"].as_f64().unwrap_or(0.)));
        }glib::Propagation::Stop
    });
    let p=page.clone();
    let d=dragging.clone();
    area.connect_motion_notify_event(move|a,event|{
        if let Some((board,card,dx,dy))=*d.borrow(){
            if event.state().contains(gdk::ModifierType::BUTTON1_MASK){
                let(x,y)=event.position();
                let mut data=p.value.borrow_mut();
                data["boards"][board]["cards"][card]["x"]=json!((x-dx).clamp(0.,2580.));
                data["boards"][board]["cards"][card]["y"]=json!((y-dy).clamp(0.,1690.));
                p.dirty.set(true);
                p.epoch.set(p.epoch.get()+1);
                a.queue_draw();
            }
        }glib::Propagation::Stop
    });
    area.connect_button_release_event(move|_,_|{
        *dragging.borrow_mut()=None;
        glib::Propagation::Stop
    });
    let p=page;
    let a=area;
    add.connect_clicked(move|button|{
        let parent=button.toplevel().and_then(|w|w.downcast::<gtk::Window>().ok()).unwrap_or_else(||parent.clone());
        let Some(title)=prompt(&parent,"New plot card","New note")else{
            return;
        };
        let index=combo.active_id().and_then(|s|s.parse::<usize>().ok()).unwrap_or(0);
        let mut data=p.value.borrow_mut();
        if let Some(cards)=data["boards"][index]["cards"].as_array_mut(){
            let i=cards.len();
            cards.push(json!({
                "id":uuid::Uuid::new_v4().to_string(),"title":title,"body":"","kind":"note","reference":null,"x":20+(i%8)*250,"y":20+(i/8)*135,"groupId":null,"labels":[]
            }));
            p.dirty.set(true);
            p.epoch.set(p.epoch.get()+1);
            a.queue_draw();
        }
    });
    refresh.clicked();
    root
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
