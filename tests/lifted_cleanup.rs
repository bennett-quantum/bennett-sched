use bennett_sched::{
    object::Object,
    schedule::{Kind, parse_entry},
    source::Trace,
};
use serde_json::{Value, json};
#[path = "common/basis.rs"]
mod basis;
mod common;

fn program(nonzero_history: bool) -> Vec<u8> {
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
        if nonzero_history {
            "r = lift.mov x\nf = lift.const.i4 7\nlift.oracle_call @o b (x) r f () ()\ns = lift.mov r\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval s"
        } else {
            "lift.oracle_call @o b (x) r f () ()\ns = lift.mov r\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval s"
        },
    )
}
fn bytes() -> Vec<u8> {
    program(false)
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
    for (inner, all, history) in [
        (0, false, false),
        (1, false, false),
        (0, true, false),
        (1, true, false),
        (0, true, true),
        (1, true, true),
    ] {
        let object = Object::parse(&program(history)).unwrap();
        let budget = if history { 6 } else { 4 };
        let carrier_count = 2 * budget;
        let mut trace = Trace::build(
            &object,
            "outer",
            1,
            &json!([[inner.to_string(), "0", "0", "0"]]),
            [budget, 0, 0, 0],
        )
        .unwrap();
        assert_eq!(trace.cleanups.len(), carrier_count as usize);
        let e = if all {
            entry(&trace.cleanups[0].carrier, json!("*"), json!("*"))
        } else {
            entry(
                &trace.cleanups[0].carrier,
                json!(if history { 5 } else { 3 }),
                json!("fwd"),
            )
        };
        let replaced = if all { carrier_count as usize } else { 1 };
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
            .filter(|i| {
                i.name == "o" && i.path[0].point == if history { "entry:4" } else { "entry:2" }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls.len(),
            carrier_count as usize + replaced,
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
                (bits & 2 != 0)
                    ^ (bits & 64 != 0
                        && (if history { inner == 0 } else { inner > 0 })
                        && bits & 1 != 0)
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

#[test]
fn fixed_lproc_instances_keep_separate_non_emitting_uncall_carriers() {
    let functions = [
        (
            "o",
            "oracle",
            json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
            "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()\n^entry():\nlift.retval x",
        ),
        (
            "worker",
            "lproc",
            json!({"abi":"0.9","tier":"q","kind":"lproc","params":[1],"result":1,"tokens":["token"],"args":[],"mem":[]}),
            "lproc @worker (p:reg[1]) (r:reg[1]) (b:token) () ()\nregs { f:reg[4] }\n^entry():\nlift.oracle_call @o b (p) r f () ()\nlift.oracle_uncall @o b (p) r f () ()\nlift.ret r",
        ),
        (
            "outer",
            "oracle",
            json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token","token"]}),
            "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b0:token, b1:token)\nregs { y:reg[1], z:reg[1] }\n^entry():\ny = lift.call @worker (x) (b0) () ()\nz = lift.call @worker (x) (b1) () ()\nlift.retval y",
        ),
    ];
    let object = Object::parse(&common::layout_sources(&functions, json!({"ptr_width":3,"H":0,"A_tab":0,"S_cells":3,"null_size":1,"heap_base":1,"heap_limit_max":0,"globals":{},"interface_regions":{}}))).unwrap();
    let mut trace = Trace::build(
        &object,
        "outer",
        0,
        &json!([["0", "0", "0", "0"], ["0", "0", "0", "0"]]),
        [1, 0, 0, 0],
    )
    .unwrap();
    assert_eq!(trace.cleanups.len(), 4);
    let keys = trace
        .cleanups
        .iter()
        .map(|p| p.carrier.path[0].instance.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys, std::collections::BTreeSet::from([vec![0], vec![1]]));
    assert!(trace.cleanups.iter().all(|p| p.carrier.path.len() == 1
        && p.carrier.path[0].symbol == object.symbols["worker"].interface_hash));
    let e = entry(&trace.cleanups[0].carrier, json!("*"), json!("*"));
    trace.realise_cleanup_entries(&[e]).unwrap();
    assert_eq!(
        trace
            .cleanups
            .iter()
            .filter(|p| p.realisation == "recompute")
            .count(),
        2
    );
}

#[test]
fn encoded_lifted_cleanup_documents_admit_fresh_call_paths_and_reject_an_absent_occurrence() {
    use bennett_sched::{Outcome, Request, canonical, schedule::Document};
    let object = object();
    let mut request = Request::default();
    request.set_object(&bytes()).unwrap();
    request
        .set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    request
        .set_region(object.closure_hash("outer").unwrap(), "forward", 0)
        .unwrap();
    request.set_params(br#"[["0","0","0","0"]]"#).unwrap();
    request.budget = Some([4, 0, 0, 0]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(
        certificate["regions"][object.closure_hash("outer").unwrap()]["cleanup"]["forward:0"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    let mut document = canonical::require_jcs(&response.schedule).unwrap();
    // Use defaults for allocations, then request all explicit uncall carriers.
    document["entries"] = json!([{"path":[{"symbol":object.symbols["outer"].interface_hash,"point":"entry:2","iteration":[],"instance":[]}],"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}}]);
    Document::parse(document.clone())
        .unwrap()
        .validate_request(&request)
        .unwrap();
    let fresh = json!({"path":[{"symbol":object.symbols["outer"].interface_hash,"point":"entry:2","iteration":[],"instance":[],"call_index":1},{"symbol":object.symbols["o"].interface_hash,"point":"$construct","iteration":[],"instance":[]}],"step":"*","phase":"*","generated":"*","kind":"alloc","alloc":{"class":"A"}});
    document["entries"].as_array_mut().unwrap().push(fresh);
    Document::parse(document.clone())
        .unwrap()
        .validate_request(&request)
        .unwrap();
    document["entries"][1]["path"][0]["call_index"] = json!(2);
    assert!(
        Document::parse(document)
            .unwrap()
            .validate_request(&request)
            .unwrap_err()
            .reason
            .contains("call_index")
    );
}

#[test]
fn deeply_projected_invocations_retain_their_containing_oracle_coordinates() {
    let source = common::caller(
        &bytes(),
        "oracle",
        "root",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token","token"]}),
        "oracle @root (x:wire in) (t:wire) (fr:wire[4]) () (a:token, b:token)\nregs { r:reg[1], f:reg[4] }",
        "lift.oracle_call @outer a (x) r f () (b)\nlift.retval r",
    );
    let object = Object::parse(&source).unwrap();
    let trace = Trace::build(
        &object,
        "root",
        0,
        &json!([["4", "0", "0", "0"], ["0", "0", "0", "0"]]),
        [1, 0, 0, 0],
    )
    .unwrap();
    let mut checked = 0;
    for (owner, inv) in trace
        .invocations
        .iter()
        .enumerate()
        .filter(|(_, i)| i.name == "o")
    {
        assert_eq!(inv.path.len(), 2);
        assert!(inv.path[0].entered_oracle.unwrap().step.is_some());
        for e in trace.events[inv.start..inv.end]
            .iter()
            .filter(|e| e.owner == owner)
        {
            assert_eq!(inv.containing, e.location.root);
            assert_eq!(
                inv.path[0].entered_oracle,
                e.location.path[0].entered_oracle
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 32);
}
