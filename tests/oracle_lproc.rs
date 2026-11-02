use bennett_sched::{canonical, expansion::simulate_basis, object::Object, source::Trace};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "common/basis.rs"]
mod basis;

fn object() -> Object {
    let root = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[2],"tgt":2,"fault":4,"mem":[],"args":[]});
    let callee = json!({"abi":"0.9","tier":"q","kind":"lproc","params":[2],"result":2,"tokens":[],"args":[],"mem":[]});
    let echo = "lproc @echo (x:reg[2]) (r:reg[2]) () () ()\n^entry():\nr = lift.mov x\nlift.ret r";
    let outer = "oracle @outer (x:wire[2] in) (t:wire[2]) (f:wire[4]) () ()\nregs { r:reg[2] }\n^entry():\nr = lift.call @echo (x)\nlift.retval r";
    let layout = json!({"ptr_width":3,"H":0,"A_tab":0,"S_cells":3,"null_size":1,"heap_base":1,"heap_limit_max":0,"globals":{},"interface_regions":{}});
    let jcs = |v: &Value| String::from_utf8(canonical::to_vec(v).unwrap()).unwrap();
    let echo_hash =
        canonical::hash(format!("isa=0.9\nptr_width=3\n{}\n{echo}\n", jcs(&callee)).as_bytes());
    let mut leaves = layout
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| !["globals", "interface_regions"].contains(&key.as_str()))
        .map(|(key, value)| (jcs(&json!([key])), jcs(value)))
        .collect::<Vec<_>>();
    leaves.sort();
    let suffix = leaves
        .iter()
        .map(|(path, value)| format!("layout {path} {value}\n"))
        .collect::<String>();
    let linked = outer.replace("@echo", &format!("@{echo_hash}"));
    let root_hash = canonical::hash(
        format!(
            "isa=0.9\nptr_width=3\n{}\n{linked}\n{suffix}{echo_hash}\n",
            jcs(&root)
        )
        .as_bytes(),
    );
    let symbols = json!({"0":{"name":"outer","kind":"oracle","signature":root,"interface_hash":root_hash},"1":{"name":"echo","kind":"lproc","signature":callee,"interface_hash":echo_hash}});
    Object::parse(format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=3\n\nsection symbols\n{}\n\nsection functions\n{outer}\n{echo}\n\nsection layout\n{}\n",jcs(&symbols),jcs(&layout)).as_bytes()).unwrap()
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
fn complete_lproc_call_return_uses_reference_activation_and_restores_every_private_bit() {
    let object = object();
    for budget in 0..=5 {
        let trace = Trace::build(&object, "outer", 1, &json!([]), [budget, 0, 8, 1]).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let program = basis::Program::compile(&leaves).unwrap();
        for input in 0..4 {
            for target in 0..4 {
                for fault in 0..16 {
                    for control in [false, true] {
                        let mut bits = BTreeMap::new();
                        put(&mut bits, "in:0", 2, input);
                        put(&mut bits, "tgt:0", 2, target);
                        put(&mut bits, "fault:0", 4, fault);
                        bits.insert("control:0".into(), control);
                        program.run(&mut bits).unwrap();
                        assert_eq!(get(&bits, "in:0", 2), input);
                        assert_eq!(
                            get(&bits, "tgt:0", 2),
                            target ^ if control && budget >= 4 { input } else { 0 }
                        );
                        assert_eq!(
                            get(&bits, "fault:0", 4),
                            fault ^ u64::from(control && budget < 4)
                        );
                        assert_eq!(bits.len(), 9);
                    }
                }
            }
        }
        if budget > 0 {
            let callee = trace
                .events
                .iter()
                .filter(|e| e.location.path[0].symbol == object.symbols["echo"].interface_hash)
                .collect::<Vec<_>>();
            assert!(!callee.is_empty());
            assert!(callee.iter().all(|e| e.location.path.len() == 1
                && e.location.path[0].instance.is_empty()
                && e.location.path[0].call_index.is_none()));
        }
    }
}

#[test]
fn lproc_depth_or_stack_exhaustion_faults_atomically_and_reverses_cleanly() {
    let object = object();
    for budget in [[1, 0, 8, 0], [4, 0, 4, 1]] {
        let trace = Trace::build(&object, "outer", 0, &json!([]), budget).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let mut bits = BTreeMap::new();
        put(&mut bits, "in:0", 2, 3);
        put(&mut bits, "tgt:0", 2, 2);
        put(&mut bits, "fault:0", 4, 4);
        simulate_basis(&leaves, &mut bits).unwrap();
        assert_eq!(get(&bits, "tgt:0", 2), 2);
        assert_eq!(get(&bits, "fault:0", 4), 4 ^ 3);
        assert_eq!(get(&bits, "in:0", 2), 3);
        assert_eq!(bits.len(), 8);
    }
}
