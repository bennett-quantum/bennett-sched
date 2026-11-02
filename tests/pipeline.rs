use bennett_sched::{Outcome, Request, canonical, decimal as d};
use serde_json::json;
mod common;

fn request() -> Request {
    let mut r = Request::default();
    r.set_object(
        &std::fs::read(
            "../bennett-isa-spec/conformance/fixtures/controlled-action-09/controlled.bisa",
        )
        .unwrap(),
    )
    .unwrap();
    r.set_target(
        &std::fs::read("../bennett-isa-spec/conformance/fixtures/controlled-action-09/target.json")
            .unwrap(),
    )
    .unwrap();
    r.set_region(
        "df131e972083b8d3b3b7d9341342b55252ca7a1963c4b7b6f702b9cdf3fbde12",
        "forward",
        0,
    )
    .unwrap();
    r.budget = Some([0; 4]);
    r.set_params(b"[]").unwrap();
    r.set_accuracy("0").unwrap();
    r
}

#[test]
fn schedules_real_controlled_fixture_and_emits_bound_exact_certificate() {
    let r = request();
    let response = r.schedule().unwrap();
    assert_eq!(
        response.outcome,
        Outcome::Ok,
        "{}",
        response.reason.to_str().unwrap()
    );
    let schedule = canonical::require_jcs(&response.schedule).unwrap();
    assert_eq!(schedule["entries"], json!([]));
    assert_eq!(schedule["bindings"]["expansion"]["name"], "ref");
    let cert = canonical::require_jcs(&response.certificate).unwrap();
    let root = &cert["regions"][schedule["bindings"]["closure_hash"].as_str().unwrap()];
    assert_eq!(root["resources"]["time"], "1");
    assert_eq!(root["resources"]["qubit_second[fast]"], "2");
    assert_eq!(root["resources"]["peak_qubits[fast]"], "2");
    assert_eq!(
        root["estimate"]["exact_resources"]["time"],
        json!({"numerator":"1","denominator":"1"})
    );
    assert_eq!(
        cert["bindings"]["schedule_hash"],
        canonical::hash(&response.schedule)
    );
    assert_eq!(
        cert["bindings"]["invocation"],
        json!({"args":[],"budget":["0","0","0","0"]})
    );
    assert_eq!(
        cert["composition"]["multiplicities"]
            .as_object()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        vec![json!(1), json!(1)]
    );
    assert_eq!(response.schedule, r.schedule().unwrap().schedule);
}

#[test]
fn infeasible_capacity_has_typed_outcome_reason_and_no_documents() {
    let mut r = request();
    r.set_capacity("fast", 1).unwrap();
    let s = r.schedule().unwrap();
    assert_eq!(s.outcome, Outcome::Infeasible);
    assert!(s.reason.to_str().unwrap().contains("capacity"));
    assert!(s.schedule.is_empty());
    assert!(s.certificate.is_empty());
}

#[test]
fn invalid_setters_are_atomic_and_incomplete_requests_return_status_errors() {
    let mut r = request();
    let target = r.target.as_ref().unwrap().hash.clone();
    assert!(r.set_target(b"{ \"bad\":0 }").is_err());
    assert_eq!(r.target.as_ref().unwrap().hash, target);
    assert!(r.set_accuracy("-0.1").is_err());
    assert_eq!(r.epsilon, Some(d::integer(0)));
    assert!(r.set_params(b"[0, 1]").is_err());
    assert!(Request::default().schedule().is_err());
}

#[test]
fn exact_target_scheme_is_expanded_once_and_bound_in_schedule() {
    let mut r = request();
    let mut t = r.target.as_ref().unwrap().value.clone();
    t["gates"][0]["op"] = json!("cx");
    t["schemes"] = json!([{"name":"ctrl-x","realises":{"op":"x","controls":{"min":1,"max":1}},"ancillae":{"count":0,"state":"clean","class":"fast"},"template":[{"gate":"cx","wires":["c[0]","t"]}]}]);
    r.set_target(&canonical::to_vec(&t).unwrap()).unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let s = canonical::require_jcs(&response.schedule).unwrap();
    assert_eq!(s["entries"][0]["gate"]["realisation"], "ctrl-x");
    let c = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(
        c["regions"][r.region.as_ref().unwrap()]["resources"]["time"],
        "1"
    );
}

#[test]
fn mandatory_oracle_state_exceeding_target_capacity_is_infeasible_before_expansion() {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let example = spec
        .split("**Example 2**")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n');
    let mut r = request();
    r.set_object(example.as_bytes()).unwrap();
    let hash = r
        .object
        .as_ref()
        .unwrap()
        .closure_hash("sum_db")
        .unwrap()
        .to_owned();
    r.set_region(&hash, "forward", 0).unwrap();
    r.budget = Some([3, 0, 0, 0]);
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Infeasible);
    assert!(
        response
            .reason
            .to_str()
            .unwrap()
            .contains("implicit oracle state")
    );
    assert!(response.schedule.is_empty());
    assert!(response.certificate.is_empty());
}

#[test]
fn context_varying_frame_classes_are_explicitly_deferred_instead_of_misreported() {
    let mut r = request();
    let flip = "7ff7153a2ad3b857ec9eb86ca99b9bcbc605f60093d28e99fb749ed16d13274b";
    let signature = json!({"abi":"0.9","args":[],"bounds":[],"in":[],"kind":"perm_action","mem":[],"mut":[1,1],"tier":"q"});
    let function = "perm_action @pair (t:wire mut, u:wire mut) () () () ()\n^entry():\naction_call @flip (t) () () ()\naction_call @flip (u) () () ()";
    let hash = canonical::hash(
        format!(
            "isa=0.9\nptr_width=32\n{}\n{}\n{flip}\n",
            String::from_utf8(canonical::to_vec(&signature).unwrap()).unwrap(),
            function.replace("@flip", &format!("@{flip}"))
        )
        .as_bytes(),
    );
    let flip_signature = json!({"abi":"0.9","args":[],"bounds":[],"in":[],"kind":"perm_action","mem":[],"mut":[1],"tier":"q"});
    let symbols = json!({"0":{"name":"pair","kind":"perm_action","signature":signature,"interface_hash":hash},"1":{"name":"flip","kind":"perm_action","signature":flip_signature,"interface_hash":flip}});
    let source = format!(
        "bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=32\n\nsection symbols\n{}\n\nsection functions\n{function}\nperm_action @flip (t:wire mut) () () () ()\n^entry():\nx t\n",
        String::from_utf8(canonical::to_vec(&symbols).unwrap()).unwrap()
    );
    r.set_object(source.as_bytes()).unwrap();
    r.set_region(&hash, "forward", 0).unwrap();
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["classes"] = json!({"A":{"capacity":1,"qubit_second":1,"dirty_rate":1},"B":{"capacity":1,"qubit_second":1,"dirty_rate":1}});
    target["gates"][0]["classes"] = json!(["*"]);
    target["gates"][0]["controls"] = json!({"min":0,"max":0});
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Unsupported);
    assert!(response.reason.to_str().unwrap().contains("frame"));
}

#[test]
fn instantaneous_native_gates_still_report_live_interface_peak() {
    let mut r = request();
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["gates"][0]["cost"]["time"] = json!(0);
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok);
    let c = canonical::parse(&response.certificate).unwrap();
    for region in c["regions"].as_object().unwrap().values() {
        assert_eq!(region["resources"]["peak_qubits[fast]"], "2");
        assert_eq!(region["resources"]["time"], "0");
        assert_eq!(region["resources"]["qubit_second[fast]"], "0");
    }
}

#[test]
fn nested_region_resources_include_its_native_scheme_ancillae() {
    let mut r = request();
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["classes"]["fast"]["capacity"] = json!(3);
    let mut cx = target["gates"][0].clone();
    cx["op"] = json!("cx");
    let mut ccx = cx.clone();
    ccx["op"] = json!("ccx");
    ccx["controls"] = json!({"min":2,"max":2});
    target["gates"] = json!([cx, ccx]);
    target["schemes"] = json!([{"name":"scratch-cx","realises":{"op":"x","controls":{"min":1,"max":1}},"ancillae":{"count":1,"state":"clean","class":"fast"},"template":[{"gate":"cx","wires":["c[0]","a[0]"]},{"gate":"ccx","wires":["c[0]","a[0]","t"]},{"gate":"cx","wires":["c[0]","a[0]"]}]}]);
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let c = canonical::parse(&response.certificate).unwrap();
    for region in c["regions"].as_object().unwrap().values() {
        assert_eq!(region["resources"]["peak_qubits[fast]"], "3");
        assert_eq!(region["resources"]["qubit_second[fast]"], "9");
        assert_eq!(region["resources"]["time"], "3");
    }
}

#[test]
fn approximate_scheme_charges_each_native_leaf_and_composes_its_error() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    let source = common::action(
        "action",
        "a",
        signature,
        "action @a (t:wire mut) () () () ()",
        "x t",
    );
    let mut r = request();
    r.set_object(&source).unwrap();
    let hash = r
        .object
        .as_ref()
        .unwrap()
        .closure_hash("a")
        .unwrap()
        .to_owned();
    r.set_region(&hash, "forward", 0).unwrap();
    r.set_accuracy("0.01").unwrap();
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["resources"]["gates"] = json!({"kind":"count","unit":"gate"});
    let gate = json!({"op":"h","classes":["fast"],"controls":{"min":0,"max":0},"epsilon":0.001,"cost":{"time":1,"gates":1}});
    let mut s = gate.clone();
    s["op"] = json!("s");
    target["gates"] = json!([gate, s]);
    target["schemes"] = json!([{"name":"hssh","realises":{"op":"x","controls":{"min":0,"max":0}},"ancillae":{"count":0,"state":"clean","class":"fast"},"template":[{"gate":"h","wires":["t"]},{"gate":"s","wires":["t"]},{"gate":"s","wires":["t"]},{"gate":"h","wires":["t"]}]}]);
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let c = canonical::parse(&response.certificate).unwrap();
    let region = &c["regions"][&hash];
    assert_eq!(region["resources"]["gates"], "4");
    assert_eq!(region["resources"]["time"], "4");
    assert_eq!(
        region["error"]["forward:0"]["inclusive"]["operator_norm"],
        "0.004"
    );
    assert_eq!(c["composition"]["epsilon_total"]["diamond"], "0.008");
    r.set_accuracy("0.0001").unwrap();
    assert_eq!(r.schedule().unwrap().outcome, Outcome::Infeasible);
}

#[test]
fn allocation_search_finds_a_usable_class_after_the_first_class_has_no_native_gate() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    let source = common::action(
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire mut) () () () ()",
        "x t",
    );
    let mut r = request();
    r.set_object(&source).unwrap();
    let hash = r
        .object
        .as_ref()
        .unwrap()
        .closure_hash("a")
        .unwrap()
        .to_owned();
    r.set_region(&hash, "forward", 0).unwrap();
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["classes"] = json!({"A":{"capacity":1,"qubit_second":1,"dirty_rate":1},"B":{"capacity":1,"qubit_second":1,"dirty_rate":1}});
    target["gates"][0]["classes"] = json!(["B"]);
    target["gates"][0]["controls"] = json!({"min":0,"max":0});
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let result = r.schedule().unwrap();
    assert_eq!(result.outcome, Outcome::Ok, "{:?}", result.reason);
    let c = canonical::parse(&result.certificate).unwrap();
    assert_eq!(c["regions"][&hash]["frame"]["slots"][0]["class"], "B");
}

#[test]
fn placement_search_finds_adjacent_vertices_when_the_lowest_free_pair_is_disconnected() {
    let mut r = request();
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["classes"]["fast"]["capacity"] = json!(3);
    target["topology"] =
        json!({"vertices":[{"class":"fast"},{"class":"fast"},{"class":"fast"}],"edges":[[1,2]]});
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let result = r.schedule().unwrap();
    assert_eq!(result.outcome, Outcome::Ok, "{:?}", result.reason);
    let s = canonical::parse(&result.schedule).unwrap();
    assert_eq!(s["interface_vertices"], json!([[1], [2]]));
}

#[test]
fn normative_inc8_schedules_through_the_abi_with_history_and_oracle_claim() {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let source = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n');
    let mut r = request();
    r.set_object(source.as_bytes()).unwrap();
    let hash = r
        .object
        .as_ref()
        .unwrap()
        .closure_hash("inc8")
        .unwrap()
        .to_owned();
    r.set_region(&hash, "forward", 0).unwrap();
    r.budget = Some([3, 0, 0, 0]);
    let mut target = r.target.as_ref().unwrap().value.clone();
    target["classes"]["fast"]["capacity"] = json!(1024);
    target["gates"]=json!(["x","cx","ccx","mcx","swap"].iter().map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":64},"epsilon":0,"cost":{"time":1}})).collect::<Vec<_>>());
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let result = r.schedule().unwrap();
    assert_eq!(result.outcome, Outcome::Ok, "{:?}", result.reason);
    let document =
        bennett_sched::schedule::Document::parse(canonical::parse(&result.schedule).unwrap())
            .unwrap();
    let trace = bennett_sched::source::Trace::build(
        r.object.as_ref().unwrap(),
        "inc8",
        0,
        &json!([]),
        [3, 0, 0, 0],
    )
    .unwrap();
    for event in &trace.events {
        bennett_sched::schedule::resolve(&document.entries, &event.location, event.location.kind)
            .unwrap();
    }
    let c = canonical::parse(&result.certificate).unwrap();
    let region = &c["regions"][&hash];
    assert_eq!(region["frame"]["history"].as_array().unwrap().len(), 3);
    assert!(
        region["frame"]["history"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["width"] == 13)
    );
    let claim = region["verification"]["oracle"].as_u64().unwrap() as usize;
    assert_eq!(region["claims"][claim]["expected"], "oracle");
    assert_eq!(
        region["claims"][claim]["invocation"]["budget"],
        json!(["3", "0", "0", "0"])
    );
    r.budget = Some([2, 0, 0, 0]);
    let result = r.schedule().unwrap();
    assert_eq!(result.outcome, Outcome::Ok);
    let c = canonical::parse(&result.certificate).unwrap();
    assert_eq!(
        c["regions"][&hash]["verification"]["fault_rate"]["faults_by_code"],
        json!({"1":1})
    );
}

#[test]
fn accuracy_allocation_handles_exact_source_gates_and_splits_a_scheme_native_error_budget() {
    for scheme in [false, true] {
        let sig = json!({"abi":"0.9","tier":"q","kind":"action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
        let source = common::action(
            "action",
            "a",
            sig,
            "action @a (t:wire mut) () () () ()",
            if scheme { "x t" } else { "x t\nh t\nh t" },
        );
        let mut r = request();
        r.set_object(&source).unwrap();
        let hash = r
            .object
            .as_ref()
            .unwrap()
            .closure_hash("a")
            .unwrap()
            .to_owned();
        r.set_region(&hash, "forward", 0).unwrap();
        r.set_accuracy(if scheme { "0.01" } else { "0.002" })
            .unwrap();
        let mut target = r.target.as_ref().unwrap().value.clone();
        let exact = json!({"op":"x","classes":["fast"],"controls":{"min":0,"max":0},"epsilon":0,"cost":{"time":1}});
        let h = json!({"op":"h","classes":["fast"],"controls":{"min":0,"max":0},"epsilon":if scheme{json!({"var":"eps"})}else{json!(0.001)},"cost":{"time":1}});
        if scheme {
            let mut s = h.clone();
            s["op"] = json!("s");
            target["gates"] = json!([h, s]);
            target["schemes"] = json!([{"name":"hssh","realises":{"op":"x","controls":{"min":0,"max":0}},"ancillae":{"count":0,"state":"clean","class":"fast"},"template":[{"gate":"h","wires":["t"]},{"gate":"s","wires":["t"]},{"gate":"s","wires":["t"]},{"gate":"h","wires":["t"]}]}]);
        } else {
            target["gates"] = json!([exact, h]);
        }
        r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
        let result = r.schedule().unwrap();
        assert_eq!(
            result.outcome,
            Outcome::Ok,
            "scheme={scheme} {:?}",
            result.reason
        );
        let c = canonical::parse(&result.certificate).unwrap();
        assert_eq!(
            c["regions"][&hash]["error"]["forward:0"]["inclusive"]["operator_norm"],
            if scheme { "0.01" } else { "0.002" }
        );
    }
}
