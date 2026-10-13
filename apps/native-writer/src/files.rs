use gtk::prelude::*;
use std::{
    fs::{
        self,File,OpenOptions
    },io::{
        Read,Write
    },path::{
        Path,PathBuf
    }
};
pub const LIMIT:usize=64*1024*1024;
pub fn choose(parent:&gtk::Window,saving:bool,name:&str,patterns:&[&str])->Option<PathBuf>{
    let dialog=gtk::FileChooserNative::new(Some(if saving{
        "Save a local file"
    }else{
        "Open a local file"
    }),Some(parent),if saving{
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
        dialog.set_current_name(name);
        dialog.set_do_overwrite_confirmation(true);
    }
    let filter=gtk::FileFilter::new();
    filter.set_name(Some(name));
    for pattern in patterns{
        filter.add_pattern(pattern);
    }dialog.add_filter(filter);
    let path=if dialog.run()==gtk::ResponseType::Accept{
        dialog.filename()
    }else{
        None
    };
    dialog.destroy();
    path
}
pub fn write(path:&Path,bytes:&[u8])->Result<(),String>{
    if bytes.len()>LIMIT{
        return Err("The export exceeds 64 MiB.".into());
    }
    let parent=path.parent().ok_or("Choose a destination folder.")?;
    let temporary=parent.join(format!(".novel-{}.pending",uuid::Uuid::new_v4()));
    let result=(||->Result<(),String>{
        let mut options=OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]{
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file=options.open(&temporary).map_err(|e|e.to_string())?;
        file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;
        drop(file);
        fs::rename(&temporary,path).map_err(|e|e.to_string())?;
        File::open(parent).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;
        Ok(())
    })();
    if temporary.exists(){
        let _=fs::remove_file(temporary);
    }result
}
pub fn read(path:&Path)->Result<String,String>{
    let file=File::open(path).map_err(|e|e.to_string())?;
    let metadata=file.metadata().map_err(|e|e.to_string())?;
    if !metadata.is_file() || metadata.len()>LIMIT as u64{
        return Err("Choose a UTF-8 file no larger than 64 MiB.".into());
    }
    let mut bytes=Vec::new();
    file.take(LIMIT as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>LIMIT{
        return Err("The import exceeds 64 MiB.".into());
    }String::from_utf8(bytes).map_err(|_|"The import must be UTF-8 text.".into())
}
