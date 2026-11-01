use bennett_sched::{object::Object,source::Trace,expansion::{Operation,simulate_basis},schedule::Phase};
use serde_json::json;
use std::collections::BTreeMap;
mod common;
fn increment(width:u64)->Object{let sig=json!({"abi":"0.9","tier":"q","kind":"oracle","in":[width],"tgt":width,"fault":4,"mem":[],"args":[]});let function=format!("oracle @inc (x:wire[{width}] in) (t:wire[{width}]) (f:wire[4]) () ()\nregs {{ one:reg[{width}], r:reg[{width}] }}");Object::parse(&common::action("oracle","inc",sig,&function,&format!("one = lift.const.i{width} 1\nr = lift.add x one\nlift.retval r"))).unwrap()}

#[test]
fn padded_oracle_commits_the_reference_result_or_budget_fault_and_cleans_all_state(){
    let object=increment(2);
    for budget in 0..=4{let trace=Trace::build(&object,"inc",1,&json!([]),[budget,0,0,0]).unwrap();let leaves=trace.events.iter().map(|e|e.leaf.clone()).collect::<Vec<_>>();
        for input in 0..4{for target in 0..4{for fault in 0..16{for control in [false,true]{let mut bits=BTreeMap::new();for bit in 0..2{bits.insert(format!("in:0[{bit}]"),input&(1<<bit)!=0);bits.insert(format!("tgt:0[{bit}]"),target&(1<<bit)!=0);}for bit in 0..4{bits.insert(format!("fault:0[{bit}]"),fault&(1<<bit)!=0);}bits.insert("control:0".into(),control);simulate_basis(&leaves,&mut bits).unwrap();assert_eq!(bits.len(),9);let get=|name:&str,n:u64|(0..n).map(|i|u64::from(bits[&format!("{name}[{i}]")])<<i).sum::<u64>();assert_eq!(get("in:0",2),input);assert_eq!(get("tgt:0",2),target^if control&&budget>=3{(input+1)%4}else{0});assert_eq!(get("fault:0",4),fault^u64::from(control&&budget<3));}}}}
        for point in ["$construct","$commit","$release"]{let events=trace.events.iter().filter(|e|e.location.path.last().unwrap().point==point).collect::<Vec<_>>();assert!(!events.is_empty());assert!(events.iter().all(|e|e.location.root.step.is_none()&&e.location.root.phase.is_none()));assert_eq!(events.iter().map(|e|e.location.generated).collect::<Vec<_>>(),(0..events.len() as u64).collect::<Vec<_>>());}
        for step in 1..=budget{let forward=trace.events.iter().filter(|e|e.location.root.step==Some(step)&&e.location.root.phase==Some(Phase::Fwd)).collect::<Vec<_>>();let reverse=trace.events.iter().filter(|e|e.location.root.step==Some(step)&&e.location.root.phase==Some(Phase::Rev)).collect::<Vec<_>>();assert_eq!(forward.len(),reverse.len());for (a,b) in forward.iter().zip(reverse.iter().rev()){assert_eq!(a.location.path,b.location.path);assert_eq!(a.location.generated,b.location.generated);}}
        assert!(trace.events.iter().all(|e|!matches!(e.leaf.op,Operation::Classical{..})));
    }
}
