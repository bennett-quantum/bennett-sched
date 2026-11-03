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

#[test]
fn action_oracle_request_binds_callee_history_and_every_concrete_selector() {
    use bennett_sched::{Outcome, Request, schedule::Document};
    let bytes = caller("oracle_call @inc b3 (x) t f () ()");
    let gates=["x","cx","ccx","mcx","swap"].map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}));
    let target = json!({"ptr_widths_supported":[32],"classes":{"fast":{"capacity":"unbounded","qubit_second":1,"dirty_rate":1}},"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"gates":gates,"schemes":[],"qram":null,"tables":{},"imports":{},"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}});
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(&canonical::to_vec(&target).unwrap())
        .unwrap();
    let root = request
        .object
        .as_ref()
        .unwrap()
        .closure_hash("a")
        .unwrap()
        .to_owned();
    let child = request
        .object
        .as_ref()
        .unwrap()
        .closure_hash("inc")
        .unwrap()
        .to_owned();
    request.set_region(&root, "forward", 1).unwrap();
    request
        .set_params(&canonical::to_vec(&params()).unwrap())
        .unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let value = canonical::require_jcs(&response.schedule).unwrap();
    let document = Document::parse(value).unwrap();
    document.validate_request(&request).unwrap();
    let trace = Trace::build(request.object.as_ref().unwrap(), "a", 1, &params(), [0; 4]).unwrap();
    for event in &trace.events {
        if event.location.kind == bennett_sched::schedule::Kind::Alloc {
            bennett_sched::schedule::resolve(
                &document.entries,
                &event.location,
                event.location.kind,
            )
            .unwrap()
            .expect("explicit class entry must address this exact nested allocation");
        }
    }
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    let region = &certificate["regions"][&child];
    assert_eq!(region["frame"]["history"].as_array().unwrap().len(), 3);
    for history in region["frame"]["history"].as_array().unwrap() {
        assert!(
            region["frame"]["scratch"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["id"] != history["id"])
        );
    }
    assert!(
        region["frame"]["history"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["width"] == 7)
    );
    assert_eq!(
        certificate["bindings"]["invocation"]["budget"],
        json!(["0", "0", "0", "0"])
    );
    assert_eq!(
        region["claims"][0]["invocation"]["budget"],
        json!(["3", "0", "0", "0"])
    );
    assert_eq!(
        certificate["composition"]["multiplicities"][format!("{child}:forward:1")],
        1
    );
    for second in ["b3", "b2"] {
        let bytes = caller(&format!(
            "oracle_call @inc b3 (x) t f () ()\noracle_call @inc {second} (x) t f () ()"
        ));
        let mut repeated = Request::default();
        repeated.set_object(&bytes).unwrap();
        repeated
            .set_target(&canonical::to_vec(&target).unwrap())
            .unwrap();
        let hash = repeated
            .object
            .as_ref()
            .unwrap()
            .closure_hash("a")
            .unwrap()
            .to_owned();
        repeated.set_region(&hash, "forward", 1).unwrap();
        repeated
            .set_params(&canonical::to_vec(&params()).unwrap())
            .unwrap();
        repeated.budget = Some([0; 4]);
        repeated.set_accuracy("0").unwrap();
        let response = repeated.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let c = canonical::require_jcs(&response.certificate).unwrap();
        let frames = c["regions"][&child]["frame_contexts"].as_array().unwrap();
        let resources = c["regions"][&child]["resource_contexts"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(resources.len(), 2);
        let mut ids = std::collections::BTreeSet::new();
        for (i, n) in [3, if second == "b3" { 3 } else { 2 }]
            .into_iter()
            .enumerate()
        {
            let history = frames[i]["frame"]["history"].as_array().unwrap();
            assert_eq!(history.len(), n);
            for h in history {
                assert!(ids.insert(h["id"].as_str().unwrap()));
            }
            assert_eq!(frames[i]["context_key"], resources[i]["context_key"]);
        }
        assert_eq!(c["regions"][&child]["resources"], region["resources"]);
        let child_time = resources
            .iter()
            .map(|r| bennett_sched::decimal::value(&r["resources"]["time"]).unwrap())
            .sum::<bennett_sched::decimal::Rational>();
        let new_time =
            bennett_sched::decimal::value(&c["regions"][&hash]["resources"]["time"]).unwrap();
        assert_eq!(new_time, child_time);
        assert_eq!(
            c["regions"][&hash]["resources"]["peak_qubits[fast]"],
            certificate["regions"][&root]["resources"]["peak_qubits[fast]"]
        );
    }
}

#[test]
fn source_adjoints_remap_caller_loop_occurrences_without_reversing_oracle_steps() {
    let mut sig = signature();
    sig["bounds"] = json!([2]);
    let a = common::caller(
        &increment(),
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire[2] mut, f:wire[4] mut) (x:wire[2] in) () (b2:token, b3:token) ()",
        "qloop 2 {\noracle_call @inc b3 (x) t f () ()\n}",
    );
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
        let mut calls = trace
            .invocations
            .iter()
            .filter(|i| i.name == "inc")
            .collect::<Vec<_>>();
        calls.sort_by_key(|i| i.start);
        assert_eq!(calls.len(), 2);
        for (iteration, inv) in calls.iter().enumerate() {
            assert_eq!(inv.path[1].iteration, vec![iteration as u64]);
            let actual = &trace.events[inv.start..inv.end];
            let reference = Trace::build(&object, "inc", 1, &json!([]), [3, 0, 0, 0]).unwrap();
            for (actual, reference) in actual.iter().zip(&reference.events) {
                assert_eq!(actual.location.path[1].iteration, vec![iteration as u64]);
                assert_eq!(
                    actual.location.path[1].entered_oracle,
                    Some(reference.location.root)
                );
                assert_eq!(actual.location.generated, reference.location.generated);
            }
        }
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let program = basis::Program::compile(&leaves).unwrap();
        let mut cases = Vec::new();
        for tuple in 0..512 {
            let mut bits = BTreeMap::new();
            put(&mut bits, "in:0", 2, tuple & 3);
            put(&mut bits, "mut:0", 2, (tuple >> 2) & 3);
            put(&mut bits, "mut:1", 4, (tuple >> 4) & 15);
            bits.insert("control:0".into(), tuple & 256 != 0);
            cases.push(bits);
        }
        let before = cases.clone();
        program.run_many(&mut cases).unwrap();
        assert_eq!(cases, before, "two identical oracle involutions cancel");
    }
}

#[test]
fn published_k77_two_token_inc8_witness_covers_every_input_and_both_controls() {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let inc = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n');
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[8,4],"in":[8],"mem":[],"args":["token","token"],"bounds":[]});
    let bytes = common::caller(
        inc.as_bytes(),
        "perm_action",
        "two_tokens",
        sig,
        "perm_action @two_tokens (t:wire[8] mut, f:wire[4] mut) (x:wire[8] in) () (b2:token, b3:token) ()",
        "oracle_call @inc8 b2 (x) t f () ()\noracle_call @inc8 b3 (x) t f () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "two_tokens", 1, &params(), [0; 4]).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let program = basis::Program::compile(&leaves).unwrap();
    let mut cases = Vec::new();
    for x in 0..256 {
        for control in [false, true] {
            let mut bits = BTreeMap::new();
            put(&mut bits, "in:0", 8, x);
            put(&mut bits, "mut:0", 8, 0);
            put(&mut bits, "mut:1", 4, 0);
            bits.insert("control:0".into(), control);
            cases.push(bits);
        }
    }
    program.run_many(&mut cases).unwrap();
    for (index, bits) in cases.iter().enumerate() {
        let x = index as u64 / 2;
        let control = index % 2 == 1;
        assert_eq!(get(bits, "in:0", 8), x);
        assert_eq!(
            get(bits, "mut:0", 8),
            if control { (x + 1) % 256 } else { 0 }
        );
        assert_eq!(get(bits, "mut:1", 4), u64::from(control));
        assert_eq!(bits.len(), 21);
    }
}
