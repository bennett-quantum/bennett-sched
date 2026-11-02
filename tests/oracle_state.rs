use bennett_sched::{object::Object, oracle_state::{Plan, ByteStorage}};
use num_bigint::BigUint;
use serde_json::json;
fn example(index: usize) -> Object {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let source = spec.split("### 16.3 Example objects").nth(1).unwrap()
        .split("## 17.").next().unwrap().split("```").skip(1).step_by(2)
        .map(|s| s.trim_start_matches('\n')).filter(|s| s.starts_with("bennett-object "))
        .nth(index - 1).unwrap();
    Object::parse(source.as_bytes()).unwrap()
}
#[test]
fn forwarded_example_has_reference_counters_locations_frames_and_unaliased_byte_map() {
    let object = example(10);
    let plan = Plan::form(&object, "through", [3,0,16,1]).unwrap();
    assert_eq!(plan.slot_widths, [4,5,5,2]);
    assert_eq!((plan.frame_width,plan.cell_width,plan.id_width), (25,5,3));
    let records = plan.records().collect::<Vec<_>>();
    let names = records.iter().map(|r| r.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names, ["pc","next","faulted","code","result","heap_used","atop","sp","fp","depth","mem","stack","source.r","instance/0/source.x","instance/0/source.r","instance/0/source.fr","instance/0/source.zero","instance/0/source.bad","alloctab","history-0","history-1","history-2"]);
    assert_eq!(records[..12].iter().map(|r|r.width).collect::<Vec<_>>(), [4,4,1,4,2,8,3,5,5,2,512,80]);
    assert_eq!(records[6].initial, BigUint::from(1u8));
    assert_eq!(records[18].width, 264);
    assert_eq!(records[18].initial, BigUint::from(16u8)+(BigUint::from(1u8)<<32)+(BigUint::from(3u8)<<64));
    assert!(records[19..].iter().all(|r|r.width==25 && r.initial==BigUint::default()));
    assert_eq!(plan.bytes.iter().map(|s|(s.base,s.extent,s.storage.clone())).collect::<Vec<_>>(), [(0,16,ByteStorage::Ordinary{offset:0}),(16,1,ByteStorage::Interface{position:0,offset:0}),(17,47,ByteStorage::Ordinary{offset:17})]);
    assert_eq!(plan.locations.iter().map(|l|(l.id,l.width,l.slot)).collect::<Vec<_>>(),[(0,2,0),(1,2,0),(2,2,1),(3,4,2),(4,4,3),(5,1,4)]);
    let memory = records[10].image.as_ref().unwrap().materialize(64).unwrap();
    assert_eq!(memory, vec![0;64]);
    assert!(records[10].image.as_ref().unwrap().materialize(63).is_err());
}
#[test]
fn recursive_example_sizes_capture_and_counter_slots_and_global_image_in_source_order() {
    let object = example(2);
    let plan = Plan::form(&object,"sum_db",[6,4080,512,0]).unwrap();
    assert_eq!(plan.index.halt,19);
    assert_eq!((plan.id_width,plan.cell_width,plan.frame_width),(4,32,106));
    assert_eq!(plan.slot_widths,[32,10,32,10,10,1]);
    let records = plan.records().collect::<Vec<_>>();
    assert_eq!(records[6].initial,BigUint::from(2u8));
    assert_eq!(records[10].width,65536);
    assert_eq!(records[11].width,16384);
    assert_eq!(plan.bytes.iter().map(|s|(s.base,s.extent)).collect::<Vec<_>>(),[(0,4096),(4096,16),(4112,4080)]);
    // Change an image without changing any addresses. The plan reads linked data,
    // not a copied default, and leaves hidden ordinary memory at zero.
    let mut object=object;
    object.data.insert("zero".into(),"a5".into());
    let plan=Plan::form(&object,"sum_db",[0,0,0,0]).unwrap();
    let memory=plan.base[10].image.as_ref().unwrap().materialize(8192).unwrap();
    assert_eq!(memory[16],0xa5);
    assert!(memory.iter().enumerate().all(|(i,b)| i==16 || *b==0));
}
#[test]
fn memoryless_state_has_no_layout_records_and_budget_extremes_do_not_wrap() {
    let object=example(1);
    let plan=Plan::form(&object,"inc8",[3,0,0,0]).unwrap();
    assert_eq!((plan.frame_width,plan.slot_widths.clone()),(13,vec![8]));
    assert_eq!(plan.records().map(|r|r.name).collect::<Vec<_>>(),["pc","next","faulted","code","result","source.one","source.r","history-0","history-1","history-2"]);
    let mut object=example(10);
    let plan=Plan::form(&object,"through",[0,0,0,u64::MAX]).unwrap();
    assert_eq!(plan.base[9].width,65);
    object.metadata.get_mut("layout").unwrap()["H"]=json!(64);
    object.ptr_width=64;
    object.metadata.get_mut("layout").unwrap()["ptr_width"]=json!(64);
    assert_eq!(Plan::form(&object,"through",[0,0,0,0]).unwrap_err().status,bennett_sched::Status::NoMem);
}

#[test]
fn frame_shape_covers_allocation_table_counter_and_wide_store_slots() {
    let mut object=example(2);
    let f=object.functions.get_mut("sum_db").unwrap();
    f.body[0].op="lift.alloc".into();
    f.body[0].operands=vec!["base".into()];
    f.body[1].op="lift.store.i128".into();
    f.body[1].lhs=None;
    f.body[1].operands=vec!["base".into(),"base".into()];
    let plan=Plan::form(&object,"sum_db",[0,0,0,2]).unwrap();
    assert_eq!(plan.slot_widths,[128,66,32,10,10,2]);
    assert_eq!(plan.frame_width,259);
}

#[test]
fn huge_budget_keeps_metadata_lazy_and_materialized_adapter_reports_nomem() {
    let object=example(1);
    let plan=Plan::form(&object,"inc8",[u64::MAX,0,0,0]).unwrap();
    assert_eq!(plan.base.len(),7);
    assert_eq!(plan.records().nth(8).unwrap().name,"history-1");
    let error=bennett_sched::source::Trace::build(&object,"inc8",0,&json!([]),[u64::MAX,0,0,0]).unwrap_err();
    assert_eq!(error.status,bennett_sched::Status::NoMem);
}
