use crate::common;
use bennett_sched::{
    expansion::{Operation, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn trace(body: &str, mutable_input: bool, memory: Option<&str>, adjoints: usize) -> Trace {
    let mem = memory.map_or_else(Vec::new, |role| vec![json!({"A":1,"W":2,"role":role})]);
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action",
        "mut":if mutable_input {vec![2,2]} else {vec![2]},
        "in":if mutable_input {vec![]} else {vec![if memory.is_some(){1}else{2}]},
        "mem":mem,"args":[],"bounds":[]});
    let header = |name: &str| {
        format!(
            "perm_action @{name} ({}) ({}) ({}) () ()",
            if mutable_input {
                "t:wire[2] mut, s:wire[2] mut"
            } else {
                "t:wire[2] mut"
            },
            if mutable_input {
                ""
            } else if memory.is_some() {
                "a:wire in"
            } else {
                "s:wire[2] in"
            },
            memory.map_or(String::new(), |role| format!("r:region[1,2] {role}")),
        )
    };
    let mut bytes = common::action("perm_action", "inner", sig.clone(), &header("inner"), body);
    if memory.is_some() {
        bytes.extend_from_slice(b"\nsection layout\n");
        bytes.extend_from_slice(
            &bennett_sched::canonical::to_vec(&json!({
                "ptr_width":32, "H":2, "A_tab":0, "S_cells":0, "null_size":1,
                "heap_base":5, "heap_limit_max":0, "globals":{}, "interface_regions":{}
            }))
            .unwrap(),
        );
        bytes.push(b'\n');
    }
    let mut callee = "inner".to_owned();
    for depth in 0..adjoints {
        let name = format!("outer{depth}");
        bytes = common::caller(
            &bytes,
            "perm_action",
            &name,
            sig.clone(),
            &header(&name),
            &format!(
                "adj action_call @{callee} ({}) ({}) ({}) ()",
                if mutable_input { "t, s" } else { "t" },
                if mutable_input {
                    ""
                } else if memory.is_some() {
                    "a"
                } else {
                    "s"
                },
                if memory.is_some() { "r" } else { "" }
            ),
        );
        callee = name;
    }
    let object = Object::parse(&bytes).unwrap();
    Trace::build(&object, &callee, 0, &json!([]), [0; 4]).unwrap()
}

fn operations(trace: &Trace) -> Value {
    // Compare the full generated stream while alpha-renaming private lifetimes.
    let mut names = BTreeMap::<String, String>::new();
    let mut wire = |name: &String| {
        if name.starts_with("mut:") || name.starts_with("in:") || name.starts_with("mem:") {
            name.clone()
        } else {
            let next = names.len();
            names
                .entry(name.clone())
                .or_insert_with(|| format!("private:{next}"))
                .clone()
        }
    };
    json!(
        trace
            .events
            .iter()
            .map(|e| match &e.leaf.op {
                Operation::Gate(g) => json!([
                    g.op,
                    g.wires.iter().map(&mut wire).collect::<Vec<_>>(),
                    g.controls.iter().map(&mut wire).collect::<Vec<_>>()
                ]),
                Operation::Alloc {
                    wires,
                    class,
                    state,
                    ..
                } => json!([
                    "alloc",
                    wires.iter().map(&mut wire).collect::<Vec<_>>(),
                    class,
                    state
                ]),
                Operation::Release {
                    wires,
                    class,
                    state,
                    ..
                } => json!([
                    "release",
                    wires.iter().map(&mut wire).collect::<Vec<_>>(),
                    class,
                    state
                ]),
                Operation::Classical { .. } => panic!("unexpected classical action leaf"),
            })
            .collect::<Vec<_>>()
    )
}

#[test]
fn self_adjoint_bundle_instructions_keep_forward_bit_ordinals() {
    for (op, mutable) in [("qxor s t", false), ("qexch s t", true)] {
        let forward = trace(op, mutable, None, 0);
        for adjoints in [1, 2, 3] {
            let adjoint = trace(op, mutable, None, adjoints);
            assert_eq!(
                operations(&adjoint),
                operations(&forward),
                "{op}, depth {adjoints}"
            );
            assert_eq!(
                adjoint
                    .events
                    .iter()
                    .map(|e| e.location.generated)
                    .collect::<Vec<_>>(),
                vec![0, 1]
            );
        }
    }
}

#[test]
fn copy_and_load_replacements_expand_forward_before_source_order_is_reversed() {
    for (body, memory) in [
        ("d = qcopy s\nqxor d t\nquncopy s d", None),
        ("d = qload r a\nqxor d t\nqunload r a d", Some("in")),
    ] {
        let forward = trace(body, false, memory, 0);
        for adjoints in [1, 2] {
            let adjoint = trace(body, false, memory, adjoints);
            assert_eq!(
                operations(&adjoint),
                operations(&forward),
                "{body}, depth {adjoints}"
            );
            let points = adjoint
                .events
                .iter()
                .map(|e| e.location.path.last().unwrap().point.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                points[0],
                if adjoints % 2 == 1 {
                    "entry:2"
                } else {
                    "entry:0"
                }
            );
        }
    }
}

#[test]
fn self_adjoint_memory_dispatch_keeps_forward_address_and_bit_order() {
    for body in ["qstore r a t", "qexchm r a t"] {
        let forward = trace(body, false, Some("mut"), 0);
        for adjoints in [1, 2] {
            let adjoint = trace(body, false, Some("mut"), adjoints);
            assert_eq!(
                operations(&adjoint),
                operations(&forward),
                "{body}, depth {adjoints}"
            );
        }
        // All region values, target values and addresses survive roundtrip.
        let leaves = forward
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let inverse = trace(body, false, Some("mut"), 1)
            .events
            .into_iter()
            .map(|e| e.leaf)
            .collect::<Vec<_>>();
        for tuple in 0..128 {
            let mut state = BTreeMap::new();
            for (bit, w) in forward.slots.iter().flat_map(|s| &s.wires).enumerate() {
                state.insert(w.clone(), tuple & (1 << bit) != 0);
            }
            let before = state.clone();
            simulate_basis(&leaves, &mut state).unwrap();
            simulate_basis(&inverse, &mut state).unwrap();
            assert_eq!(state, before);
        }
    }
}
