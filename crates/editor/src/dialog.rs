use crate::model::Result;
use gtk::prelude::*;
use serde_json::{
    json,Value
};
pub fn choose(request:&Value)->Result<Value>{
    let saving=match request["mode"].as_str(){
        Some("save")=>true,Some("open")=>false,_=>return Err("Unsupported file selection.".into())
    };
    let (name,patterns):(&str,&[&str])=match request["format"].as_str(){
        Some("docx")=>("Word manuscript (.docx)",&["*.docx"]),Some("text")=>("Plain UTF-8 text",&["*.txt"]),Some("markdown")=>("Markdown",&["*.md","*.markdown"]),Some("html")=>("Self-contained HTML",&["*.html","*.htm"]),Some("project")=>("Novel Workspace project archive",&["*.novel.json","*.json"]),Some("alphabet")=>("Alphabet definitions",&["*.alphabet.json","*.json"]),Some("dictionary")=>("Local dictionary data",&["*.json"]),_=>return Err("Unsupported file selection.".into())
    };
    let suggested=request["suggestedName"].as_str().unwrap_or("document");
    if suggested.is_empty() || suggested.chars().count()>200 || [".",".."].contains(&suggested) || suggested.chars().any(|c|c.is_control() || c=='/' || c=='\\'){
        return Err("Invalid suggested filename.".into());
    }
    gtk::init().map_err(|_|"The desktop file chooser is unavailable. Keep the document open and try again in your desktop session.")?;
    let dialog=gtk::FileChooserNative::new(Some(if saving{
        "Save a local file"
    }else{
        "Open a local file"
    }),None::<&gtk::Window>,if saving{
        gtk::FileChooserAction::Save
    }else{
        gtk::FileChooserAction::Open
    },Some(if saving{
        "Save"
    }else{
        "Open"
    }),Some("Cancel"));
    dialog.set_modal(true);
    dialog.set_local_only(true);
    if saving{
        dialog.set_current_name(suggested);
        dialog.set_do_overwrite_confirmation(true);
    }
    let filter=gtk::FileFilter::new();
    filter.set_name(Some(name));
    for pattern in patterns{
        filter.add_pattern(pattern);
    }dialog.add_filter(filter);
    let response=dialog.run();
    let path=if response==gtk::ResponseType::Accept{
        dialog.filename()
    }else{
        None
    };
    dialog.destroy();
    Ok(match path{
        Some(path)=>json!({
            "cancelled":false,"path":path
        }),None=>json!({
            "cancelled":true
        })
    })
}
