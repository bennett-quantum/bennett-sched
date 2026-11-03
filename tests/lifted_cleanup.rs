use bennett_sched::{
    object::Object,
    schedule::{Kind, parse_entry},
    source::Trace,
};
use serde_json::{Value, json};
#[path = "common/basis.rs"]
mod basis;
mod common;

fn bytes() -> Vec<u8> {
    let base = common::action(
        "oracle",
        "o",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    common::caller(
        &base,
        "oracle",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]}),
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b:token)\nregs { r:reg[1], f:reg[4], s:reg[1] }",
        "lift.oracle_call @o b (x) r f () ()\ns = lift.mov r\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval s",
    )
}
fn object() -> Object {
    Object::parse(&bytes()).unwrap()
}
fn entry(
    carrier: &bennett_sched::schedule::Location,
    step: Value,
    phase: Value,
) -> bennett_sched::schedule::Entry {
    parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":step,"phase":phase,"generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap()
}
#[test]
fn lifted_uncall_carriers_are_non_emitting_and_ties_resolve_before_mutation() {
    let object = object();
    for budget in 0..=4 {
        let mut trace = Trace::build(
            &object,
            "outer",
            1,
            &json!([["0", "0", "0", "0"]]),
            [budget, 0, 0, 0],
        )
        .unwrap();
        assert_eq!(trace.cleanups.len(), 2 * budget as usize);
        for pair in &trace.cleanups {
            let l = &pair.carrier;
            assert_eq!(pair.owner, 0);
            assert_eq!(l.kind, Kind::Cleanup);
            assert_eq!(l.generated, 0);
            assert_eq!(l.path.len(), 1);
            assert_eq!(l.path[0].point, "entry:2");
            assert_eq!(l.path[0].call_index, None);
            assert_eq!(l.path[0].entered_oracle, None);
            assert!((1..=budget).contains(&l.root.step.unwrap()));
            assert!(l.root.phase.is_some());
        }
        if budget > 0 {
            let e = entry(&trace.cleanups[0].carrier, json!("*"), json!("*"));
            let before = trace
                .events
                .iter()
                .map(|e| format!("{:?}", e))
                .collect::<Vec<_>>();
            assert!(
                trace
                    .realise_cleanup_entries(&[e.clone(), e])
                    .unwrap_err()
                    .reason
                    .contains("multiple maximal")
            );
            assert_eq!(
                before,
                trace
                    .events
                    .iter()
                    .map(|e| format!("{:?}", e))
                    .collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn lifted_recompute_matches_reverse_for_complete_external_domains_and_keeps_step_scopes_separate() {
    use std::collections::{BTreeMap, BTreeSet};
    let object = object();
    for (inner, all) in [(0, false), (1, false), (0, true), (1, true)] {
        let mut trace = Trace::build(
            &object,
            "outer",
            1,
            &json!([[inner.to_string(), "0", "0", "0"]]),
            [4, 0, 0, 0],
        )
        .unwrap();
        assert_eq!(trace.cleanups.len(), 8);
        let e = if all {
            entry(&trace.cleanups[0].carrier, json!("*"), json!("*"))
        } else {
            entry(&trace.cleanups[0].carrier, json!(3), json!("fwd"))
        };
        let replaced = if all { 8 } else { 1 };
        trace.realise_cleanup_entries(&[e]).unwrap();
        assert_eq!(
            trace
                .cleanups
                .iter()
                .filter(|p| p.realisation == "recompute")
                .count(),
            replaced
        );
        let calls = trace
            .invocations
            .iter()
            .filter(|i| i.name == "o" && i.path[0].point == "entry:2")
            .collect::<Vec<_>>();
        assert_eq!(
            calls.len(),
            8 + replaced,
            "one of eight uncall contexts is replaced by two fresh calls"
        );
        let fresh = calls
            .iter()
            .filter(|i| i.path[0].call_index == Some(1))
            .collect::<Vec<_>>();
        assert_eq!(fresh.len(), replaced);
        let mut addresses = BTreeSet::new();
        for event in &trace.events {
            assert!(
                addresses.insert(format!("{:?}", event.location)),
                "duplicate native source address: {:?}",
                event.location
            );
        }
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let native = basis::Program::compile(&leaves).unwrap();
        let mut cases = (0..128)
            .map(|bits| {
                let mut state = BTreeMap::new();
                for (name, n, offset) in [("in:0", 1, 0), ("tgt:0", 1, 1), ("fault:0", 4, 2)] {
                    for bit in 0..n {
                        state.insert(format!("{name}[{bit}]"), bits & (1 << (offset + bit)) != 0);
                    }
                }
                state.insert("control:0".into(), bits & 64 != 0);
                state
            })
            .collect::<Vec<_>>();
        native.run_many(&mut cases).unwrap();
        for (bits, state) in cases.iter().enumerate() {
            assert_eq!(state["in:0[0]"], bits & 1 != 0);
            assert_eq!(
                state["tgt:0[0]"],
                (bits & 2 != 0) ^ (bits & 64 != 0 && inner > 0 && bits & 1 != 0)
            );
            for bit in 0..4 {
                assert_eq!(
                    state[&format!("fault:0[{bit}]")],
                    bits & (1 << (2 + bit)) != 0
                );
            }
            assert_eq!(state.len(), 7);
        }
    }
}

#[test]
fn action_oracle_calls_preserve_nested_lifted_carriers_and_per_step_recompute() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,4],"in":[1],"mem":[],"args":["token","token"],"bounds":[]});
    let source = common::caller(
        &bytes(),
        "perm_action",
        "entry",
        sig,
        "perm_action @entry (t:wire mut, f:wire[4] mut) (x:wire in) () (a:token, b:token) ()",
        "oracle_call @outer a (x) t f () (b)",
    );
    let object = Object::parse(&source).unwrap();
    let mut trace = Trace::build(
        &object,
        "entry",
        1,
        &json!([["4", "0", "0", "0"], ["1", "0", "0", "0"]]),
        [0; 4],
    )
    .unwrap();
    assert_eq!(trace.cleanups.len(), 8);
    let selected = entry(&trace.cleanups[0].carrier, json!(3), json!("fwd"));
    trace.realise_cleanup_entries(&[selected]).unwrap();
    assert_eq!(
        trace
            .cleanups
            .iter()
            .filter(|p| p.realisation == "recompute")
            .count(),
        1
    );
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let native = basis::Program::compile(&leaves).unwrap();
    let mut cases = (0..128)
        .map(|bits| {
            std::collections::BTreeMap::from_iter(
                (0..4)
                    .map(|i| (format!("mut:1[{i}]"), bits & (1 << (2 + i)) != 0))
                    .chain([
                        ("mut:0[0]".into(), bits & 2 != 0),
                        ("in:0[0]".into(), bits & 1 != 0),
                        ("control:0".into(), bits & 64 != 0),
                    ]),
            )
        })
        .collect::<Vec<_>>();
    native.run_many(&mut cases).unwrap();
    for (bits, state) in cases.iter().enumerate() {
        assert_eq!(
            state["mut:0[0]"],
            (bits & 2 != 0) ^ (bits & 1 != 0 && bits & 64 != 0)
        );
        assert_eq!(state["in:0[0]"], bits & 1 != 0);
        for i in 0..4 {
            assert_eq!(state[&format!("mut:1[{i}]")], bits & (1 << (2 + i)) != 0);
        }
        assert_eq!(state.len(), 7);
    }
}
