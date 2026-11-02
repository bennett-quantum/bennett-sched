use bennett_sched::{expansion::simulate_basis, object::Object, source::Trace};
use serde_json::json;
use std::collections::BTreeMap;
#[path = "common/basis.rs"]
mod basis;
fn mapped_reader() -> Object {
    reader(false)
}
fn reader(check: bool) -> Object {
    Object::parse(&encoded_reader(check)).unwrap()
}
fn encoded_reader(check: bool) -> Vec<u8> {
    let signature = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[8],"tgt":8,"fault":4,"mem":[{"A":0,"W":8,"role":"in"}],"args":[]});
    let layout = json!({"ptr_width":3,"H":2,"A_tab":0,"S_cells":0,"null_size":1,"heap_base":3,"heap_limit_max":0,"globals":{},"interface_regions":{"read":{"db":{"base":2}}}});
    let check = if check { "lift.boundscheck p 1\n" } else { "" };
    let function = format!(
        "oracle @read (x:wire[8] in) (t:wire[8]) (f:wire[4]) (db:region[0,8] in) ()\nregs {{ p:reg[3], r:reg[8] }}\n^entry():\np = lift.addr @db\n{check}r = lift.load.i8 p\nlift.retval r"
    );
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
    bytes.into_bytes()
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

#[test]
fn explicit_boundscheck_cleans_its_literal_and_faults_before_access_when_length_crosses_entry() {
    for crossing in [false, true] {
        let mut object = reader(true);
        if crossing {
            object.functions.get_mut("read").unwrap().body[1].operands[1] = "2".into();
        }
        let trace = Trace::build(&object, "read", 0, &json!([]), [4, 0, 0, 0]).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for byte in [0, 1, 127, 255] {
            let mut bits = BTreeMap::new();
            put(&mut bits, "in:0", 8, 13);
            put(&mut bits, "tgt:0", 8, 0xa5);
            put(&mut bits, "fault:0", 4, 5);
            put(&mut bits, "mem:0", 8, byte);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(
                get(&bits, "tgt:0", 8),
                0xa5 ^ if crossing { 0 } else { byte }
            );
            assert_eq!(get(&bits, "fault:0", 4), 5 ^ if crossing { 7 } else { 0 });
            assert_eq!(get(&bits, "mem:0", 8), byte);
            assert_eq!(bits.len(), 28);
        }
    }
}

#[test]
fn oversized_interface_is_a_nomem_error_before_creating_any_wire_strings() {
    let mut object = mapped_reader();
    object.functions.get_mut("read").unwrap().groups[0][0].ty = format!("wire[{}]", u64::MAX);
    let error = Trace::build(&object, "read", 0, &json!([]), [0, 0, 0, 0]).unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::NoMem);
}

#[test]
fn ordinary_global_image_is_initialized_and_reversed_without_touching_mapped_input() {
    let mut object = mapped_reader();
    let mut symbol = object.symbols["read"].clone();
    symbol.name = "image".into();
    symbol.kind = "data".into();
    symbol.signature = json!({"abi":"0.9","kind":"data","tier":"-","size":1});
    object.symbols.insert("image".into(), symbol);
    object.data.insert("image".into(), "a5".into());
    let layout = object.metadata.get_mut("layout").unwrap();
    layout["A_tab"] = json!(1);
    layout["globals"] = json!({"image":{"base":1,"size":1,"image":"image"}});
    object.functions.get_mut("read").unwrap().body[0].operands[0] = "@image".into();
    let trace = Trace::build(&object, "read", 1, &json!([]), [3, 0, 0, 0]).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for control in [false, true] {
        for byte in [0, 0xff] {
            let mut bits = BTreeMap::new();
            put(&mut bits, "in:0", 8, 0x13);
            put(&mut bits, "tgt:0", 8, 0x53);
            put(&mut bits, "fault:0", 4, 9);
            put(&mut bits, "mem:0", 8, byte);
            bits.insert("control:0".into(), control);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(
                get(&bits, "tgt:0", 8),
                0x53 ^ if control { 0xa5 } else { 0 }
            );
            assert_eq!(get(&bits, "fault:0", 4), 9);
            assert_eq!(get(&bits, "mem:0", 8), byte);
            assert_eq!(bits.len(), 29);
        }
    }
}

#[test]
fn encoded_layout_request_emits_an_admitted_schedule_and_bound_frame_certificate() {
    use bennett_sched::{Outcome, Request, canonical, schedule::Document};
    let bytes = encoded_reader(false);
    let object = Object::parse(&bytes).unwrap();
    let gates=["x","cx","ccx","mcx","swap"].iter().map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}})).collect::<Vec<_>>();
    let target = json!({"ptr_widths_supported":[3],"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"classes":{"fast":{"capacity":"unbounded","qubit_second":1,"dirty_rate":1}},"gates":gates,"schemes":[],"qram":null,"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":0},"objective":{"weighted":{"time":1}},"tables":{},"imports":{}});
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(&canonical::to_vec(&target).unwrap())
        .unwrap();
    request
        .set_region(object.closure_hash("read").unwrap(), "forward", 0)
        .unwrap();
    request.budget = Some([3, 0, 0, 0]);
    request.set_params(b"[]").unwrap();
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let schedule = Document::parse(canonical::require_jcs(&response.schedule).unwrap()).unwrap();
    schedule.validate_request(&request).unwrap();
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(certificate["bindings"]["object_hash"], object.object_hash);
    let region = &certificate["regions"][object.closure_hash("read").unwrap()];
    let claim = region["verification"]["oracle"].as_u64().unwrap() as usize;
    assert_eq!(
        region["claims"][claim]["domain"]["regions"],
        json!([{"name":"db","cells":"all"}])
    );
    assert!(!response.schedule.is_empty() && !response.certificate.is_empty());
}

#[test]
fn store_then_load_covers_every_input_old_byte_pair_and_restores_mapped_memory() {
    let mut object = mapped_reader();
    let f = object.functions.get_mut("read").unwrap();
    let mut store = f.body[1].clone();
    store.point = "entry:1".into();
    store.lhs = None;
    store.op = "lift.store.i8".into();
    store.operands = vec!["p".into(), "x".into()];
    f.body.insert(1, store);
    for (i, node) in f.body.iter_mut().enumerate() {
        node.point = format!("entry:{i}");
    }
    let trace = Trace::build(&object, "read", 1, &json!([]), [4, 0, 0, 0]).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let program = basis::Program::compile(&leaves).unwrap();
    // Exhaust the two byte inputs and both controls. Target/fault values vary
    // across this witness; their independent XOR behavior is checked below.
    for input in 0..256 {
        let mut cases = Vec::new();
        let mut expected = Vec::new();
        for old in 0..256 {
            for control in [false, true] {
                let target = input ^ old ^ 0xa5;
                let fault = old & 15;
                let mut bits = BTreeMap::new();
                put(&mut bits, "in:0", 8, input);
                put(&mut bits, "tgt:0", 8, target);
                put(&mut bits, "fault:0", 4, fault);
                put(&mut bits, "mem:0", 8, old);
                bits.insert("control:0".into(), control);
                cases.push(bits);
                expected.push((old, target, fault, control));
            }
        }
        program.run_many(&mut cases).unwrap();
        for (bits, (old, target, fault, control)) in cases.iter().zip(expected) {
            assert_eq!(get(bits, "in:0", 8), input);
            assert_eq!(get(bits, "mem:0", 8), old);
            assert_eq!(
                get(bits, "tgt:0", 8),
                target ^ if control { input } else { 0 }
            );
            assert_eq!(get(bits, "fault:0", 4), fault);
            assert_eq!(bits.len(), 29);
        }
    }
}

#[test]
fn store_bounds_faults_and_step_cuts_preserve_arbitrary_target_fault_and_memory() {
    for bad_address in [false, true] {
        let mut object = mapped_reader();
        let f = object.functions.get_mut("read").unwrap();
        f.body[1].lhs = None;
        f.body[1].op = "lift.store.i8".into();
        f.body[1].operands = vec!["p".into(), "x".into()];
        f.body[2].operands = vec!["x".into()];
        if bad_address {
            f.body[0].op = "lift.const.i3".into();
            f.body[0].operands = vec!["3".into()];
        }
        for b in 0..=3 {
            let trace = Trace::build(&object, "read", 1, &json!([]), [b, 0, 0, 0]).unwrap();
            let leaves = trace
                .events
                .iter()
                .map(|e| e.leaf.clone())
                .collect::<Vec<_>>();
            let program = basis::Program::compile(&leaves).unwrap();
            let mut cases = Vec::new();
            let mut expected = Vec::new();
            for target in 0..256 {
                for fault in 0..16 {
                    for control in [false, true] {
                        let mut bits = BTreeMap::new();
                        put(&mut bits, "in:0", 8, 0xa5);
                        put(&mut bits, "tgt:0", 8, target);
                        put(&mut bits, "fault:0", 4, fault);
                        put(&mut bits, "mem:0", 8, 0x53);
                        bits.insert("control:0".into(), control);
                        cases.push(bits);
                        expected.push((target, fault, control));
                    }
                }
            }
            program.run_many(&mut cases).unwrap();
            for (bits, (target, fault, control)) in cases.iter().zip(expected) {
                let cause = if !control {
                    0
                } else if bad_address && b >= 2 {
                    7
                } else if b < 3 {
                    1
                } else {
                    0
                };
                assert_eq!(get(bits, "fault:0", 4), fault ^ cause);
                assert_eq!(
                    get(bits, "tgt:0", 8),
                    target
                        ^ if control && !bad_address && b == 3 {
                            0xa5
                        } else {
                            0
                        }
                );
                assert_eq!(get(bits, "in:0", 8), 0xa5);
                assert_eq!(get(bits, "mem:0", 8), 0x53);
                assert_eq!(bits.len(), 29);
            }
        }
    }
}
