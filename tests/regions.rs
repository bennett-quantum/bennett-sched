use bennett_sched::{canonical,object::Object,source::Trace,expansion::{Operation,simulate_basis}};
use serde_json::json;
use std::collections::BTreeMap;
mod common;

fn layout(mut bytes:Vec<u8>)->Vec<u8>{bytes.extend_from_slice(b"\nsection layout\n{\"A_tab\":0,\"H\":0,\"S_cells\":0,\"globals\":{},\"heap_base\":1,\"heap_limit_max\":0,\"interface_regions\":{},\"null_size\":1,\"ptr_width\":32}\n");bytes}

fn region_object(address:u64,cell:u64,image:Option<&str>)->Vec<u8>{
    let signature=json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});let image_suffix=if image.is_some(){" @image"}else{""};let function=format!("perm_action @a (t:wire mut) () () () ()\n^entry():\nx t\nqregion r {address} {cell}{image_suffix}\nx t");
    let mut symbols=json!({});let mut rewritten=function.clone();let mut suffix=String::new();let mut data=String::new();
    if let Some(hex)=image{let sig=json!({"abi":"0.9","tier":"-","kind":"data","size":hex.len()/2});let hash=canonical::hash(format!("isa=0.9\nptr_width=32\n{}\nimage {hex}\n",String::from_utf8(canonical::to_vec(&sig).unwrap()).unwrap()).as_bytes());symbols["1"]=json!({"name":"image","kind":"data","signature":sig,"interface_hash":hash});rewritten=rewritten.replace("@image",&format!("@{hash}"));suffix=format!("{hash}\n");data=format!("\nsection data\nimage {hex}\n");}
    let hash=canonical::hash(format!("isa=0.9\nptr_width=32\n{}\n{rewritten}\n{suffix}",String::from_utf8(canonical::to_vec(&signature).unwrap()).unwrap()).as_bytes());symbols["0"]=json!({"name":"a","kind":"perm_action","signature":signature,"interface_hash":hash});
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=32\n\nsection symbols\n{}\n\nsection functions\n{function}\n{data}",String::from_utf8(canonical::to_vec(&symbols).unwrap()).unwrap()).into_bytes()
}

#[test]
fn local_regions_prepare_at_function_entry_restore_at_exit_and_ignore_false_control_for_images(){
    for (address,cell,image) in [(0,3,None),(0,8,Some("81")),(3,1,Some("a5"))]{let object=Object::parse(&region_object(address,cell,image)).unwrap();let trace=Trace::build(&object,"a",1,&json!([]),[0;4]).unwrap();let first=&trace.events[0];assert!(matches!(&first.leaf.op,Operation::Alloc{wires,..}if wires.len()==(1<<address)*cell as usize));assert_eq!(first.location.path.last().unwrap().point,"entry:1");assert!(matches!(trace.events.last().unwrap().leaf.op,Operation::Release{..}));
        let image_gates=trace.events.iter().filter(|e|e.location.path.last().unwrap().point=="entry:1"&&matches!(e.leaf.op,Operation::Gate(_))).collect::<Vec<_>>();assert_eq!(image_gates.len(),image.map_or(0,|s|u8::from_str_radix(s,16).unwrap().count_ones() as usize*2));for event in image_gates{let Operation::Gate(g)=&event.leaf.op else{panic!()};assert!(g.controls.is_empty());}
        let leaves=trace.events.iter().map(|e|e.leaf.clone()).collect::<Vec<_>>();for c in [false,true]{for t in [false,true]{let mut bits=BTreeMap::from([("control:0".into(),c),("mut:0[0]".into(),t)]);simulate_basis(&leaves,&mut bits).unwrap();assert_eq!(bits,BTreeMap::from([("control:0".into(),c),("mut:0[0]".into(),t)]));}}
    }
    let invalid=Object::parse(&region_object(0,3,Some("81"))).unwrap();assert!(Trace::build(&invalid,"a",0,&json!([]),[0;4]).is_err());
}

#[test]
fn interface_region_shape_role_and_cell_identity_are_preserved_across_calls(){
    let signature=json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[{"A":1,"W":3,"role":"in"}],"args":[],"bounds":[]});let base=layout(common::action("perm_action","a",signature.clone(),"perm_action @a (t:wire mut) () (r:region[1,3] in) () ()","x t"));let caller=common::caller(&base,"perm_action","b",signature.clone(),"perm_action @b (t:wire mut) () (s:region[1,3] in) () ()","action_call @a (t) () (s) ()");
    let object=Object::parse(&caller).unwrap();let trace=Trace::build(&object,"b",0,&json!([]),[0;4]).unwrap();assert_eq!(trace.slots[1].id,"mem:0");assert_eq!(trace.slots[1].role,"mem_in");assert_eq!(trace.slots[1].wires.len(),6);assert_eq!(trace.invocations[1].slots[1].wires,trace.slots[1].wires);
    let wrong=layout(common::action("perm_action","a",signature,"perm_action @a (t:wire mut) () (r:region[0,6] in) () ()","x t"));assert!(Object::parse(&wrong).and_then(|o|Trace::build(&o,"a",0,&json!([]),[0;4])).is_err());
    let signature=json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[{"A":0,"W":1,"role":"mut"}],"args":[],"bounds":[]});let wrong=layout(common::action("perm_action","a",signature,"perm_action @a (t:wire mut) () (r:region[0,1] mut) () ()","x r"));assert!(Trace::build(&Object::parse(&wrong).unwrap(),"a",0,&json!([]),[0;4]).is_err());
}
