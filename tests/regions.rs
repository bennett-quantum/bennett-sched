#[path = "support/pins.rs"]
mod pins;
use crate::common;
use bennett_sched::{
    canonical,
    expansion::{Operation, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::json;
use std::collections::BTreeMap;

fn layout(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.extend_from_slice(b"\nsection layout\n{\"A_tab\":0,\"H\":0,\"S_cells\":0,\"globals\":{},\"heap_base\":1,\"heap_limit_max\":0,\"interface_regions\":{},\"null_size\":1,\"ptr_width\":32}\n");
    bytes
}

fn region_object(address: u64, cell: u64, image: Option<&str>) -> Vec<u8> {
    let image_suffix = if image.is_some() { " @image" } else { "" };
    region_program(
        &format!("x t\nqregion r {address} {cell}{image_suffix}\nx t"),
        &[],
        image,
    )
}

fn region_program(body: &str, bounds: &[u64], image: Option<&str>) -> Vec<u8> {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":bounds});
    let bound_line = if bounds.is_empty() {
        String::new()
    } else {
        format!(
            "regs {{  }} bounds {{{}}}\n",
            bounds
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let function =
        format!("perm_action @a (t:wire mut) () () () ()\n{bound_line}^entry():\n{body}");
    let mut symbols = json!({});
    let mut rewritten = function.clone();
    let mut suffix = String::new();
    let mut data = String::new();
    if let Some(hex) = image {
        let sig = json!({"abi":"0.9","tier":"-","kind":"data","size":hex.len()/2});
        let hash = canonical::hash(
            format!(
                "isa=0.9\nptr_width=32\n{}\nimage {hex}\n",
                String::from_utf8(canonical::to_vec(&sig).unwrap()).unwrap()
            )
            .as_bytes(),
        );
        symbols["1"] = json!({"name":"image","kind":"data","signature":sig,"interface_hash":hash});
        rewritten = rewritten.replace("@image", &format!("@{hash}"));
        suffix = format!("{hash}\n");
        data = format!("\nsection data\nimage {hex}\n");
    }
    let hash = canonical::hash(
        format!(
            "isa=0.9\nptr_width=32\n{}\n{rewritten}\n{suffix}",
            String::from_utf8(canonical::to_vec(&signature).unwrap()).unwrap()
        )
        .as_bytes(),
    );
    symbols["0"] =
        json!({"name":"a","kind":"perm_action","signature":signature,"interface_hash":hash});
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=32\n\nsection symbols\n{}\n\nsection functions\n{function}\n{data}",String::from_utf8(canonical::to_vec(&symbols).unwrap()).unwrap()).into_bytes()
}

#[test]
fn local_regions_prepare_at_function_entry_restore_at_exit_and_ignore_false_control_for_images() {
    for (address, cell, image) in [(0, 3, None), (0, 8, Some("81")), (3, 1, Some("a5"))] {
        let object = Object::parse(&region_object(address, cell, image)).unwrap();
        let trace = Trace::build(&object, "a", 1, &json!([]), [0; 4]).unwrap();
        let first = &trace.events[0];
        assert!(
            matches!(&first.leaf.op,Operation::Alloc{wires,..}if wires.len()==(1<<address)*cell as usize)
        );
        assert_eq!(first.location.path.last().unwrap().point, "$construct");
        assert!(matches!(
            trace.events.last().unwrap().leaf.op,
            Operation::Release { .. }
        ));
        let image_gates = trace
            .events
            .iter()
            .filter(|e| {
                ["$construct", "$release"].contains(&e.location.path.last().unwrap().point.as_str())
                    && matches!(e.leaf.op, Operation::Gate(_))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            image_gates.len(),
            image.map_or(0, |s| u8::from_str_radix(s, 16).unwrap().count_ones()
                as usize
                * 2)
        );
        for event in image_gates {
            let Operation::Gate(g) = &event.leaf.op else {
                panic!()
            };
            assert!(g.controls.is_empty());
        }
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for c in [false, true] {
            for t in [false, true] {
                let mut bits = BTreeMap::from([("control:0".into(), c), ("mut:0[0]".into(), t)]);
                simulate_basis(&leaves, &mut bits).unwrap();
                assert_eq!(
                    bits,
                    BTreeMap::from([("control:0".into(), c), ("mut:0[0]".into(), t)])
                );
            }
        }
    }
    let invalid = Object::parse(&region_object(0, 3, Some("81"))).unwrap();
    assert!(Trace::build(&invalid, "a", 0, &json!([]), [0; 4]).is_err());
}

#[test]
fn k83_nested_declarations_are_once_per_invocation_in_canonical_support_order() {
    let bytes = region_program(
        "qloop 0 {\nqregion r 0 8 @image\n}\nqloop 2 {\nqregion s 0 8 @image\nx t\nx t\n}\nqif t {\nqregion u 0 8 @image\n} {\nqregion v 0 8 @image\n}",
        &[0, 2],
        Some("81"),
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "a", 1, &json!([]), [0; 4]).unwrap();
    for (point, names, allocation) in [
        ("$construct", vec!["r", "s", "u", "v"], true),
        ("$release", vec!["v", "u", "s", "r"], false),
    ] {
        let support = trace
            .events
            .iter()
            .filter(|e| e.location.path.last().unwrap().point == point)
            .collect::<Vec<_>>();
        assert_eq!(support.len(), 12);
        for (ordinal, event) in support.iter().enumerate() {
            assert_eq!(event.location.generated, ordinal as u64);
            assert_eq!(event.leaf.ordinal, ordinal as u64);
            assert!(event.location.path.last().unwrap().iteration.is_empty());
            assert!(event.location.path.last().unwrap().instance.is_empty());
            assert_eq!(event.location.root, Default::default());
            if let Operation::Gate(g) = &event.leaf.op {
                assert!(g.controls.is_empty(), "images are unconditional");
            }
        }
        let ids = support
            .iter()
            .filter_map(|e| match &e.leaf.op {
                Operation::Alloc { identity, .. } if allocation => Some(identity),
                Operation::Release { identity, .. } if !allocation => Some(identity),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), names.len());
        for (id, name) in ids.iter().zip(names) {
            assert!(id.ends_with(&format!("/{name}")), "{id}");
        }
    }
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for control in [false, true] {
        for t in [false, true] {
            let before = BTreeMap::from([("control:0".into(), control), ("mut:0[0]".into(), t)]);
            let mut bits = before.clone();
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(bits, before);
            simulate_basis(&bennett_sched::expansion::inverse(&leaves), &mut bits).unwrap();
            assert_eq!(bits, before);
        }
    }
}

#[test]
fn k83_source_adjoints_have_forward_support_domains_and_raw_inverse_retains_addresses() {
    let callee = region_program("qloop 0 {\nqregion r 0 8 @image\n}\nx t", &[0], Some("81"));
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[2]});
    for orientation in ["", "adj ", "adj adj "] {
        let bytes = common::caller(
            &callee,
            "perm_action",
            "b",
            signature.clone(),
            "perm_action @b (t:wire mut) () () () ()",
            &format!("qloop 2 {{\n{orientation}action_call @a (t) () () ()\n}}"),
        );
        let object = Object::parse(&bytes).unwrap();
        let trace = Trace::build(&object, "b", 0, &json!([]), [0; 4]).unwrap();
        for inv in trace.invocations.iter().skip(1) {
            let events = &trace.events[inv.start..inv.end];
            assert_eq!(
                events.first().unwrap().location.path.last().unwrap().point,
                "$construct"
            );
            assert_eq!(
                events.last().unwrap().location.path.last().unwrap().point,
                "$release"
            );
            for point in ["$construct", "$release"] {
                let group = events
                    .iter()
                    .filter(|e| e.location.path.last().unwrap().point == point)
                    .collect::<Vec<_>>();
                assert_eq!(
                    group
                        .iter()
                        .map(|e| e.location.generated)
                        .collect::<Vec<_>>(),
                    vec![0, 1, 2]
                );
                for e in group {
                    assert_eq!(&e.location.path[..inv.path.len()], inv.path.as_slice());
                }
            }
        }
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let inverse = bennett_sched::expansion::inverse(&leaves);
        assert_eq!(
            inverse.iter().map(|l| l.ordinal).collect::<Vec<_>>(),
            leaves.iter().rev().map(|l| l.ordinal).collect::<Vec<_>>()
        );
        for t in [false, true] {
            let mut bits = BTreeMap::from([("mut:0[0]".into(), t)]);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(bits, BTreeMap::from([("mut:0[0]".into(), t)]));
        }
    }
}

#[test]
fn k83_request_support_selectors_check_exact_generated_domains_and_empty_points() {
    use bennett_sched::{Outcome, Request, Status, schedule::Document};
    let gates = ["x","cx","ccx","mcx","swap"].map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}));
    let target = json!({"ptr_widths_supported":[32],"classes":{"fast":{"capacity":"unbounded","qubit_second":1,"dirty_rate":1}},"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"gates":gates,"schemes":[],"qram":null,"tables":{},"imports":{},"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}});
    for has_region in [false, true] {
        let bytes = if has_region {
            region_program("qloop 0 {\nqregion r 0 8 @image\n}", &[0], Some("81"))
        } else {
            region_program("x t", &[], None)
        };
        let mut request = Request::default();
        request.set_object(&bytes).unwrap();
        request
            .set_target(&canonical::to_vec(&target).unwrap())
            .unwrap();
        let hash = request
            .object
            .as_ref()
            .unwrap()
            .closure_hash("a")
            .unwrap()
            .to_owned();
        request.set_region(&hash, "forward", 1).unwrap();
        request.set_params(b"[]").unwrap();
        request.budget = Some([0; 4]);
        request.set_accuracy("0").unwrap();
        let response = request.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let value = canonical::require_jcs(&response.schedule).unwrap();
        Document::parse(value.clone())
            .unwrap()
            .validate_request(&request)
            .unwrap();
        if has_region {
            assert_eq!(value["entries"][0]["path"][0]["point"], "$construct");
            assert_eq!(value["entries"][0]["generated"], 0);
            let certificate = canonical::require_jcs(&response.certificate).unwrap();
            assert_eq!(
                certificate["regions"][&hash]["frame"]["scratch"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                certificate["regions"][&hash]["resources"]["peak_qubits[fast]"],
                "10"
            );
        }
        for point in ["$construct", "$release"] {
            let entry = json!({"path":[{"symbol":hash,"point":point,"iteration":[],"instance":[]}],"step":"*","phase":"*","generated":"*","kind":"gate","gate":{"realisation":"native"}});
            let mut schedule = value.clone();
            schedule["entries"]
                .as_array_mut()
                .unwrap()
                .extend([entry.clone(), entry]);
            let result = Document::parse(schedule.clone())
                .unwrap()
                .validate_request(&request);
            if has_region {
                assert_eq!(result.unwrap_err().status, Status::ScheduleInfeasible);
            } else {
                result.unwrap();
            }
            // At the action support level neither a concrete step nor phase matches.
            let entries = schedule["entries"].as_array_mut().unwrap();
            let n = entries.len();
            for e in &mut entries[n - 2..] {
                e["step"] = json!(1);
                e["phase"] = json!("fwd");
            }
            Document::parse(schedule.clone())
                .unwrap()
                .validate_request(&request)
                .unwrap();
            schedule["entries"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["path"][0]["iteration"] = json!([0]);
            assert_eq!(
                Document::parse(schedule)
                    .unwrap()
                    .validate_request(&request)
                    .unwrap_err()
                    .status,
                Status::ScheduleInfeasible
            );
        }
    }
}

#[test]
fn interface_region_shape_role_and_cell_identity_are_preserved_across_calls() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[{"A":1,"W":3,"role":"in"}],"args":[],"bounds":[]});
    let base = layout(common::action(
        "perm_action",
        "a",
        signature.clone(),
        "perm_action @a (t:wire mut) () (r:region[1,3] in) () ()",
        "x t",
    ));
    let caller = common::caller(
        &base,
        "perm_action",
        "b",
        signature.clone(),
        "perm_action @b (t:wire mut) () (s:region[1,3] in) () ()",
        "action_call @a (t) () (s) ()",
    );
    let object = Object::parse(&caller).unwrap();
    let trace = Trace::build(&object, "b", 0, &json!([]), [0; 4]).unwrap();
    assert_eq!(trace.slots[1].id, "mem:0");
    assert_eq!(trace.slots[1].role, "mem_in");
    assert_eq!(trace.slots[1].wires.len(), 6);
    assert_eq!(trace.invocations[1].slots[1].wires, trace.slots[1].wires);
    let wrong = layout(common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut) () (r:region[0,6] in) () ()",
        "x t",
    ));
    assert!(
        Object::parse(&wrong)
            .and_then(|o| Trace::build(&o, "a", 0, &json!([]), [0; 4]))
            .is_err()
    );
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[{"A":0,"W":1,"role":"mut"}],"args":[],"bounds":[]});
    let wrong = layout(common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut) () (r:region[0,1] mut) () ()",
        "x r",
    ));
    assert!(Trace::build(&Object::parse(&wrong).unwrap(), "a", 0, &json!([]), [0; 4]).is_err());
}

#[test]
fn every_k79_source_address_vector_is_admitted_or_rejected_before_ref_helper_expansion() {
    let manifest: serde_json::Value = canonical::parse(
        &std::fs::read(pins::spec(
            "conformance/fixtures/accepted-k78-k80-09/k79.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let mut admitted = 0;
    let mut rejected = 0;
    for vector in manifest["vectors"].as_array().unwrap() {
        let source = vector["source"].as_str().unwrap();
        let symbols: serde_json::Value = canonical::parse(
            source
                .split("section symbols\n")
                .nth(1)
                .unwrap()
                .split("\n\n")
                .next()
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        let mut signature = symbols["0"]["signature"].clone();
        signature["abi"] = json!("0.9");
        let function = source
            .split("section functions\n")
            .nth(1)
            .unwrap()
            .split("\n\nsection")
            .next()
            .unwrap();
        let (header, rest) = function.split_once('\n').unwrap();
        let body = rest
            .strip_prefix("^entry():\n")
            .unwrap()
            .trim_end_matches('\n');
        let object = Object::parse(&layout(common::action(
            "perm_action",
            "m",
            signature,
            header,
            body,
        )))
        .unwrap();
        let result = Trace::build(&object, "m", 1, &json!([]), [0; 4]);
        if vector["source_admission"] == true {
            let trace = result.unwrap_or_else(|e| panic!("{}: {e}", vector["operation"]));
            let leaves = trace
                .events
                .iter()
                .map(|e| e.leaf.clone())
                .collect::<Vec<_>>();
            let names = ["mut:0[0]", "in:0[0]", "in:0[1]", "control:0", "mem:0[0]"];
            for tuple in vector["tuples"].as_array().unwrap() {
                let before = tuple["before"].as_u64().unwrap();
                let mut bits = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (n.to_string(), before & (1 << i) != 0))
                    .collect::<BTreeMap<_, _>>();
                simulate_basis(&leaves, &mut bits).unwrap();
                let packed = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| u64::from(bits[*n]) << i)
                    .sum::<u64>();
                assert_eq!(
                    packed,
                    tuple["after"].as_u64().unwrap(),
                    "{} {before}",
                    vector["operation"]
                );
                assert_eq!(bits.len() as u64, tuple["live_wires"].as_u64().unwrap());
                simulate_basis(&bennett_sched::expansion::inverse(&leaves), &mut bits).unwrap();
                let packed = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| u64::from(bits[*n]) << i)
                    .sum::<u64>();
                assert_eq!(packed, tuple["inverse_after"].as_u64().unwrap());
            }
            admitted += 1;
        } else {
            assert_eq!(
                result.unwrap_err().status,
                bennett_sched::Status::Arg,
                "{} {}",
                vector["operation"],
                vector["address_actual"]
            );
            rejected += 1;
        }
    }
    assert_eq!(admitted, 4);
    assert_eq!(rejected, 8);
}

#[test]
fn a_region_cell_write_closes_a_load_window_even_when_the_cell_is_restored() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[{"A":0,"W":1,"role":"mut"}],"args":[],"bounds":[]});
    let source = layout(common::action(
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire mut) () (r:region[0,1] mut) () ()",
        "d = qload r ()\nqstore r () t\nqstore r () t\nqunload r () d",
    ));
    let error =
        Trace::build(&Object::parse(&source).unwrap(), "a", 0, &json!([]), [0; 4]).unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::Arg);
    assert!(
        error.reason.contains("cell") && error.reason.contains("window"),
        "{error}"
    );
}
