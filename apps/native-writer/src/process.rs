//! Private stdio workers. IO runs outside GTK; storage replies may arrive out of order.
use glib::{
    ControlFlow,MainContext,Priority
};
use serde_json::{
    json,Value
};
use std::{
    collections::{
        HashMap,VecDeque
    },io::{
        BufRead,BufReader,Read,Write
    },path::Path,process::{
        Command,Stdio
    },sync::{
        mpsc,Arc,Mutex,atomic::{
            AtomicBool,Ordering
        }
    }
};
pub type Reply=Result<Value,String>;
struct Request {
    value:Value,reply:glib::Sender<Reply>
}
#[derive(Default)]struct Pending{
    replies:HashMap<u64,glib::Sender<Reply>>,order:VecDeque<u64>
}
#[derive(Clone)]pub struct Process{
    sender:mpsc::Sender<Option<Request>>
}
impl Process{
    pub fn start(executable:&Path,argument:&Path,companion:bool)->Result<Self,String>{
        let mut child=Command::new(executable).arg(argument).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().map_err(|e|format!("Could not start {}: {e}",executable.display()))?;
        let mut input=child.stdin.take().ok_or("Missing local input.")?;
        let mut output=BufReader::new(child.stdout.take().ok_or("Missing local output.")?);
        let pending=Arc::new(Mutex::new(Pending::default()));
        let replies=pending.clone();
        let alive=Arc::new(AtomicBool::new(true));
        let reader_alive=alive.clone();
        std::thread::spawn(move||{
            loop{
                let mut bytes=Vec::new();
                if (&mut output).take(128_000_001).read_until(b'\n',&mut bytes).is_err() || bytes.len()>128_000_000 || bytes.last()!=Some(&b'\n'){
                    break;
                }
                let Ok(result)=serde_json::from_slice::<Value>(&bytes)else{
                    break;
                };
                let Ok(mut pending)=replies.lock()else{
                    break;
                };
                let id=if companion{
                    result["id"].as_u64()
                }else{
                    pending.order.pop_front()
                };
                let Some(id)=id else{
                    break;
                };
                if companion{
                    pending.order.retain(|i|*i!=id);
                }
                let Some(sender)=pending.replies.remove(&id)else{
                    break;
                };
                let response=if result["ok"]==true{
                    Ok(result[if companion{
                        "value"
                    }else{
                        "frame"
                    }].clone())
                }else{
                    Err(result["error"].as_str().unwrap_or("Local request failed.").into())
                };
                let _=sender.send(response);
            }
            reader_alive.store(false,Ordering::Release);
            if let Ok(mut pending)=replies.lock(){
                for (_,sender) in pending.replies.drain(){
                    let _=sender.send(Err("Local process stopped. Your confirmed revisions remain on disk.".into()));
                }
            }
        });
        let (sender,receiver)=mpsc::channel::<Option<Request>>();
        std::thread::spawn(move||{
            let mut id=0u64;
            for request in receiver{
                let Some(request)=request else{
                    break;
                };
                id+=1;
                let value=if companion{
                    json!({
                        "id":id,"op":request.value["op"],"args":request.value["args"]
                    })
                }else{
                    request.value
                };
                let mut bytes=match serde_json::to_vec(&value){
                    Ok(b)=>b,Err(e)=>{
                        let _=request.reply.send(Err(e.to_string()));
                        continue;
                    }
                };
                if bytes.len()>if companion{
                    128_000_000
                }else{
                    2_100_000
                }{
                    let _=request.reply.send(Err("Local request exceeds its limit.".into()));
                    continue;
                }bytes.push(b'\n');
                {
                    let Ok(mut p)=pending.lock()else{
                        break;
                    };
                    if !alive.load(Ordering::Acquire){
                        let _=request.reply.send(Err("Local process is unavailable. Reopen the app.".into()));
                        continue;
                    }if p.replies.len()>=512{
                        let _=request.reply.send(Err("The local request queue is full. Try again after pending work finishes.".into()));
                        continue;
                    }p.replies.insert(id,request.reply);
                    p.order.push_back(id);
                }
                if input.write_all(&bytes).and_then(|_|input.flush()).is_err(){
                    break;
                }
            }
            alive.store(false,Ordering::Release);
            drop(input);
            let _=child.kill();
            let _=child.wait();
            if let Ok(mut p)=pending.lock(){
                for (_,sender)in p.replies.drain(){
                    let _=sender.send(Err("Local process stopped. Your confirmed revisions remain on disk.".into()));
                }
            }
        });
        Ok(Self{
            sender
        })
    }
    #[allow(deprecated)]     pub fn request(&self,value:Value,callback:impl FnOnce(Reply)+'static){
        let (sender,receiver)=MainContext::channel(Priority::DEFAULT);
        let mut callback=Some(callback);
        receiver.attach(None,move|reply|{
            if let Some(callback)=callback.take(){
                callback(reply);
            }ControlFlow::Break
        });
        if self.sender.send(Some(Request{
            value,reply:sender.clone()
        })).is_err(){
            let _=sender.send(Err("Local process is unavailable. Reopen the app.".into()));
        }
    }
    pub fn rpc(&self,op:&str,args:Value,callback:impl FnOnce(Reply)+'static){
        self.request(json!({
            "op":op,"args":args
        }),callback);
    }
    pub fn stop(&self){
        let _=self.sender.send(None);
    }
}
