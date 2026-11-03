#[allow(dead_code)]
mod common;
use bennett_sched::object::Object;
#[test]
fn every_printed_09_example_object_has_its_published_interface_hash() {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let examples = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("## 17.")
        .next()
        .unwrap();
    let mut count = 0;
    for block in examples.split("```").skip(1).step_by(2) {
        let block = block.trim_start_matches('\n');
        if block.starts_with("bennett-object ") {
            count += 1;
            Object::parse(block.as_bytes())
                .unwrap_or_else(|e| panic!("normative object #{count}: {e}"));
        }
    }
    assert_eq!(count, 10, "contract example inventory changed");
}

#[test]
fn every_normative_quantum_region_has_an_audited_typed_scheduler_outcome() {
    use bennett_sched::{Outcome, Request, canonical};
    use serde_json::json;
    let gates=["x","cx","ccx","mcx","swap","h","s","sdg","t","tdg","cz"].iter().map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}})).collect::<Vec<_>>();
    let target = json!({"ptr_widths_supported":[32,64],"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"classes":{"fast":{"capacity":"unbounded","qubit_second":1,"dirty_rate":1}},"gates":gates,"schemes":[],"qram":null,"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":0},"objective":{"weighted":{"time":1}},"tables":{},"imports":{}});
    let target = canonical::to_vec(&target).unwrap();
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let text = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("## 17.")
        .next()
        .unwrap();
    let mut checked = 0;
    let mut ok = 0;
    let mut unsupported = 0;
    let mut host_limited = 0;
    for source in text
        .split("```")
        .skip(1)
        .step_by(2)
        .map(|s| s.trim_start_matches('\n'))
        .filter(|s| s.starts_with("bennett-object "))
    {
        let object = Object::parse(source.as_bytes()).unwrap();
        for f in object
            .functions
            .values()
            .filter(|f| ["oracle", "action", "perm_action"].contains(&f.kind.as_str()))
        {
            let arg_group = if f.kind == "oracle" { 4 } else { 3 };
            let args = f.groups[arg_group]
                .iter()
                .map(|p| {
                    if p.ty == "token" {
                        json!(["3", "0", "0", "0"])
                    } else if p.ty == "f64" {
                        json!("0x0000000000000000")
                    } else {
                        json!("0")
                    }
                })
                .collect::<Vec<_>>();
            let mut request = Request::default();
            request.set_object(source.as_bytes()).unwrap();
            request.set_target(&target).unwrap();
            request
                .set_region(object.closure_hash(&f.name).unwrap(), "forward", 0)
                .unwrap();
            request.budget = Some([3, 0, 0, 0]);
            request
                .set_params(&canonical::to_vec(&json!(args)).unwrap())
                .unwrap();
            request.set_accuracy("0").unwrap();
            let result = match common::schedule_distinct_tokens(&mut request) {
                Err(error) if f.name == "sum_db" => {
                    assert_eq!(error.status, bennett_sched::Status::NoMem);
                    assert!(error.reason.contains("host workspace"));
                    host_limited += 1;
                    checked += 1;
                    // Retain the positive-budget host-limit observation above;
                    // also exercise this exact object at B=0, including its full
                    // layout initialization, commit and release construction.
                    request.budget = Some([0, 0, 0, 0]);
                    let zero = common::schedule_distinct_tokens(&mut request).unwrap();
                    assert_eq!(zero.outcome, Outcome::Ok, "{:?}", zero.reason);
                    bennett_sched::schedule::Document::parse(
                        canonical::require_jcs(&zero.schedule).unwrap(),
                    )
                    .unwrap()
                    .validate_request(&request)
                    .unwrap();
                    canonical::require_jcs(&zero.certificate).unwrap();
                    continue;
                }
                Err(error) => panic!("{}: {error}", f.name),
                Ok(result) => result,
            };
            let postponed = ["g", "ginv", "addya", "through", "plus_db"].contains(&f.name.as_str());
            assert_eq!(
                result.outcome,
                if postponed {
                    Outcome::Unsupported
                } else {
                    Outcome::Ok
                },
                "{}: {:?}",
                f.name,
                result.reason
            );
            if postponed {
                assert!(
                    result
                        .reason
                        .to_str()
                        .unwrap()
                        .contains("bennett-sched-ark")
                );
                assert!(result.schedule.is_empty() && result.certificate.is_empty());
                unsupported += 1;
            } else {
                bennett_sched::schedule::Document::parse(
                    canonical::require_jcs(&result.schedule).unwrap(),
                )
                .unwrap()
                .validate_request(&request)
                .unwrap();
                canonical::require_jcs(&result.certificate).unwrap();
                ok += 1;
            }
            checked += 1;
        }
    }
    assert_eq!((checked, ok, unsupported, host_limited), (13, 7, 5, 1));
}
