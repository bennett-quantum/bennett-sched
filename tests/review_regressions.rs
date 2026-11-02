use bennett_sched::{Outcome, Request, canonical};
use serde_json::{Value, json};
use std::collections::BTreeSet;
mod common;

fn signature(mut_widths: &[u64], input_widths: &[u64], bounds: &[u64]) -> Value {
    json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":mut_widths,"in":input_widths,"mem":[],"args":[],"bounds":bounds})
}
fn target(classes: Value, gates: Value) -> Value {
    json!({"ptr_widths_supported":[32,64],"classes":classes,"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"gates":gates,"schemes":[],"qram":null,"tables":{},"imports":{},"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}})
}
fn gates() -> Value {
    json!(["x","cx","ccx"].map(|op|json!({"op":op,"classes":["*"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}})))
}
fn request(bytes: &[u8], region: &str, target: &Value) -> Request {
    let mut r = Request::default();
    r.set_object(bytes).unwrap();
    r.set_target(&canonical::to_vec(target).unwrap()).unwrap();
    let hash = r
        .object
        .as_ref()
        .unwrap()
        .closure_hash(region)
        .unwrap()
        .to_owned();
    r.set_region(&hash, "forward", 0).unwrap();
    r.budget = Some([0; 4]);
    r.set_params(b"[]").unwrap();
    r.set_accuracy("0").unwrap();
    r
}

#[test]
fn repeated_identical_scratch_calls_have_complete_distinct_dynamic_frames() {
    let callee = common::action(
        "perm_action",
        "sc",
        signature(&[1], &[], &[]),
        "perm_action @sc (t:wire mut) () () () ()",
        "d = qalloc any 1\ncx t d\ncx t d\nqfree d",
    );
    for repeats in [1, 2, 3] {
        let body = (0..repeats)
            .map(|_| "action_call @sc (t) () () ()")
            .collect::<Vec<_>>()
            .join("\n");
        let bytes = common::caller(
            &callee,
            "perm_action",
            "outer",
            signature(&[1], &[], &[]),
            "perm_action @outer (t:wire mut) () () () ()",
            &body,
        );
        let r = request(
            &bytes,
            "outer",
            &target(
                json!({"fast":{"capacity":2,"qubit_second":1,"dirty_rate":1}}),
                gates(),
            ),
        );
        let response = r.schedule().unwrap();
        assert_eq!(
            response.outcome,
            Outcome::Ok,
            "{repeats} calls: {:?}",
            response.reason
        );
        let c = canonical::require_jcs(&response.certificate).unwrap();
        let hash = r.object.as_ref().unwrap().closure_hash("sc").unwrap();
        let scratch = c["regions"][hash]["frame"]["scratch"].as_array().unwrap();
        assert_eq!(scratch.len(), repeats);
        assert_eq!(
            scratch
                .iter()
                .map(|s| s["id"].as_str().unwrap())
                .collect::<BTreeSet<_>>()
                .len(),
            repeats
        );
        assert!(
            scratch
                .iter()
                .all(|s| s["width"] == 1 && s["class"] == "fast" && s["state"] == "clean")
        );
        let resources = &c["regions"][r.region.as_ref().unwrap()]["resources"];
        assert_eq!(resources["peak_qubits[fast]"], "2");
        assert_eq!(resources["qubit_second[fast]"], (4 * repeats).to_string());
    }
}

#[test]
fn interface_classes_follow_serializable_defaults_even_when_another_class_has_the_gate() {
    let bytes = common::action(
        "perm_action",
        "flip",
        signature(&[1], &[], &[]),
        "perm_action @flip (t:wire mut) () () () ()",
        "x t",
    );
    for a_capacity in [8, 0] {
        let target = target(
            json!({"a":{"capacity":a_capacity,"qubit_second":1,"dirty_rate":1},"b":{"capacity":8,"qubit_second":1,"dirty_rate":1}}),
            json!([{"op":"x","classes":["b"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}]),
        );
        let r = request(&bytes, "flip", &target);
        let response = r.schedule().unwrap();
        if a_capacity == 8 {
            assert_eq!(
                response.outcome,
                Outcome::Infeasible,
                "an interface on default class a cannot execute an x available only on b"
            );
            assert!(response.schedule.is_empty() && response.certificate.is_empty());
        } else {
            assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
            let c = canonical::require_jcs(&response.certificate).unwrap();
            assert_eq!(
                c["regions"][r.region.as_ref().unwrap()]["frame"]["slots"][0]["class"],
                "b"
            );
        }
    }
}

#[test]
fn topology_vertices_serialize_a_selected_interface_class() {
    let bytes = common::action(
        "perm_action",
        "flip",
        signature(&[1], &[], &[]),
        "perm_action @flip (t:wire mut) () () () ()",
        "x t",
    );
    let mut target = target(
        json!({"a":{"capacity":8,"qubit_second":1,"dirty_rate":1},"b":{"capacity":8,"qubit_second":1,"dirty_rate":1}}),
        json!([{"op":"x","classes":["b"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}]),
    );
    target["topology"] = json!({"vertices":[{"class":"a"},{"class":"b"}],"edges":[]});
    let r = request(&bytes, "flip", &target);
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let schedule = canonical::require_jcs(&response.schedule).unwrap();
    assert_eq!(schedule["interface_vertices"], json!([[1]]));
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(
        certificate["regions"][r.region.as_ref().unwrap()]["frame"]["slots"][0]["class"],
        "b"
    );
}

#[test]
fn infeasible_topology_search_skips_unused_interface_vertex_permutations() {
    use std::{sync::mpsc, time::Duration};
    for width in [2, 8] {
        let bytes = common::action(
            "perm_action",
            "w",
            signature(&[width], &[], &[]),
            &format!("perm_action @w (t:wire[{width}] mut) () () () ()"),
            "cx t[0] t[1]",
        );
        let mut target = target(
            json!({"fast":{"capacity":12,"qubit_second":1,"dirty_rate":1}}),
            gates(),
        );
        target["topology"] = json!({"vertices":(0..12).map(|_|json!({"class":"fast"})).collect::<Vec<_>>(),"edges":[]});
        let r = request(&bytes, "w", &target);
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(r.schedule().map(|r| r.outcome));
        });
        let result = receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("placement search explored irrelevant wire permutations for over30s");
        assert_eq!(result.unwrap(), Outcome::Infeasible, "width{width}");
    }
}

#[test]
fn class_conflicts_skip_vertex_permutations_without_losing_a_feasible_class() {
    use std::{sync::mpsc, time::Duration};
    let bytes = common::action(
        "perm_action",
        "w",
        signature(&[8], &[], &[]),
        "perm_action @w (t:wire[8] mut) () () () ()",
        "cx t[0] t[1]",
    );
    let mut target = target(
        json!({"a":{"capacity":12,"qubit_second":1,"dirty_rate":1},"b":{"capacity":8,"qubit_second":1,"dirty_rate":1}}),
        json!([{"op":"cx","classes":["b"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}]),
    );
    let vertices = (0..20)
        .map(|i| json!({"class":if i<12{"a"}else{"b"}}))
        .collect::<Vec<_>>();
    let edges = (12..20)
        .flat_map(|a| (a + 1..20).map(move |b| json!([a, b])))
        .collect::<Vec<_>>();
    target["topology"] = json!({"vertices":vertices,"edges":edges});
    let r = request(&bytes, "w", &target);
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(r.schedule());
    });
    let response = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("class mismatch retried irrelevant vertex permutations")
        .unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let schedule = canonical::require_jcs(&response.schedule).unwrap();
    assert_eq!(
        schedule["interface_vertices"],
        json!([(12..20).collect::<Vec<_>>()])
    );
}

#[test]
fn admission_rejects_root_maximal_ties_but_accepts_shielded_or_empty_domains() {
    use bennett_sched::{Status, schedule::Document};
    let bytes = common::action(
        "perm_action",
        "flip",
        signature(&[1], &[], &[]),
        "perm_action @flip (t:wire mut) () () () ()",
        "x t",
    );
    let t = target(
        json!({"fast":{"capacity":8,"qubit_second":1,"dirty_rate":1}}),
        gates(),
    );
    let r = request(&bytes, "flip", &t);
    let response = r.schedule().unwrap();
    let mut schedule = canonical::require_jcs(&response.schedule).unwrap();
    let hash = r.object.as_ref().unwrap().symbols["flip"]
        .interface_hash
        .clone();
    let broad = json!({"path":[],"step":"*","phase":"*","generated":"*","kind":"gate","gate":{"realisation":"native"}});
    schedule["entries"] = json!([broad.clone(), broad.clone()]);
    let error = Document::parse(schedule.clone())
        .unwrap()
        .validate_request(&r)
        .unwrap_err();
    assert_eq!(error.status, Status::ScheduleInfeasible);
    assert_eq!(
        error.reason,
        "§11: multiple maximal matching schedule entries"
    );
    let exact = json!({"path":[{"symbol":hash,"point":"entry:0","iteration":[]}],"step":"*","phase":"*","generated":0,"kind":"gate","gate":{"realisation":"native"}});
    schedule["entries"] = json!([broad.clone(), broad.clone(), exact]);
    Document::parse(schedule)
        .unwrap()
        .validate_request(&r)
        .unwrap();
    let empty = common::action(
        "perm_action",
        "empty",
        signature(&[1], &[], &[0]),
        "perm_action @empty (t:wire mut) () () () ()",
        "qloop 0 {\nx t\n}",
    );
    let r = request(&empty, "empty", &t);
    let response = r.schedule().unwrap();
    let mut schedule = canonical::require_jcs(&response.schedule).unwrap();
    schedule["entries"] = json!([broad.clone(), broad]);
    Document::parse(schedule)
        .unwrap()
        .validate_request(&r)
        .unwrap();
}

#[test]
fn admission_rejects_an_alloc_decision_naming_an_undeclared_target_class() {
    use bennett_sched::schedule::Document;
    let bytes = common::action(
        "perm_action",
        "scratch",
        signature(&[1], &[], &[]),
        "perm_action @scratch (t:wire mut) () () () ()",
        "d = qalloc any 1\ncx t d\ncx t d\nqfree d",
    );
    let r = request(
        &bytes,
        "scratch",
        &target(
            json!({"fast":{"capacity":8,"qubit_second":1,"dirty_rate":1}}),
            gates(),
        ),
    );
    let response = r.schedule().unwrap();
    let mut schedule = canonical::require_jcs(&response.schedule).unwrap();
    let entry = schedule["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["kind"] == "alloc")
        .unwrap();
    entry["alloc"]["class"] = json!("missing");
    let error = Document::parse(schedule)
        .unwrap()
        .validate_request(&r)
        .unwrap_err();
    assert!(error.reason.contains("undeclared target class"), "{error}");
}

#[test]
fn varying_callee_resource_contexts_are_not_silently_overwritten() {
    let flip = common::action(
        "perm_action",
        "flip",
        signature(&[1], &[], &[]),
        "perm_action @flip (t:wire mut) () () () ()",
        "x t",
    );
    let t = target(
        json!({"fast":{"capacity":8,"qubit_second":1,"dirty_rate":1}}),
        json!([
            {"op":"x","classes":["*"],"controls":{"min":0,"max":0},"epsilon":0,"cost":{"time":1}},
            {"op":"x","classes":["*"],"controls":{"min":1,"max":1},"epsilon":0,"cost":{"time":100}}
        ]),
    );
    let plain = "action_call @flip (t) () () ()";
    let controlled = "ctl c action_call @flip (t) () () ()";
    for (first, second) in [(plain, controlled), (controlled, plain), (plain, plain)] {
        let bytes = common::caller(
            &flip,
            "perm_action",
            "outer",
            signature(&[1], &[1], &[]),
            "perm_action @outer (t:wire mut) (c:wire in) () () ()",
            &format!("{first}\n{second}"),
        );
        let r = request(&bytes, "outer", &t);
        let response = r.schedule().unwrap();
        if first == second {
            assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
            let cert = canonical::require_jcs(&response.certificate).unwrap();
            let hash = r.object.as_ref().unwrap().closure_hash("flip").unwrap();
            assert_eq!(cert["regions"][hash]["resources"]["time"], "1");
            assert_eq!(
                cert["regions"][r.region.as_ref().unwrap()]["resources"]["time"],
                "2"
            );
        } else {
            assert_eq!(
                response.outcome,
                Outcome::Unsupported,
                "differing contexts cannot silently replace each other's scalar resource report"
            );
            assert!(
                response
                    .reason
                    .to_str()
                    .unwrap()
                    .contains("bennett-sched-1na")
            );
            assert!(response.schedule.is_empty() && response.certificate.is_empty());
        }
    }
}
