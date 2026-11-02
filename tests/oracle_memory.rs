use bennett_sched::{expansion::simulate_basis, object::Object, source::Trace};
use serde_json::json;
use std::collections::BTreeMap;
fn mapped_reader() -> Object {
    let signature = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[8],"tgt":8,"fault":4,"mem":[{"A":0,"W":8,"role":"in"}],"args":[]});
    let layout = json!({"ptr_width":3,"H":2,"A_tab":0,"S_cells":0,"null_size":1,"heap_base":3,"heap_limit_max":0,"globals":{},"interface_regions":{"read":{"db":{"base":2}}}});
    let function = "oracle @read (x:wire[8] in) (t:wire[8]) (f:wire[4]) (db:region[0,8] in) ()\nregs { p:reg[3], r:reg[8] }\n^entry():\np = lift.addr @db\nr = lift.load.i8 p\nlift.retval r";
    // Independently form the §9.2 oracle envelope, with exact key-component
    // layout leaves. This fixture passes the ordinary encoded-object loader.
    let mut leaves = layout
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| !["globals", "interface_regions"].contains(&key.as_str()))
        .map(|(key, value)| (json!([key]), value.clone()))
        .collect::<Vec<_>>();
    leaves.push((
        json!(["interface_regions", "read", "db"]),
        json!({"base":2}),
    ));
    let jcs = |v: &serde_json::Value| {
        String::from_utf8(bennett_sched::canonical::to_vec(v).unwrap()).unwrap()
    };
    leaves.sort_by_key(|(path, _)| jcs(path));
    let suffix = leaves
        .iter()
        .map(|(path, value)| format!("layout {} {}\n", jcs(path), jcs(value)))
        .collect::<String>();
    let hash = bennett_sched::canonical::hash(
        format!(
            "isa=0.9\nptr_width=3\n{}\n{function}\n{suffix}",
            jcs(&signature)
        )
        .as_bytes(),
    );
    let symbols =
        json!({"0":{"name":"read","kind":"oracle","signature":signature,"interface_hash":hash}});
    let bytes = format!(
        "bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=3\n\nsection symbols\n{}\n\nsection functions\n{function}\n\nsection layout\n{}\n",
        jcs(&symbols),
        jcs(&layout)
    );
    Object::parse(bytes.as_bytes()).unwrap()
}
fn put(bits: &mut BTreeMap<String, bool>, name: &str, width: u64, value: u64) {
    for i in 0..width {
        bits.insert(format!("{name}[{i}]"), value & (1 << i) != 0);
    }
}
fn get(bits: &BTreeMap<String, bool>, name: &str, width: u64) -> u64 {
    (0..width)
        .map(|i| u64::from(bits[&format!("{name}[{i}]")]) << i)
        .sum()
}
#[test]
fn mapped_byte_reads_use_input_cells_and_restore_the_entire_private_layout_state() {
    let object = mapped_reader();
    let trace = Trace::build(&object, "read", 1, &json!([]), [3, 0, 0, 0]).unwrap();
    assert_eq!(trace.slots.last().unwrap().id, "mem:0");
    assert_eq!(trace.slots.last().unwrap().role, "mem_in");
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for byte in 0..256 {
        let mut bits = BTreeMap::new();
        put(&mut bits, "in:0", 8, 13);
        put(&mut bits, "tgt:0", 8, 0xa5);
        put(&mut bits, "fault:0", 4, 5);
        put(&mut bits, "mem:0", 8, byte);
        bits.insert("control:0".into(), true);
        simulate_basis(&leaves, &mut bits).unwrap();
        assert_eq!(get(&bits, "tgt:0", 8), 0xa5 ^ byte);
        assert_eq!(get(&bits, "fault:0", 4), 5);
        assert_eq!(get(&bits, "in:0", 8), 13);
        assert_eq!(get(&bits, "mem:0", 8), byte);
        assert_eq!(bits.len(), 29);
    }
}
#[test]
fn inherent_memory_bounds_fault_and_budget_control_paths_restore_state() {
    let mut object = mapped_reader();
    let f = object.functions.get_mut("read").unwrap();
    f.body[0].op = "lift.const.i3".into();
    f.body[0].operands = vec!["3".into()];
    for budget in [0, 1, 2, 3] {
        let trace = Trace::build(&object, "read", 1, &json!([]), [budget, 0, 0, 0]).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for control in [false, true] {
            let mut bits = BTreeMap::new();
            put(&mut bits, "in:0", 8, 31);
            put(&mut bits, "tgt:0", 8, 0x53);
            put(&mut bits, "fault:0", 4, 9);
            put(&mut bits, "mem:0", 8, 0xf0);
            bits.insert("control:0".into(), control);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(get(&bits, "tgt:0", 8), 0x53);
            assert_eq!(
                get(&bits, "fault:0", 4),
                9 ^ if !control {
                    0
                } else if budget < 2 {
                    1
                } else {
                    7
                }
            );
            assert_eq!(get(&bits, "mem:0", 8), 0xf0);
            assert_eq!(bits.len(), 29);
        }
    }
}
