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
    let trace = common::trace_distinct_tokens(&object, "a", 0, &json!([]), [0; 4]).unwrap();
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
    let empty = common::trace_distinct_tokens(
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
    let trace = common::trace_distinct_tokens(
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
        let trace = common::trace_distinct_tokens(
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
    let mut trace = common::trace_distinct_tokens(&object, "a", 1, &json!([]), [0; 4]).unwrap();
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
    let mut trace = common::trace_distinct_tokens(&object, "a", 1, &json!([]), [0; 4]).unwrap();
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
        let mut trace = common::trace_distinct_tokens(&object, "a", 0, &json!([]), [0; 4]).unwrap();
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
    let mut trace = common::trace_distinct_tokens(&object, "a", 0, &json!([]), [0; 4]).unwrap();
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

fn call_cleanup(body: &str, mix: bool) -> Object {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[if mix {2}else{1}],"in":if mix {vec![]}else{vec![1]},"mem":[],"args":[],"bounds":[]});
    let base = common::action(
        "perm_action",
        "mix",
        sig,
        if mix {
            "perm_action @mix (a:wire[2] mut) () () () ()"
        } else {
            "perm_action @mix (a:wire mut) (x:wire in) () () ()"
        },
        if mix { "cx a[0] a[1]" } else { "cx x a" },
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[2],"mem":[],"args":[],"bounds":[]});
    Object::parse(&common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () (x:wire[2] in) () () ()",
        body,
    ))
    .unwrap()
}

fn recompute_at(trace: &mut Trace, point: &str) {
    let carrier = trace
        .cleanups
        .iter()
        .find(|p| p.owner == 0 && p.carrier.path.last().unwrap().point == point)
        .unwrap()
        .carrier
        .clone();
    let entry = bennett_sched::schedule::parse_entry(&json!({
        "path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),
        "step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}
    }))
    .unwrap();
    trace.realise_cleanup_entries(&[entry]).unwrap();
}

#[test]
fn k80_two_call_recompute_has_fresh_edges_and_separate_caller_leaf_ordinals() {
    use bennett_sched::expansion::{Operation, inverse, simulate_basis};
    use std::collections::BTreeMap;
    // Distinct input slices make the two call occurrences distinguishable.
    let object = call_cleanup(
        "d = qalloc any 1\naction_call @mix (d) (x[0]) () ()\naction_call @mix (d) (x[1]) () ()\nadj action_call @mix (d) (x[1]) () ()\nadj action_call @mix (d) (x[0]) () ()\nqfree d",
        false,
    );
    let mut trace = common::trace_distinct_tokens(&object, "outer", 1, &json!([]), [0; 4]).unwrap();
    recompute_at(&mut trace, "entry:5");
    let caller = trace
        .events
        .iter()
        .filter(|e| e.owner == 0 && e.location.path.last().unwrap().point == "entry:5")
        .collect::<Vec<_>>();
    assert_eq!(
        caller
            .iter()
            .map(|e| e.location.generated)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(matches!(&caller[1].leaf.op,Operation::Gate(g)if g.op=="cx"));
    let fresh = trace
        .invocations
        .iter()
        .filter(|i| i.parent == Some(0) && i.path.last().unwrap().point == "entry:5")
        .collect::<Vec<_>>();
    assert_eq!(fresh.len(), 4);
    assert_eq!(
        fresh
            .iter()
            .map(|i| i.path.last().unwrap().call_index.unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    for child in fresh {
        assert_eq!(child.end - child.start, 1);
        let e = &trace.events[child.start];
        assert_eq!(e.location.path.last().unwrap().point, "entry:0");
        assert_eq!(e.location.generated, 0);
        assert_eq!(
            e.owner,
            trace
                .invocations
                .iter()
                .position(|i| std::ptr::eq(i, child))
                .unwrap()
        );
    }
    assert!(!trace.invocations.iter().any(|i| i.parent == Some(0)
        && ["entry:3", "entry:4"].contains(&i.path.last().unwrap().point.as_str())));
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for bits in 0..8 {
        let mut state = BTreeMap::from([
            ("in:0[0]".into(), bits & 1 != 0),
            ("in:0[1]".into(), bits & 2 != 0),
            ("control:0".into(), bits & 4 != 0),
        ]);
        let before = state.clone();
        simulate_basis(&leaves, &mut state).unwrap();
        assert_eq!(state, before);
        simulate_basis(&inverse(&leaves), &mut state).unwrap();
        assert_eq!(state, before);
    }
}

#[test]
fn k81_call_recompute_replays_the_exact_nonzero_copy_history_without_owning_it() {
    use bennett_sched::expansion::simulate_basis;
    use std::collections::BTreeMap;
    let object = call_cleanup(
        "d = qcopy x\naction_call @mix (d) () () ()\nadj action_call @mix (d) () () ()\nquncopy x d",
        true,
    );
    let mut trace = common::trace_distinct_tokens(&object, "outer", 1, &json!([]), [0; 4]).unwrap();
    let original = trace
        .events
        .iter()
        .filter(|e| e.location.path.first().unwrap().point == "entry:0")
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    recompute_at(&mut trace, "entry:2");
    let retained = trace
        .events
        .iter()
        .filter(|e| e.location.path.first().unwrap().point == "entry:0")
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    assert_eq!(format!("{original:?}"), format!("{retained:?}"));
    let fresh = trace
        .invocations
        .iter()
        .filter(|i| i.parent == Some(0) && i.path.last().unwrap().point == "entry:2")
        .collect::<Vec<_>>();
    assert_eq!(fresh.len(), 2);
    assert_eq!(
        fresh
            .iter()
            .map(|i| i.path.last().unwrap().call_index.unwrap())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(trace.cleanups.iter().any(|p| p.owner == 0
        && p.carrier.path.last().unwrap().point == "entry:3"
        && p.realisation == "reverse"));
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for bits in 0..8 {
        let mut state = BTreeMap::from([
            ("in:0[0]".into(), bits & 1 != 0),
            ("in:0[1]".into(), bits & 2 != 0),
            ("control:0".into(), bits & 4 != 0),
        ]);
        let before = state.clone();
        simulate_basis(&leaves, &mut state).unwrap();
        assert_eq!(state, before);
    }
}

#[test]
fn fresh_call_cleanup_selectors_apply_after_rebinding_to_the_new_carrier_edge() {
    use bennett_sched::schedule::parse_entry;
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]});
    let base = common::action(
        "perm_action",
        "mix",
        sig,
        "perm_action @mix (a:wire mut) (x:wire in) () () ()",
        "d = qcopy x\ncx d a\nquncopy x d",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[1],"mem":[],"args":[],"bounds":[]});
    let bytes = common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () (x:wire in) () () ()",
        "d = qalloc any 1\naction_call @mix (d) (x) () ()\nadj action_call @mix (d) (x) () ()\nqfree d",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut trace = common::trace_distinct_tokens(&object, "outer", 0, &json!([]), [0; 4]).unwrap();
    let carrier = trace
        .cleanups
        .iter()
        .find(|p| p.owner == 0)
        .unwrap()
        .carrier
        .clone();
    let root=parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
    let mut edge = carrier.path.last().unwrap().json();
    edge["call_index"] = json!(0);
    let nested=parse_entry(&json!({"path":[edge,{"symbol":object.symbols["mix"].interface_hash,"point":"entry:2","iteration":[]}],"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
    trace.realise_cleanup_entries(&[root, nested]).unwrap();
    let replay = trace
        .invocations
        .iter()
        .position(|i| {
            i.parent == Some(0)
                && i.path.last().unwrap().point == "entry:3"
                && i.path.last().unwrap().call_index == Some(0)
        })
        .unwrap();
    assert!(
        trace
            .cleanups
            .iter()
            .any(|p| p.owner == replay && p.realisation == "recompute")
    );
}

#[test]
fn a_recomputed_schedule_admits_its_fresh_call_paths_and_rejects_nonexistent_occurrences() {
    use bennett_sched::{Outcome, Request, canonical, schedule::Document};
    use std::sync::Arc;
    let object = call_cleanup(
        "d = qalloc any 1\naction_call @mix (d) (x[0]) () ()\naction_call @mix (d) (x[1]) () ()\nadj action_call @mix (d) (x[1]) () ()\nadj action_call @mix (d) (x[0]) () ()\nqfree d",
        false,
    );
    let mut request = Request::default();
    request
        .set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    request
        .set_region(object.closure_hash("outer").unwrap(), "forward", 1)
        .unwrap();
    request.object = Some(Arc::new(object));
    request.set_params(b"[]").unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("0").unwrap();
    let response = common::schedule_distinct_tokens(&mut request).unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let mut schedule = canonical::require_jcs(&response.schedule).unwrap();
    let source = request.object.as_ref().unwrap();
    let mut path =
        json!([{"symbol":source.symbols["outer"].interface_hash,"point":"entry:5","iteration":[]}]);
    schedule["entries"].as_array_mut().unwrap().push(json!({"path":path,"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}}));
    path[0]["call_index"] = json!(2);
    path.as_array_mut().unwrap().push(
        json!({"symbol":source.symbols["mix"].interface_hash,"point":"entry:0","iteration":[]}),
    );
    schedule["entries"].as_array_mut().unwrap().push(json!({"path":path,"step":"*","phase":"*","generated":0,"kind":"gate","gate":{"realisation":"native"}}));
    Document::parse(schedule.clone())
        .unwrap()
        .validate_request(&request)
        .unwrap();
    let last = schedule["entries"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap();
    last["path"][0]["call_index"] = json!(4);
    let error = Document::parse(schedule)
        .unwrap()
        .validate_request(&request)
        .unwrap_err();
    assert_eq!(
        error.status,
        bennett_sched::Status::ScheduleInfeasible,
        "{error}"
    );
    assert!(error.reason.contains("call_index"), "{error}");
}

#[test]
fn fresh_source_adjoints_rebuild_while_flags_and_local_region_support_spaces() {
    use bennett_sched::expansion::{Operation, simulate_basis};
    use std::collections::{BTreeMap, BTreeSet};
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[2],"mem":[],"args":[],"bounds":[2]});
    let base = common::action(
        "perm_action",
        "mix",
        sig,
        "perm_action @mix (a:wire mut) (cond:wire[2] in) () () ()",
        "qregion r 0 1\nqloop 2 while cond {\nx a\n}",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[2],"mem":[],"args":[],"bounds":[]});
    let bytes = common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () (x:wire[2] in) () () ()",
        "d = qalloc any 1\naction_call @mix (d) (x) () ()\nadj action_call @mix (d) (x) () ()\nqfree d",
    );
    let mut trace = common::trace_distinct_tokens(
        &Object::parse(&bytes).unwrap(),
        "outer",
        1,
        &json!([]),
        [0; 4],
    )
    .unwrap();
    recompute_at(&mut trace, "entry:3");
    let children = trace
        .invocations
        .iter()
        .filter(|i| i.parent == Some(0) && i.path.last().unwrap().point == "entry:3")
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2);
    assert!(!children[0].adjoint);
    assert!(children[1].adjoint);
    let mut ids = BTreeSet::new();
    for c in children {
        let mut next = BTreeMap::new();
        for e in &trace.events[c.start..c.end] {
            assert_eq!(e.location.path.first().unwrap().point, "entry:3");
            let p = e.location.path.last().unwrap();
            let key = (p.point.clone(), p.iteration.clone());
            let ordinal = next.entry(key).or_insert(0);
            assert_eq!(e.location.generated, *ordinal);
            *ordinal += 1;
            if let Operation::Alloc { identity, .. } = &e.leaf.op {
                assert!(ids.insert(identity.clone()));
            }
        }
        assert_eq!(
            trace.events[c.start].location.path.last().unwrap().point,
            "$construct"
        );
        assert_eq!(
            trace.events[c.end - 1].location.path.last().unwrap().point,
            "$release"
        );
    }
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for bits in 0..8 {
        let mut state = BTreeMap::from([
            ("in:0[0]".into(), bits & 1 != 0),
            ("in:0[1]".into(), bits & 2 != 0),
            ("control:0".into(), bits & 4 != 0),
        ]);
        let before = state.clone();
        simulate_basis(&leaves, &mut state).unwrap();
        assert_eq!(state, before);
    }
}

#[test]
fn independent_call_history_does_not_copy_a_nonpermutation_preparation() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    let base = common::action(
        "perm_action",
        "mix",
        sig,
        "perm_action @mix (a:wire mut) () () () ()",
        "x a",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"action","mut":[],"in":[],"mem":[],"args":[],"bounds":[]});
    let bytes = common::caller(
        &base,
        "action",
        "outer",
        sig,
        "action @outer () () () () ()",
        "d = qalloc any 1\nh d\naction_call @mix (d) () () ()\nadj action_call @mix (d) () () ()\nx d\nx d\nh d\nqfree d",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut trace = common::trace_distinct_tokens(&object, "outer", 0, &json!([]), [0; 4]).unwrap();
    let carrier = trace
        .cleanups
        .iter()
        .find(|p| p.owner == 0 && p.carrier.path.last().unwrap().point == "entry:3")
        .unwrap()
        .carrier
        .clone();
    let entry=bennett_sched::schedule::parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
    let error = trace.realise_cleanup_entries(&[entry]).unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::Arg, "{error}");
    assert!(error.reason.contains("permutation"), "{error}");
}

#[test]
fn replay_preserves_empty_call_spans_inside_fresh_callees() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]});
    let mut empty_sig = sig.clone();
    empty_sig["bounds"] = json!([0]);
    let base = common::action(
        "perm_action",
        "empty",
        empty_sig,
        "perm_action @empty (a:wire mut) (x:wire in) () () ()",
        "qloop 0 {\nx a\n}",
    );
    let base = common::caller(
        &base,
        "perm_action",
        "mix",
        sig,
        "perm_action @mix (a:wire mut) (x:wire in) () () ()",
        "cx x a\naction_call @empty (a) (x) () ()",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[1],"mem":[],"args":[],"bounds":[]});
    let bytes = common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () (x:wire in) () () ()",
        "d = qalloc any 1\naction_call @mix (d) (x) () ()\nadj action_call @mix (d) (x) () ()\nqfree d",
    );
    let mut trace = common::trace_distinct_tokens(
        &Object::parse(&bytes).unwrap(),
        "outer",
        0,
        &json!([]),
        [0; 4],
    )
    .unwrap();
    recompute_at(&mut trace, "entry:3");
    for inv in trace.invocations.iter().filter(|i| i.name == "empty") {
        assert_eq!(inv.start, inv.end);
        let parent = &trace.invocations[inv.parent.unwrap()];
        assert_eq!(
            inv.start,
            if parent.adjoint {
                parent.start
            } else {
                parent.end
            },
            "empty child span did not follow source order: {inv:?} {parent:?}"
        );
    }
}

#[test]
fn replayed_oracle_calls_keep_the_explicit_token_and_forward_history_coordinates() {
    use bennett_sched::{
        expansion::{Operation, simulate_basis},
        schedule::Phase,
    };
    use std::collections::{BTreeMap, BTreeSet};
    let base = common::action(
        "oracle",
        "o",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[1],"mem":[],"args":["token"],"bounds":[]});
    let bytes = common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () (x:wire in) () (b:token) ()",
        "t = qalloc any 1\nf = qalloc any 4\noracle_call @o b (x) t f () ()\noracle_call @o b (x) t f () ()\nqfree t\nqfree f",
    );
    let mut trace = common::trace_distinct_tokens(
        &Object::parse(&bytes).unwrap(),
        "outer",
        1,
        &json!([["2", "0", "0", "0"]]),
        [0; 4],
    )
    .unwrap();
    recompute_at(&mut trace, "entry:4");
    let fresh = trace
        .invocations
        .iter()
        .filter(|i| i.parent == Some(0) && i.path.last().unwrap().point == "entry:4")
        .collect::<Vec<_>>();
    assert_eq!(fresh.len(), 2);
    let mut ids = BTreeSet::new();
    for (n, inv) in fresh.iter().enumerate() {
        assert_eq!(inv.budget, [2, 0, 0, 0]);
        assert!(!inv.adjoint);
        assert_eq!(inv.path.last().unwrap().call_index, Some(n as u64));
        let mut phases = BTreeSet::new();
        for e in &trace.events[inv.start..inv.end] {
            assert_eq!(e.location.root, Default::default());
            let coordinate = e.location.path[0].entered_oracle.unwrap();
            if coordinate.phase == Some(Phase::Fwd) {
                phases.insert("fwd");
            }
            if coordinate.phase == Some(Phase::Rev) {
                phases.insert("rev");
            }
            if let Operation::Alloc { identity, .. } = &e.leaf.op {
                assert!(ids.insert(identity.clone()));
            }
        }
        assert_eq!(phases, BTreeSet::from(["fwd", "rev"]));
    }
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for x in [false, true] {
        for c in [false, true] {
            let mut state = BTreeMap::from([("in:0[0]".into(), x), ("control:0".into(), c)]);
            let before = state.clone();
            simulate_basis(&leaves, &mut state).unwrap();
            assert_eq!(state, before);
        }
    }
}
