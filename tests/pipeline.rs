use bennett_sched::{Outcome, Request, canonical, decimal as d};
use serde_json::json;

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
fn postponed_oracle_has_a_specific_bead_reason_and_no_feasible_document() {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let example = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n');
    let mut r = request();
    r.set_object(example.as_bytes()).unwrap();
    r.set_region(
        "ed9865300b85a519c2379de98e36e635e72b5b78d8bdfca03dccf9a09aa3df5e",
        "forward",
        0,
    )
    .unwrap();
    r.budget = Some([3, 0, 0, 0]);
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Unsupported);
    assert!(
        response
            .reason
            .to_str()
            .unwrap()
            .contains("bennett-sched-ark")
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
