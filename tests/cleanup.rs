use bennett_sched::{object::Object, source::Trace};
use serde_json::json;
mod common;

fn simple(body: &str) -> Object {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[2],"in":[2],"mem":[],"args":[],"bounds":[]});
    Object::parse(&common::action(
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire[2] mut) (x:wire[2] in) () () ()",
        body,
    ))
    .unwrap()
}
#[test]
fn release_cleanup_coalesces_the_maximal_terminal_source_chain_and_keeps_disjoint_rows_out() {
    let object =
        simple("d = qalloc any 1\ncx x[0] d\ncx x[1] d\ncx x[1] d\nx t[0]\ncx x[0] d\nqfree d");
    let trace = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap();
    assert_eq!(trace.cleanups.len(), 1);
    let pair = &trace.cleanups[0];
    assert_eq!(pair.carrier.path.last().unwrap().point, "entry:6");
    assert_eq!(pair.carrier.generated, 0);
    assert_eq!(
        pair.computation
            .iter()
            .map(|l| l.path.last().unwrap().point.as_str())
            .collect::<Vec<_>>(),
        ["entry:1", "entry:2"]
    );
    assert_eq!(
        pair.cleanup
            .iter()
            .map(|l| l.path.last().unwrap().point.as_str())
            .collect::<Vec<_>>(),
        ["entry:3", "entry:5", "entry:6"]
    );
    let empty = Trace::build(
        &simple("d = qalloc any 1\nqfree d"),
        "a",
        0,
        &json!([]),
        [0; 4],
    )
    .unwrap();
    assert_eq!(empty.cleanups.len(), 1);
    assert!(empty.cleanups[0].computation.is_empty());
}

#[test]
fn intrinsic_boundaries_and_joint_call_inverses_have_exactly_one_owner() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[2],"in":[],"mem":[],"args":[],"bounds":[]});
    let base = common::action(
        "perm_action",
        "mix",
        sig,
        "perm_action @mix (a:wire[2] mut) () () () ()",
        "cx a[0] a[1]",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[2],"mem":[],"args":[],"bounds":[]});
    let bytes = common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () (x:wire[2] in) () () ()",
        "d = qcopy x\naction_call @mix (d) () () ()\nadj action_call @mix (d) () () ()\nquncopy x d",
    );
    let trace = Trace::build(
        &Object::parse(&bytes).unwrap(),
        "outer",
        0,
        &json!([]),
        [0; 4],
    )
    .unwrap();
    let mut points = trace
        .cleanups
        .iter()
        .filter(|p| p.owner == 0)
        .map(|p| p.carrier.path.last().unwrap().point.as_str())
        .collect::<Vec<_>>();
    points.sort();
    assert_eq!(points, ["entry:2", "entry:3"]);
    for order in ["qfree a\nqfree b", "qfree b\nqfree a"] {
        let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,1],"in":[],"mem":[],"args":[],"bounds":[]});
        let base = common::action(
            "perm_action",
            "swap2",
            sig,
            "perm_action @swap2 (a:wire mut, b:wire mut) () () () ()",
            "swap a b",
        );
        let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[],"mem":[],"args":[],"bounds":[]});
        let bytes = common::caller(
            &base,
            "perm_action",
            "outer",
            sig,
            "perm_action @outer () () () () ()",
            &format!(
                "a = qalloc any 1\nb = qalloc any 1\naction_call @swap2 (a, b) () () ()\nadj action_call @swap2 (a, b) () () ()\n{order}"
            ),
        );
        let trace = Trace::build(
            &Object::parse(&bytes).unwrap(),
            "outer",
            0,
            &json!([]),
            [0; 4],
        )
        .unwrap();
        let pairs = trace
            .cleanups
            .iter()
            .filter(|p| p.owner == 0)
            .collect::<Vec<_>>();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].carrier.path.last().unwrap().point, "entry:4");
        assert_eq!(pairs[0].cleanup.len(), 2);
        assert_eq!(pairs[1].cleanup.len(), 1);
        assert_eq!(
            pairs[0].computation[0].path.last().unwrap().point,
            "entry:2"
        );
        assert!(pairs[1].computation.is_empty());
    }
}

#[test]
fn k80_recompute_owns_eight_contiguous_leaves_and_cleanup_ties_are_resolved_at_the_carrier() {
    use bennett_sched::{
        expansion::{Operation, simulate_basis},
        schedule::{Kind, parse_entry},
    };
    use std::collections::BTreeMap;
    let object = simple("d = qalloc any 1\ncx x[0] d\ncx x[1] d\ncx x[1] d\ncx x[0] d\nqfree d");
    let mut trace = Trace::build(&object, "a", 1, &json!([]), [0; 4]).unwrap();
    let carrier = trace.cleanups[0].carrier.clone();
    let entry=parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
    trace.realise_cleanup_entries(&[entry.clone()]).unwrap();
    let replacement = trace
        .events
        .iter()
        .filter(|e| e.location.path.last().unwrap().point == "entry:5")
        .collect::<Vec<_>>();
    assert_eq!(replacement.len(), 8);
    assert_eq!(
        replacement
            .iter()
            .map(|e| e.location.generated)
            .collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );
    assert_eq!(
        replacement
            .iter()
            .filter(|e| matches!(e.leaf.op, Operation::Gate(_)))
            .count(),
        5
    );
    assert_eq!(replacement[0].location.kind, Kind::Alloc);
    assert_eq!(trace.cleanups[0].realisation, "recompute");
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for bits in 0..32 {
        let mut state = BTreeMap::from([
            ("mut:0[0]".into(), bits & 1 != 0),
            ("mut:0[1]".into(), bits & 2 != 0),
            ("in:0[0]".into(), bits & 4 != 0),
            ("in:0[1]".into(), bits & 8 != 0),
            ("control:0".into(), bits & 16 != 0),
        ]);
        let before = state.clone();
        simulate_basis(&leaves, &mut state).unwrap();
        assert_eq!(state, before);
    }
    let mut trace = Trace::build(&object, "a", 1, &json!([]), [0; 4]).unwrap();
    let error = trace
        .realise_cleanup_entries(&[entry.clone(), entry])
        .unwrap_err();
    assert!(error.reason.contains("multiple maximal"));
    assert_eq!(trace.events.len(), 6);
}

#[test]
fn empty_and_intrinsic_recompute_keep_the_release_and_restore_all_source_inputs() {
    use bennett_sched::{expansion::simulate_basis, schedule::parse_entry};
    use std::collections::BTreeMap;
    for body in ["d = qalloc any 1\nqfree d", "d = qcopy x\nquncopy x d"] {
        let object = simple(body);
        let mut trace = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap();
        let p = &trace.cleanups[0];
        let entry=parse_entry(&json!({"path":p.carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
        trace.realise_cleanup_entries(&[entry]).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for bits in 0..16 {
            let mut state = ["mut:0[0]", "mut:0[1]", "in:0[0]", "in:0[1]"]
                .iter()
                .enumerate()
                .map(|(i, n)| (n.to_string(), bits & (1 << i) != 0))
                .collect::<BTreeMap<_, _>>();
            let before = state.clone();
            simulate_basis(&leaves, &mut state).unwrap();
            assert_eq!(state, before, "{body}");
        }
    }
}

#[test]
fn recompute_replay_uses_carrier_coordinates_instead_of_importing_original_loop_iterations() {
    use bennett_sched::schedule::parse_entry;
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1,1],"mem":[],"args":[],"bounds":[1]});
    let bytes = common::action(
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire mut) (x:wire in, z:wire in) () () ()",
        "d = qalloc any 1\nqloop 1 {\ncx x d\ncx z d\n}\ncx z d\ncx x d\nqfree d",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut trace = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap();
    let carrier = trace.cleanups[0].carrier.clone();
    let entry=parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
    trace.realise_cleanup_entries(&[entry]).unwrap();
    let fresh = trace
        .events
        .iter()
        .filter(|e| e.location.path == carrier.path)
        .collect::<Vec<_>>();
    assert_eq!(fresh.len(), 8);
    assert!(
        fresh
            .iter()
            .all(|e| e.location.path.last().unwrap().iteration.is_empty())
    );
    assert_eq!(trace.events[1].location.path.last().unwrap().iteration, [0]);
}
