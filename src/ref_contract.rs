//! The single boundary for the unresolved `ref` helper binding contract (enz).
//! Named recipes stay closed. No dynamic caller environment is supplied here.
use crate::{Error,Result,Status,expansion::Env};
use serde_json::Value;
use std::collections::BTreeSet;

/// Detect undeclared wire bases in legacy helpers. This audit deliberately
/// over-approximates local declarations; the compiler still checks lexical
/// lifetimes and every actual type. It is not a substitute for those checks.
pub fn unbound_wires(recipe:&Value)->BTreeSet<String>{
    fn declarations(v:&Value,names:&mut BTreeSet<String>){match v{Value::Object(o)=>{
        for key in ["name","alloc","declare","for","each"]{if let Some(s)=o.get(key).and_then(Value::as_str){names.insert(s.into());}}
        for key in ["parameters","fields"]{if let Some(a)=o.get(key).and_then(Value::as_array){for p in a{if let Some(n)=p.as_str(){names.insert(n.into());}}}}
        for v in o.values(){declarations(v,names);}
    },Value::Array(a)=>for v in a{declarations(v,names);},_=>{}}}
    fn bases(v:&Value,out:&mut BTreeSet<String>){match v{Value::String(s)=>{out.insert(s.split('[').next().unwrap_or(s).into());},Value::Array(a)=>for v in a{bases(v,out);},Value::Object(o)=>if let Some(s)=o.get("range").and_then(Value::as_str){out.insert(s.into());},_=>{}}}
    fn uses(v:&Value,out:&mut BTreeSet<String>){match v{Value::Object(o)=>{if o.contains_key("gate"){if let Some(w)=o.get("wires"){bases(w,out);}}for v in o.values(){uses(v,out);}},Value::Array(a)=>for v in a{uses(v,out);},_=>{}}}
    let mut declared=BTreeSet::new();declarations(recipe,&mut declared);let mut read=BTreeSet::new();uses(&recipe["body"],&mut read);read.retain(|n|!declared.contains(n));read
}
pub(crate) fn named_scope(recipe:&Value,bound:Env)->Result<Env>{
    let missing=unbound_wires(recipe);
    if !missing.is_empty(){return Err(Error::new(Status::GateUnsupported,format!("§11.1 K19: ref recipe {} has unbound wires {}; binding contract blocked by bennett-sched-enz",recipe["kind"].as_str().unwrap_or("<unnamed>"),missing.into_iter().collect::<Vec<_>>().join(", "))));}
    Ok(bound)
}
