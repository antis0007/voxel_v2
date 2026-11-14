//! Bounded interning for legacy presentation fields typed as static strings.
//! Repeated snapshots reuse an entry; remote data cannot grow the table forever.
use serde::Deserialize;
use std::{collections::HashSet,sync::{Mutex,OnceLock}};
fn intern(value:String)->Result<&'static str,String> {
    if value.len()>512 {return Err("network label too long".into());}
    static LABELS:OnceLock<Mutex<HashSet<&'static str>>>=OnceLock::new();
    let mut labels=LABELS.get_or_init(Default::default).lock().map_err(|_|"network label lock poisoned")?;
    if let Some(&s)=labels.get(value.as_str()){return Ok(s);}
    if labels.len()>=4096{return Err("network label limit".into());}
    let s:&'static str=Box::leak(value.into_boxed_str());labels.insert(s);Ok(s)
}
pub fn string<'de,D:serde::Deserializer<'de>>(d:D)->Result<&'static str,D::Error>{
    intern(String::deserialize(d)?).map_err(serde::de::Error::custom)
}
pub fn optional<'de,D:serde::Deserializer<'de>>(d:D)->Result<Option<&'static str>,D::Error>{
    Option::<String>::deserialize(d)?.map(intern).transpose().map_err(serde::de::Error::custom)
}
