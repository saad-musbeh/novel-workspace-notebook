//! Saved interface preferences never change manuscript paragraph attributes.
use serde::{Deserialize,Serialize};
use serde_json::Value;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="lowercase")]
pub enum NativeDeskDirection { Ltr,Rtl }
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeDeskPreferences {
    pub schema_version:u32,
    pub direction:NativeDeskDirection,
    pub writing_width:u32,
}
impl Default for NativeDeskPreferences {fn default()->Self{Self{schema_version:1,direction:NativeDeskDirection::Ltr,writing_width:0}}}
impl NativeDeskPreferences {
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=1{return Err("Unsupported native desk preferences version.".into())}
        if self.writing_width!=0&&!(360..=1200).contains(&self.writing_width){return Err("Writing width must be Fill page or 360–1200 pixels.".into())}Ok(())
    }
    pub fn from_project_settings(value:&Value)->Result<Self,String>{
        let Some(desk)=value.get("nativeDesk")else{return Ok(Self::default())};
        let parsed:Self=serde_json::from_value(desk.clone()).map_err(|_|"Invalid native desk preferences fields.")?;parsed.validate()?;Ok(parsed)
    }
}
