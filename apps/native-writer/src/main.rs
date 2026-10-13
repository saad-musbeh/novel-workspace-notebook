mod process;
mod files;
mod editor;
mod tools;
mod app;
mod export;
mod planning;
mod alphabets;
mod local_resources;
use std::path::PathBuf;
fn main(){
    if let Err(error)=run(){
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run()->Result<(),String>{
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg|arg=="--help"){
        println!("Novel Workspace (Rust) 0.4.0\nUsage: novel-workspace-rust [--no-focus]\n--no-focus opens without taking keyboard focus, for isolated desktop checks.");
        return Ok(());
    }
    if args.iter().any(|arg|arg!="--no-focus"){
        return Err("Unknown argument. Use --help for usage.".into());
    }
    gtk::init().map_err(|_|"A graphical desktop session is required.")?;
    let data=std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")).join("local.novel.workspace.spike");
    let cache=std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")).join("local.novel.workspace.spike");
    std::fs::create_dir_all(&data).and_then(|_|std::fs::create_dir_all(&cache)).map_err(|e|e.to_string())?;
    let executable=std::env::current_exe().map_err(|e|e.to_string())?;
    let binaries=executable.parent().ok_or("Missing executable folder.")?;
    let _app=if args.is_empty(){app::Workspace::new(&data,&cache,binaries)?}else{app::Workspace::new_with_focus(&data,&cache,binaries,false)?};
    gtk::main();
    Ok(())
}
