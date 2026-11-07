use crate::common;
use bennett_sched::{
    expansion::{Operation, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn adjoint_calls_preserve_caller_loop_coordinates_and_remap_only_the_callee() {
    let base = common::action(
        "perm_action",
        "flip",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[2]}),
        "perm_action @flip (t:wire mut) () () () ()",
        "qloop 2 {\nx t\n}",
    );
    for prefix in ["", "adj "] {
        let caller = common::caller(
            &base,
            "perm_action",
            "outer",
            json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[3]}),
            "perm_action @outer (t:wire mut) () () () ()",
            &format!("qloop 3 {{\n{prefix}action_call @flip (t) () () ()\n}}"),
        );
        let object = Object::parse(&caller).unwrap();
        let trace = Trace::build(&object, "outer", 0, &json!([]), [0; 4]).unwrap();
        let actual = trace
            .events
            .iter()
            .map(|e| {
                (
                    e.location.path[0].iteration.clone(),
                    e.location.path[1].iteration.clone(),
                )
            })
            .collect::<Vec<_>>();
        // Source traversal addresses each caller occurrence before entering
        // the callee's local ascending iteration space (§5.3).
        let expected = vec![
            (vec![0], vec![0]),
            (vec![0], vec![1]),
            (vec![1], vec![0]),
            (vec![1], vec![1]),
            (vec![2], vec![0]),
            (vec![2], vec![1]),
        ];
        assert_eq!(actual, expected, "{prefix}action_call");
    }
}

#[test]
fn nested_adjoint_calls_leave_every_outside_loop_coordinate_unchanged() {
    let base = common::action(
        "perm_action",
        "flip",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[2]}),
        "perm_action @flip (t:wire mut) () () () ()",
        "qloop 2 {\nx t\n}",
    );
    let middle = common::caller(
        &base,
        "perm_action",
        "middle",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[3]}),
        "perm_action @middle (t:wire mut) () () () ()",
        "qloop 3 {\nadj action_call @flip (t) () () ()\n}",
    );
    let outer = common::caller(
        &middle,
        "perm_action",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[2]}),
        "perm_action @outer (t:wire mut) () () () ()",
        "qloop 2 {\nadj action_call @middle (t) () () ()\n}",
    );
    let trace = Trace::build(
        &Object::parse(&outer).unwrap(),
        "outer",
        0,
        &json!([]),
        [0; 4],
    )
    .unwrap();
    let actual = trace
        .events
        .iter()
        .map(|e| {
            e.location
                .path
                .iter()
                .map(|p| p.iteration.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let expected = vec![
        vec![vec![0], vec![0], vec![0]],
        vec![vec![0], vec![0], vec![1]],
        vec![vec![0], vec![1], vec![0]],
        vec![vec![0], vec![1], vec![1]],
        vec![vec![0], vec![2], vec![0]],
        vec![vec![0], vec![2], vec![1]],
        vec![vec![1], vec![0], vec![0]],
        vec![vec![1], vec![0], vec![1]],
        vec![vec![1], vec![1], vec![0]],
        vec![vec![1], vec![1], vec![1]],
        vec![vec![1], vec![2], vec![0]],
        vec![vec![1], vec![2], vec![1]],
    ];
    assert_eq!(actual, expected);
    for invocation in trace.invocations.iter().skip(1) {
        let expected = invocation.path.iter().map(|p| p.json()).collect::<Vec<_>>();
        for event in &trace.events[invocation.start..invocation.end] {
            let prefix = event
                .location
                .path
                .iter()
                .take(invocation.path.len())
                .map(|p| p.json())
                .collect::<Vec<_>>();
            assert_eq!(
                prefix, expected,
                "descendant invocation path must follow its remapped events"
            );
        }
    }
}

#[test]
fn while_flags_pad_every_iteration_and_restore_scratch_for_every_condition() {
    for n in 1..=3 {
        let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[n],"mem":[],"args":[],"bounds":[n]});
        let bytes = common::action(
            "perm_action",
            "a",
            signature,
            &format!("perm_action @a (t:wire mut) (cond:wire[{n}] in) () () ()"),
            &format!("qloop {n} while cond {{\nx t\n}}"),
        );
        let object = Object::parse(&bytes).unwrap();
        for inherited in 0..=1 {
            let trace = Trace::build(&object, "a", inherited, &json!([]), [0; 4]).unwrap();
            let body = trace
                .events
                .iter()
                .filter(|e| e.location.path.last().unwrap().point == "entry:1")
                .collect::<Vec<_>>();
            assert_eq!(body.len(), n as usize);
            assert_eq!(
                body.iter()
                    .map(|e| e.location.path.last().unwrap().iteration.clone())
                    .collect::<Vec<_>>(),
                (0..n).map(|i| vec![i]).collect::<Vec<_>>()
            );
            let support = trace
                .events
                .iter()
                .filter(|e| e.location.path.last().unwrap().point == "entry:0")
                .collect::<Vec<_>>();
            assert_eq!(support.len(), 2 * n as usize + 2);
            assert_eq!(
                support
                    .iter()
                    .map(|e| e.location.generated)
                    .collect::<Vec<_>>(),
                (0..2 * n + 2).collect::<Vec<_>>()
            );
            assert!(
                matches!(&support[0].leaf.op,Operation::Alloc{wires,..}if wires.len()==n as usize)
            );
            let leaves = trace
                .events
                .iter()
                .map(|e| e.leaf.clone())
                .collect::<Vec<_>>();
            for control in 0..=u8::from(inherited != 0) {
                for condition in 0..1u64 << n {
                    for t in [false, true] {
                        let mut bits = BTreeMap::from([("mut:0[0]".into(), t)]);
                        for i in 0..n {
                            bits.insert(format!("in:0[{i}]"), condition & (1 << i) != 0);
                        }
                        if inherited != 0 {
                            bits.insert("control:0".into(), control != 0);
                        }
                        let mut parity = false;
                        for i in 0..n {
                            parity ^= condition & ((1 << (i + 1)) - 1) == (1 << (i + 1)) - 1;
                        }
                        simulate_basis(&leaves, &mut bits).unwrap();
                        assert_eq!(
                            bits["mut:0[0]"],
                            t ^ (parity && (inherited == 0 || control != 0))
                        );
                        assert_eq!(bits.len(), 1 + n as usize + inherited as usize);
                    }
                }
            }
        }
    }
}

#[test]
fn while_rejects_writes_to_its_conditions_and_loop_live_names_must_agree() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,2],"in":[],"mem":[],"args":[],"bounds":[2]});
    let bytes = common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut, cond:wire[2] mut) () () () ()",
        "qloop 2 while cond {\nx cond[1]\n}",
    );
    assert!(Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4]).is_err());
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[1]});
    let bytes = common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut) () () () ()",
        "qloop 1 {\nd = qalloc any 1\n}\nqfree d",
    );
    assert!(Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4]).is_err());
}

#[test]
fn action_adjoint_computes_all_while_flags_before_reverse_bodies_and_double_adjoint_is_literal_forward()
 {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[3],"mem":[],"args":[],"bounds":[3]});
    let base = common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut) (cond:wire[3] in) () () ()",
        "qloop 3 while cond {\nx t\n}",
    );
    let caller_signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[3],"mem":[],"args":[],"bounds":[]});
    let inverse = common::caller(
        &base,
        "perm_action",
        "b",
        caller_signature.clone(),
        "perm_action @b (t:wire mut) (cond:wire[3] in) () () ()",
        "adj action_call @a (t) (cond) () ()",
    );
    let object = Object::parse(&inverse).unwrap();
    let trace = Trace::build(&object, "b", 0, &json!([]), [0; 4]).unwrap();
    let body = trace
        .events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.location.path.last().unwrap().point == "entry:1")
        .collect::<Vec<_>>();
    assert_eq!(
        body.iter()
            .map(|(_, e)| e.location.path.last().unwrap().iteration.clone())
            .collect::<Vec<_>>(),
        vec![vec![0], vec![1], vec![2]]
    );
    assert_eq!(body[0].0, 4);
    assert_eq!(body[2].0, 6);
    assert_eq!(trace.events.len(), 11);
    let double = common::caller(
        &inverse,
        "perm_action",
        "c",
        caller_signature,
        "perm_action @c (t:wire mut) (cond:wire[3] in) () () ()",
        "adj action_call @b (t) (cond) () ()",
    );
    let double =
        Trace::build(&Object::parse(&double).unwrap(), "c", 0, &json!([]), [0; 4]).unwrap();
    let forward = Trace::build(&Object::parse(&base).unwrap(), "a", 0, &json!([]), [0; 4]).unwrap();
    fn shape(trace: &Trace) -> Vec<(String, Vec<u64>, u64, String, Vec<String>, Vec<String>)> {
        let flags = trace
            .events
            .iter()
            .find_map(|e| {
                if let Operation::Alloc { wires, .. } = &e.leaf.op {
                    Some(wires.clone())
                } else {
                    None
                }
            })
            .unwrap();
        let remap = |w: &String| {
            flags
                .iter()
                .position(|f| f == w)
                .map(|i| format!("flag[{i}]"))
                .unwrap_or_else(|| w.clone())
        };
        trace
            .events
            .iter()
            .map(|e| {
                let p = e.location.path.last().unwrap();
                let (op, w, c) = match &e.leaf.op {
                    Operation::Gate(g) => (
                        g.op.clone(),
                        g.wires.iter().map(&remap).collect(),
                        g.controls.iter().map(&remap).collect(),
                    ),
                    Operation::Alloc { wires, .. } => {
                        ("alloc".into(), wires.iter().map(&remap).collect(), vec![])
                    }
                    Operation::Release { wires, .. } => {
                        ("release".into(), wires.iter().map(&remap).collect(), vec![])
                    }
                    _ => panic!(),
                };
                (
                    p.point.clone(),
                    p.iteration.clone(),
                    e.location.generated,
                    op,
                    w,
                    c,
                )
            })
            .collect()
    }
    assert_eq!(shape(&double), shape(&forward));
}
