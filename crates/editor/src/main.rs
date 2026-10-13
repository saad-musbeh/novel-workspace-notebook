use novel_editor::{
    dialog,engine::NativeEditor
};
use serde_json::{
    json,Value
};
use std::{
    io::{
        self,BufRead,Read,Write
    },path::PathBuf
};
fn main(){
    if let Err(error)=run(){
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run()->Result<(),String>{
    let argument=std::env::args_os().nth(1).ok_or("Missing private editor directory.")?;
    let choosing=argument=="--choose";
    let directory=PathBuf::from(argument);
    if !choosing{
        std::fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
    }
    let stdin=io::stdin();
    let mut input=stdin.lock();
    let stdout=io::stdout();
    let mut output=stdout.lock();
    let mut editor=None;
    loop{
        let mut line=Vec::new();
        let limit=if choosing{
            4096
        }else{
            2_100_000
        };
        if (&mut input).take(limit+1).read_until(b'\n',&mut line).map_err(|e|e.to_string())?==0{
            break;
        }
        if line.len() as u64>limit || line.last()!=Some(&b'\n'){
            return Err("Editor request too large or incomplete.".into());
        }
        let reply=(||->Result<Value,String>{
            let request:Value=serde_json::from_slice(&line).map_err(|_|"Invalid editor request.")?;
            if choosing{
                let mut response=dialog::choose(&request)?;
                response["ok"]=json!(true);
                return Ok(response);
            }
            let frame=match request["type"].as_str(){
                Some("open")=>{
                    let mut candidate=NativeEditor::new(&directory,request.clone())?;
                    let frame=candidate.request(&request)?;
                    editor=Some(candidate);
                    frame
                },                 Some("resume")=>{
                    let mut candidate=NativeEditor::resume(&directory)?;
                    let top=candidate.top;
                    let height=candidate.height;
                    let frame=candidate.request(&json!({
                        "type":"snapshot","top":top,"viewportHeight":height
                    }))?;
                    editor=Some(candidate);
                    frame
                },                 _=>editor.as_mut().ok_or("The native manuscript editor is not open.")?.request(&request)?,
            };
            Ok(json!({
                "ok":true,"frame":frame
            }))
        })().unwrap_or_else(|error|json!({
            "ok":false,"error":error
        }));
        serde_json::to_writer(&mut output,&reply).map_err(|e|e.to_string())?;
        output.write_all(b"\n").and_then(|_|output.flush()).map_err(|e|e.to_string())?;
        if choosing{
            break;
        }
    }
    Ok(())
}
