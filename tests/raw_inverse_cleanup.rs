use bennett_sched::{object::Object, schedule::parse_entry, source::Trace};
use serde_json::json;
use std::collections::BTreeMap;
#[path = "common/basis.rs"]
mod basis;
use crate::common;

#[test]
fn nested_recompute_is_selected_in_forward_space_before_raw_inverse_traversal() {
    let base = common::action(
        "perm_action",
        "mix",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @mix (t:wire mut) (x:wire in) () () ()",
        "d = qcopy x\ncx d t\nquncopy x d",
    );
    let bytes = common::caller(
        &base,
        "oracle",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () ()\nregs { r:reg[1] }",
        "lift.perm_call @mix (r) (x) () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    for phase in ["fwd", "rev", "*"] {
        let mut trace =
            common::trace_distinct_tokens(&object, "outer", 1, &json!([]), [2, 0, 0, 0]).unwrap();
        assert_eq!(trace.cleanups.len(), 4);
        let carrier = &trace.cleanups[0].carrier;
        let entry = parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":phase,"generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
        trace
            .realise_cleanup_entries(&[entry])
            .unwrap_or_else(|error| panic!("{phase}: {error}"));
        assert_eq!(
            trace
                .cleanups
                .iter()
                .filter(|p| p.realisation == "recompute")
                .count(),
            if phase == "*" { 4 } else { 2 }
        );
        check_basis(&trace);
    }
}

fn check_basis(trace: &Trace) {
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let native = basis::Program::compile(&leaves).unwrap();
    let mut cases = (0..128)
        .map(|tuple| {
            let mut state = BTreeMap::from([
                ("in:0[0]".into(), tuple & 1 != 0),
                ("tgt:0[0]".into(), tuple & 2 != 0),
                ("control:0".into(), tuple & 64 != 0),
            ]);
            for bit in 0..4 {
                state.insert(format!("fault:0[{bit}]"), tuple & (1 << (bit + 2)) != 0);
            }
            state
        })
        .collect::<Vec<_>>();
    let before = cases.clone();
    native.run_many(&mut cases).unwrap();
    for (state, initial) in cases.iter().zip(&before) {
        assert_eq!(state["in:0[0]"], initial["in:0[0]"]);
        assert_eq!(
            state["tgt:0[0]"],
            initial["tgt:0[0]"] ^ (initial["in:0[0]"] && initial["control:0"])
        );
        for bit in 0..4 {
            let name = format!("fault:0[{bit}]");
            assert_eq!(state[&name], initial[&name]);
        }
        assert_eq!(state.len(), 7);
    }
}

#[test]
fn nested_oracle_phase_selection_survives_forward_normalisation() {
    let mix = common::action(
        "perm_action",
        "mix",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @mix (t:wire mut) (x:wire in) () () ()",
        "d = qcopy x\ncx d t\nquncopy x d",
    );
    let middle = common::caller(
        &mix,
        "oracle",
        "middle",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @middle (x:wire in) (t:wire) (fr:wire[4]) () ()\nregs { r:reg[1] }",
        "lift.perm_call @mix (r) (x) () ()\nlift.retval r",
    );
    let bytes = common::caller(
        &middle,
        "oracle",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]}),
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b:token)\nregs { r:reg[1], f:reg[4] }",
        "lift.oracle_call @middle b (x) r f () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    for phase in ["rev", "*"] {
        let mut trace = common::trace_distinct_tokens(
            &object,
            "outer",
            1,
            &json!([["2", "0", "0", "0"]]),
            [2, 0, 0, 0],
        )
        .unwrap();
        assert_eq!(trace.cleanups.len(), 16);
        let carrier = &trace.cleanups[0].carrier;
        let e = parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":phase,"generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
        trace.realise_cleanup_entries(&[e.clone()]).unwrap();
        assert_eq!(
            trace
                .cleanups
                .iter()
                .filter(|p| p.realisation == "recompute")
                .count(),
            if phase == "*" { 16 } else { 8 }
        );
        for pair in &trace.cleanups {
            assert_eq!(pair.realisation == "recompute", e.matches(&pair.carrier));
        }
        let mut addresses = std::collections::BTreeSet::new();
        for event in &trace.events {
            assert!(
                addresses.insert(format!("{:?}", event.location)),
                "duplicate: {:?}",
                event.location
            );
        }
        check_basis(&trace);
    }
}
#[test]
fn raw_inverse_recompute_retains_empty_call_occurrences_at_their_rebuilt_boundaries() {
    let empty = common::action(
        "perm_action",
        "empty",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[0]}),
        "perm_action @empty (t:wire mut) () () () ()",
        "qloop 0 {\nx t\n}",
    );
    let mix = common::caller(
        &empty,
        "perm_action",
        "mix",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @mix (t:wire mut) (x:wire in) () () ()",
        "d = qcopy x\naction_call @empty (d) () () ()\nadj action_call @empty (d) () () ()\ncx d t\nquncopy x d",
    );
    let bytes = common::caller(
        &mix,
        "oracle",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () ()\nregs { r:reg[1] }",
        "lift.perm_call @mix (r) (x) () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    for point in ["entry:2", "entry:4"] {
        let mut trace =
            common::trace_distinct_tokens(&object, "outer", 1, &json!([]), [2, 0, 0, 0]).unwrap();
        let carrier = &trace
            .cleanups
            .iter()
            .find(|p| p.carrier.path.last().unwrap().point == point)
            .unwrap()
            .carrier;
        let e = parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
        trace.realise_cleanup_entries(&[e]).unwrap();
        let calls = trace
            .invocations
            .iter()
            .filter(|i| i.name == "empty")
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), if point == "entry:2" { 12 } else { 8 });
        for call in calls {
            assert_eq!(call.start, call.end);
            assert!(call.start <= trace.events.len());
            let parent = &trace.invocations[call.parent.unwrap()];
            assert!(parent.start <= call.start && call.end <= parent.end);
        }
        check_basis(&trace);
    }
}
