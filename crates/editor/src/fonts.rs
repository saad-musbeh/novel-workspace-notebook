//! Register checked project fonts only in this disposable editor process.
use crate::model::Result;
use serde_json::Value;
use sha2::{
    Digest, Sha256
};
use std::{
    ffi::{
        c_char,c_int,c_void,CStr,CString
    },fs,io::Read,path::Path
};
#[link(name="fontconfig")] extern "C" {
    fn FcConfigGetCurrent() -> *mut c_void;
    fn FcConfigAppFontAddFile(config:*mut c_void,file:*const u8) -> c_int;
    fn FcFreeTypeQuery(file:*const u8,index:u32,blanks:*mut c_void,count:*mut c_int) -> *mut c_void;
    fn FcPatternGetString(pattern:*mut c_void,object:*const c_char,index:c_int,value:*mut *mut u8) -> c_int;
    fn FcPatternDestroy(pattern:*mut c_void);
}
pub fn register(directory:&Path,fonts:&Value) -> Result<Vec<String>> {
    let fonts=fonts.as_array().filter(|v|v.len()<=8).ok_or("Invalid project font list.")?;
    if fonts.is_empty(){
        return Ok(Vec::new());
    }
    let root=directory.join("fonts").canonicalize().map_err(|_|"The project font folder is unavailable.")?;
    let mut validated=Vec::new();
    for font in fonts {
        let path=Path::new(font["path"].as_str().ok_or("Invalid project font asset.")?).canonicalize().map_err(|_|"The project font is unavailable.")?;
        let metadata=fs::metadata(&path).map_err(|e|e.to_string())?;
        if !path.starts_with(&root) || !metadata.is_file() || metadata.len()>16*1024*1024{
            return Err("Project font is outside the private editor session or exceeds its size limit.".into());
        }
        let mut content=Vec::new();
        fs::File::open(&path).and_then(|f|f.take(16*1024*1024+1).read_to_end(&mut content)).map_err(|e|e.to_string())?;
        if content.len()>16*1024*1024 || content.len()<12 || ![b"\0\x01\0\0",b"OTTO",b"true"].contains(&(&content[..4]).try_into().unwrap()){
            return Err("Project font must be a single-font TTF or OTF.".into());
        }
        if format!("{:x}",Sha256::digest(&content))!=font["checksum"].as_str().unwrap_or(""){
            return Err("Project font checksum failed. Original manuscript text is preserved.".into());
        }
        let family=font["family"].as_str().filter(|s|!s.trim().is_empty() && s.chars().count()<=200 && !s.chars().any(|c|c<' ')).ok_or("Invalid project font family.")?;
        validated.push((path,family.to_owned()));
    }
    // Fontconfig objects remain private to this process. Query patterns are
    // destroyed on every success/error branch; returned strings are borrowed.
    unsafe {
        let config=FcConfigGetCurrent();
        if config.is_null(){
            return Err("The installed font engine is unavailable.".into());
        }
        for (path,family) in &validated {
            use std::os::unix::ffi::OsStrExt;
            let path=CString::new(path.as_os_str().as_bytes()).map_err(|_|"Invalid project font path.")?;
            let mut count=0;
            let pattern=FcFreeTypeQuery(path.as_ptr().cast(),0,std::ptr::null_mut(),&mut count);
            if pattern.is_null(){
                return Err("The imported project font could not be read by Fontconfig.".into());
            }
            let mut matched=false;
            let mut index=0;
            let mut name=std::ptr::null_mut();
            while FcPatternGetString(pattern,b"family\0".as_ptr().cast(),index,&mut name)==0 {
                if !name.is_null() && CStr::from_ptr(name.cast()).to_bytes()==family.as_bytes(){
                    matched=true;
                }index+=1;
            }
            FcPatternDestroy(pattern);
            if !matched{
                return Err("The imported project font family does not match its definition.".into());
            }
            if FcConfigAppFontAddFile(config,path.as_ptr().cast())==0{
                return Err("The imported project font could not be loaded in this editor process.".into());
            }
        }
    }
    Ok(validated.into_iter().map(|(_,family)|family).collect())
}
pub fn writing_system(selection:&Value,families:&[String]) -> Result<Option<String>> {
    if selection.is_null(){
        return Ok(None);
    }
    if !selection.is_object() || !selection["definitionId"].as_str().is_some_and(|s|!s.is_empty()) || !selection["mappingVersion"].as_u64().is_some_and(|n|n>0) || !["auto","ltr","rtl"].contains(&selection["direction"].as_str().unwrap_or("")){
        return Err("Invalid selected alphabet definition or mapping version.".into());
    }
    if selection["fontFamily"].is_null(){
        return Ok(None);
    }
    let family=selection["fontFamily"].as_str().filter(|s|families.iter().any(|f|f==s)).ok_or("The selected alphabet font is unavailable in this project. Original text is preserved.")?;
    Ok(Some(family.into()))
}
