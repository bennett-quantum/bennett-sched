use bennett_sched::{canonical, object::Object, source::Trace};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "common/basis.rs"]
mod basis;
mod common;

fn increment() -> Vec<u8> {
    common::action(
        "oracle",
        "inc",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[2],"tgt":2,"fault":4,"mem":[],"args":[]}),
        "oracle @inc (x:wire[2] in) (t:wire[2]) (f:wire[4]) () ()\nregs { one:reg[2], r:reg[2] }",
        "one = lift.const.i2 1\nr = lift.add x one\nlift.retval r",
    )
}
fn signature() -> Value {
    json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[2,4],"in":[2],"mem":[],"args":["token","token"],"bounds":[]})
}
fn caller(body: &str) -> Vec<u8> {
    common::caller(
        &increment(),
        "perm_action",
        "a",
        signature(),
        "perm_action @a (t:wire[2] mut, f:wire[4] mut) (x:wire[2] in) () (b2:token, b3:token) ()",
        body,
    )
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
fn params() -> Value {
    json!([["2", "0", "0", "0"], ["3", "0", "0", "0"]])
}

#[test]
fn two_oracle_tokens_bind_distinct_constructions_independently_of_the_action_budget() {
    let bytes = caller("oracle_call @inc b2 (x) t f () ()\noracle_call @inc b3 (x) t f () ()");
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "a", 1, &params(), [0; 4]).unwrap();
    assert_eq!(trace.invocations.len(), 3);
    assert_eq!(trace.invocations[1].budget, [2, 0, 0, 0]);
    assert_eq!(trace.invocations[2].budget, [3, 0, 0, 0]);
    let mut identities = std::collections::BTreeSet::new();
    for e in &trace.events {
        if let bennett_sched::expansion::Operation::Alloc { identity, .. } = &e.leaf.op {
            assert!(
                identities.insert(identity.clone()),
                "callee private allocation identity collided"
            );
        }
        assert_eq!(e.location.path.len(), 2);
        assert_eq!(e.location.path[0].call_index, Some(0));
    }
    check(&trace);
}
fn check(trace: &Trace) {
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let program = basis::Program::compile(&leaves).unwrap();
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for x in 0..4 {
        for t in 0..4 {
            for f in 0..16 {
                for c in [false, true] {
                    let mut bits = BTreeMap::new();
                    put(&mut bits, "in:0", 2, x);
                    put(&mut bits, "mut:0", 2, t);
                    put(&mut bits, "mut:1", 4, f);
                    bits.insert("control:0".into(), c);
                    cases.push(bits);
                    expected.push((x, t, f, c));
                }
            }
        }
    }
    program.run_many(&mut cases).unwrap();
    for (bits, (x, t, f, c)) in cases.iter().zip(expected) {
        assert_eq!(get(bits, "in:0", 2), x);
        assert_eq!(get(bits, "mut:0", 2), t ^ if c { (x + 1) % 4 } else { 0 });
        assert_eq!(get(bits, "mut:1", 4), f ^ u64::from(c));
        assert_eq!(bits.len(), 9);
    }
}

#[test]
fn enclosing_action_adjoints_keep_oracle_construction_step_phase_and_forward_ordinals() {
    let a = caller("oracle_call @inc b2 (x) t f () ()\noracle_call @inc b3 (x) t f () ()");
    for orientation in ["", "adj ", "adj adj "] {
        let bytes = common::caller(
            &a,
            "perm_action",
            "b",
            signature(),
            "perm_action @b (t:wire[2] mut, f:wire[4] mut) (x:wire[2] in) () (b2:token, b3:token) ()",
            &format!("{orientation}action_call @a (t, f) (x) () (b2, b3)"),
        );
        let object = Object::parse(&bytes).unwrap();
        let trace = Trace::build(&object, "b", 1, &params(), [0; 4]).unwrap();
        for inv in trace.invocations.iter().filter(|i| i.name == "inc") {
            let actual = &trace.events[inv.start..inv.end];
            assert_eq!(
                actual.first().unwrap().location.path.last().unwrap().point,
                "$construct"
            );
            assert_eq!(
                actual.last().unwrap().location.path.last().unwrap().point,
                "$release"
            );
            let reference = Trace::build(&object, "inc", 1, &json!([]), inv.budget).unwrap();
            assert_eq!(actual.len(), reference.events.len());
            for (a, b) in actual.iter().zip(&reference.events) {
                assert_eq!(
                    a.location.path.last().unwrap().json(),
                    b.location.path.last().unwrap().json()
                );
                assert_eq!(a.location.generated, b.location.generated);
                assert_eq!(a.location.path[1].entered_oracle, Some(b.location.root));
            }
        }
        check(&trace);
    }
}

#[test]
fn malformed_call_operands_and_misplaced_rows_are_argument_errors() {
    for body in [
        "oracle_call @inc t (x) t f () ()",
        "oracle_call @inc b3 (x) x f () ()",
        "oracle_call @inc b3 (x) t[0] f () ()",
        "lift.free t",
    ] {
        let error = Object::parse(&caller(body))
            .and_then(|object| Trace::build(&object, "a", 0, &params(), [0; 4]))
            .err()
            .expect("invalid call admitted");
        assert_eq!(error.status, bennett_sched::Status::Arg, "{body}: {error}");
    }
    let mut object = Object::parse(&increment()).unwrap();
    object.functions.get_mut("inc").unwrap().body[0].op = "lift.unknown".into();
    let error = Trace::build(&object, "inc", 0, &json!([]), [3, 0, 0, 0])
        .err()
        .expect("unknown row admitted");
    assert_eq!(error.status, bennett_sched::Status::Arg);
}

#[test]
fn region_call_cycles_are_rejected_even_inside_zero_trip_loops() {
    let noop = common::caller(
        &increment(),
        "perm_action",
        "noop",
        signature(),
        "perm_action @noop (t:wire[2] mut, f:wire[4] mut) (x:wire[2] in) () (b2:token, b3:token) ()",
        "x t[0]\nx t[0]",
    );
    let mut sig = signature();
    sig["bounds"] = json!([0]);
    let bytes = common::caller(
        &noop,
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire[2] mut, f:wire[4] mut) (x:wire[2] in) () (b2:token, b3:token) ()",
        "qloop 0 {\naction_call @noop (t, f) (x) () (b2, b3)\n}",
    );
    let mut object = Object::parse(&bytes).unwrap();
    // Change only the already typed edge in the source AST to a self edge.
    object.functions.get_mut("a").unwrap().body[0].bodies[0][0].operands[0] = "@a".into();
    let error = Trace::build(&object, "a", 0, &params(), [0; 4])
        .err()
        .expect("region cycle hidden by zero-trip loop");
    assert_eq!(error.status, bennett_sched::Status::Arg);
    assert!(error.reason.contains("cyclic region"), "{error}");
}
