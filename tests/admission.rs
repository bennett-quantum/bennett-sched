use bennett_sched::{canonical,object::Object,schedule::Document,Status};
use serde_json::{Value,json};

fn controlled()->(Object,Value){let root="../bennett-isa-spec/conformance/fixtures/controlled-action-09";(Object::parse(&std::fs::read(format!("{root}/controlled.bisa")).unwrap()).unwrap(),canonical::parse(&std::fs::read(format!("{root}/schedule.json")).unwrap()).unwrap())}

#[test]
fn every_path_is_checked_against_the_source_call_chain_and_loop_depth() {
    let (object,schedule)=controlled();Document::parse(schedule.clone()).unwrap().validate_paths(&object).unwrap();
    for malformed in 0..5{let mut v=schedule.clone();match malformed{0=>v["entries"][0]["path"][1]["point"]=json!("entry:100"),1=>v["entries"][0]["path"][0]["symbol"]=v["entries"][0]["path"][1]["symbol"].clone(),2=>v["entries"][0]["path"][1]["iteration"]=json!([0]),3=>v["entries"][0]["path"][0]["call_index"]=json!(1),_=>v["entries"][0]["path"][1]["instance"]=json!([0])};let e=Document::parse(v).unwrap().validate_paths(&object).unwrap_err();assert_eq!(e.status,Status::ScheduleInfeasible,"{malformed}: {e}");}
}

#[test]
fn published_nonexistent_inc8_row_rejects_and_reserved_oracle_points_are_valid() {
    let spec=std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();let bytes=spec.split("### 16.3 Example objects").nth(1).unwrap().split("```").nth(1).unwrap().trim_start_matches('\n');let object=Object::parse(bytes.as_bytes()).unwrap();let (_,mut schedule)=controlled();let hash=object.closure_hash("inc8").unwrap();schedule["bindings"]["closure_hash"]=json!(hash);schedule["bindings"]["region"]=json!("inc8");schedule["entries"]=json!([{"path":[{"symbol":hash,"point":"entry:3","iteration":[]}],"step":"*","phase":"*","generated":"*","kind":"start","start":{"time":0}}]);
    let case:Value=canonical::parse(&std::fs::read("../bennett-isa-spec/conformance/cases/H/H-path-not-in-object/case.json").unwrap()).unwrap();schedule["entries"][0]["path"][0]["point"]=case["scenario"]["point"].clone();assert_eq!(Document::parse(schedule.clone()).unwrap().validate_paths(&object).unwrap_err().status,Status::ScheduleInfeasible);
    for point in ["entry:0","$construct","$step","$commit","$release"]{schedule["entries"][0]["path"][0]["point"]=json!(point);Document::parse(schedule.clone()).unwrap().validate_paths(&object).unwrap();}
    let (action,mut a)=controlled();a["entries"][0]["path"]=json!([{"symbol":a["bindings"]["closure_hash"],"point":"$construct","iteration":[]}]);assert!(Document::parse(a).unwrap().validate_paths(&action).is_err());
}
