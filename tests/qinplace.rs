use bennett_sched::{
    canonical,
    expansion::{Operation, inverse, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "common/basis.rs"]
mod basis;
mod common;

fn oracles(width: u64, ins: bool) -> Vec<u8> {
    oracles_arguments(width, ins, false)
}
fn oracles_arguments(width: u64, ins: bool, extra: bool) -> Vec<u8> {
    let in_widths = if ins { vec![width, width] } else { vec![width] };
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":in_widths,"tgt":width,"fault":4,"mem":[],"args":if extra{vec!["i128","f64","token","ptr"]}else{vec![]}});
    let inputs = if ins {
        format!("x:wire[{width}] in, y:wire[{width}] in")
    } else {
        format!("x:wire[{width}] in")
    };
    let header = |name| {
        format!(
            "oracle @{name} ({inputs}) (t:wire[{width}]) (f:wire[4]) () ({})\nregs {{ one:reg[{width}], r:reg[{width}] }}",
            if extra {
                "big:i128, angle:f64, other:token, addr:ptr"
            } else {
                ""
            }
        )
    };
    let body = |op| {
        if ins {
            format!("r = lift.{op} x y\nlift.retval r")
        } else {
            format!("one = lift.const.i{width} 1\nr = lift.{op} x one\nlift.retval r")
        }
    };
    let g = common::action("oracle", "g", sig.clone(), &header("g"), &body("add"));
    common::caller(&g, "oracle", "h", sig, &header("h"), &body("sub"))
}
fn signature(width: u64, ins: bool) -> Value {
    json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[width],"in":if ins{vec![width]}else{vec![]},"mem":[],"args":["token"],"bounds":[]})
}
fn header(name: &str, width: u64, ins: bool) -> String {
    format!(
        "perm_action @{name} (x:wire[{width}] mut) ({}) () (b:token) ()",
        if ins {
            format!("y:wire[{width}] in")
        } else {
            String::new()
        }
    )
}
fn action(width: u64, ins: bool, op: &str) -> Vec<u8> {
    common::caller(
        &oracles(width, ins),
        "perm_action",
        "p",
        signature(width, ins),
        &header("p", width, ins),
        op,
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

#[test]
fn qinplace_and_source_adjoints_keep_private_order_call_occurrences_and_clean_full_domains() {
    for width in [1, 2] {
        for ins in [false, true] {
            for orientation in ["", "adj ", "adj adj "] {
                let op = if ins {
                    "qinplace @g @h b x (y) () ()"
                } else {
                    "qinplace @g @h b x"
                };
                let p = action(width, ins, op);
                let args = if ins { "(y)" } else { "()" };
                let bytes = common::caller(
                    &p,
                    "perm_action",
                    "outer",
                    signature(width, ins),
                    &header("outer", width, ins),
                    &format!("{orientation}action_call @p (x) {args} () (b)"),
                );
                let object = Object::parse(&bytes).unwrap();
                let b = if ins { 2 } else { 3 };
                let trace = Trace::build(
                    &object,
                    "outer",
                    1,
                    &json!([[b.to_string(), "0", "0", "0"]]),
                    [0; 4],
                )
                .unwrap();
                let owner = trace
                    .invocations
                    .iter()
                    .position(|i| i.name == "p")
                    .unwrap();
                let private = trace
                    .events
                    .iter()
                    .filter(|e| e.owner == owner)
                    .collect::<Vec<_>>();
                assert_eq!(private.len(), width as usize + 4);
                let widths = private
                    .iter()
                    .filter_map(|e| match &e.leaf.op {
                        Operation::Alloc { wires, .. } | Operation::Release { wires, .. } => {
                            Some(wires.len())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(widths, vec![width as usize, 4, width as usize, 4]);
                assert_eq!(
                    private
                        .iter()
                        .map(|e| e.location.generated)
                        .collect::<Vec<_>>(),
                    (0..width + 4).collect::<Vec<_>>()
                );
                let mut calls = trace
                    .invocations
                    .iter()
                    .filter(|i| i.parent == Some(owner))
                    .collect::<Vec<_>>();
                calls.sort_by_key(|i| i.start);
                let expected = if orientation == "adj " {
                    vec!["h", "g"]
                } else {
                    vec!["g", "h"]
                };
                assert_eq!(
                    calls.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(
                    calls
                        .iter()
                        .map(|i| i.path.last().unwrap().call_index)
                        .collect::<Vec<_>>(),
                    vec![Some(0), Some(1)]
                );
                let leaves = trace
                    .events
                    .iter()
                    .map(|e| e.leaf.clone())
                    .collect::<Vec<_>>();
                let raw = inverse(&leaves);
                let raw_widths = raw
                    .iter()
                    .filter_map(|l| match &l.op {
                        Operation::Alloc {
                            identity, wires, ..
                        }
                        | Operation::Release {
                            identity, wires, ..
                        } if identity.starts_with("source-qinplace/") => Some(wires.len()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    raw_widths,
                    vec![4, width as usize, 4, width as usize],
                    "raw inverse retains the forward trace's reversed lifetime sequence"
                );
                for x in 0..1 << width {
                    for y in 0..if ins { 1 << width } else { 1 } {
                        for control in [false, true] {
                            let mut bits = BTreeMap::new();
                            put(&mut bits, "mut:0", width, x);
                            if ins {
                                put(&mut bits, "in:0", width, y);
                            }
                            bits.insert("control:0".into(), control);
                            let before = bits.clone();
                            simulate_basis(&leaves, &mut bits).unwrap();
                            let delta = if ins { y } else { 1 };
                            let modulus = 1 << width;
                            let expected = if !control {
                                x
                            } else if orientation == "adj " {
                                (x + modulus - delta) % modulus
                            } else {
                                (x + delta) % modulus
                            };
                            assert_eq!(get(&bits, "mut:0", width), expected);
                            if ins {
                                assert_eq!(get(&bits, "in:0", width), y);
                            }
                            assert_eq!(bits.len(), before.len());
                            simulate_basis(&inverse(&leaves), &mut bits).unwrap();
                            assert_eq!(bits, before);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn qinplace_transfer_preserves_input_versions_only_after_the_inverse_restores_x() {
    for restore in [false, true] {
        let inverse = if restore {
            "adj qinplace @g @h b x\n"
        } else {
            ""
        };
        let bytes = action(
            2,
            false,
            &format!("d = qcopy x\nqinplace @g @h b x\n{inverse}quncopy x d"),
        );
        let object = Object::parse(&bytes).unwrap();
        let result = Trace::build(&object, "p", 0, &json!([["3", "0", "0", "0"]]), [0; 4]);
        if restore {
            let trace = result.unwrap();
            let leaves = trace
                .events
                .iter()
                .map(|e| e.leaf.clone())
                .collect::<Vec<_>>();
            for x in 0..4 {
                let mut bits = BTreeMap::new();
                put(&mut bits, "mut:0", 2, x);
                simulate_basis(&leaves, &mut bits).unwrap();
                assert_eq!(get(&bits, "mut:0", 2), x);
                assert_eq!(bits.len(), 2);
            }
        } else {
            assert!(
                result.is_err(),
                "qinplace must not preserve the old input version while x has changed"
            );
        }
    }
}

#[test]
fn qinplace_typed_arguments_use_singleton_bit_domains_and_exact_nested_token_tuples() {
    use bennett_sched::{Outcome, Request};
    let mut sig = signature(2, false);
    sig["args"] = json!(["token", "i128", "f64", "token", "ptr"]);
    let bytes = common::caller(
        &oracles_arguments(2, false, true),
        "perm_action",
        "p",
        sig,
        "perm_action @p (x:wire[2] mut) () () (b:token, big:i128, angle:f64, other:token, addr:ptr) ()",
        "qinplace @g @h b x () () (big, angle, other, addr)",
    );
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(include_bytes!("../data/k83-target.json"))
        .unwrap();
    let hash = request
        .object
        .as_ref()
        .unwrap()
        .closure_hash("p")
        .unwrap()
        .to_owned();
    request.set_region(&hash, "forward", 0).unwrap();
    request.set_params(br#"[["3","0","0","0"],"18446744073709551617","0x8000000000000000",["2","0","0","0"],"4294967295"]"#).unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let cert = canonical::require_jcs(&response.certificate).unwrap();
    let region = &cert["regions"][&hash];
    let entry = &region["verification"]["inverse_pairs"][0];
    assert_eq!(entry["budget"], json!(["3", "0", "0", "0"]));
    assert_eq!(entry["arg_tokens"], json!({"other":["2","0","0","0"]}));
    let domain = &region["claims"][entry["claim"].as_u64().unwrap() as usize]["domain"]["args"];
    assert_eq!(
        domain,
        &json!([{"name":"big","width":128,"lo":"18446744073709551617","hi":"18446744073709551617"},{"name":"angle","width":64,"lo":"9223372036854775808","hi":"9223372036854775808"},{"name":"addr","width":32,"lo":"4294967295","hi":"4294967295"}])
    );
}

#[test]
fn qinplace_rejects_aliases_read_only_targets_and_missing_argument_lists() {
    for op in [
        "qinplace @g @h b x",
        "qinplace @g @h b x (x) () ()",
        "qinplace @g @h b y (x) () ()",
        "ctl x[0] qinplace @g @h b x (y) () ()",
    ] {
        let object = Object::parse(&action(2, true, op)).unwrap();
        let error = Trace::build(&object, "p", 0, &json!([["2", "0", "0", "0"]]), [0; 4])
            .err()
            .expect("bad qinplace accepted");
        assert_eq!(error.status, bennett_sched::Status::Arg, "{op}: {error}");
    }
}

#[test]
fn an_inverse_pair_domain_that_exceeds_host_tables_is_not_replaced_by_a_smaller_domain() {
    let object = Object::parse(&action(21, false, "qinplace @g @h b x")).unwrap();
    let error = Trace::build(&object, "p", 0, &json!([["3", "0", "0", "0"]]), [0; 4])
        .err()
        .expect("host proof domain silently truncated");
    assert_eq!(error.status, bennett_sched::Status::NoMem);
    assert!(error.reason.contains("complete domain"), "{error}");
}

fn memory_oracles() -> Vec<u8> {
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1,1],"tgt":1,"fault":4,"mem":[{"A":0,"W":8,"role":"in"}],"args":[]});
    let layout = json!({"ptr_width":3,"H":1,"A_tab":0,"S_cells":0,"null_size":1,"heap_base":2,"heap_limit_max":0,"globals":{},"interface_regions":{"g":{"db":{"base":1}},"h":{"db":{"base":1}}}});
    let jcs = |v: &Value| String::from_utf8(canonical::to_vec(v).unwrap()).unwrap();
    let mut symbols = json!({});
    let mut functions = Vec::new();
    for (index, (name, op)) in [("g", "add"), ("h", "sub")].into_iter().enumerate() {
        let function = format!(
            "oracle @{name} (x:wire in, y:wire in) (t:wire) (f:wire[4]) (db:region[0,8] in) ()\nregs {{ r:reg[1] }}\n^entry():\nr = lift.{op} x y\nlift.retval r"
        );
        let mut leaves = layout
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| !["globals", "interface_regions"].contains(&key.as_str()))
            .map(|(key, value)| (jcs(&json!([key])), jcs(value)))
            .collect::<Vec<_>>();
        leaves.push((
            jcs(&json!(["interface_regions", name, "db"])),
            jcs(&json!({"base":1})),
        ));
        leaves.sort();
        let suffix = leaves
            .iter()
            .map(|(path, value)| format!("layout {path} {value}\n"))
            .collect::<String>();
        let hash = canonical::hash(
            format!("isa=0.9\nptr_width=3\n{}\n{function}\n{suffix}", jcs(&sig)).as_bytes(),
        );
        symbols[index.to_string()] =
            json!({"name":name,"kind":"oracle","signature":sig,"interface_hash":hash});
        functions.push(function);
    }
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=3\n\nsection symbols\n{}\n\nsection functions\n{}\n\nsection layout\n{}\n",jcs(&symbols),functions.join("\n"),jcs(&layout)).into_bytes()
}

#[test]
fn qinplace_proof_and_controlled_execution_cover_every_interface_region_cell_value() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[{"A":0,"W":8,"role":"in"}],"args":["token"],"bounds":[]});
    let bytes = common::caller(
        &memory_oracles(),
        "perm_action",
        "p",
        sig,
        "perm_action @p (x:wire mut) (y:wire in) (db:region[0,8] in) (b:token) ()",
        "qinplace @g @h b x (y) (db) ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "p", 1, &json!([["2", "0", "0", "0"]]), [0; 4]).unwrap();
    assert_eq!(
        trace.qinplaces[0].domain["regions"],
        json!([{"name":"db","cells":"all"}])
    );
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let program = basis::Program::compile(&leaves).unwrap();
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for x in 0..2 {
        for y in 0..2 {
            for byte in 0..256 {
                for control in [false, true] {
                    let mut bits = BTreeMap::new();
                    put(&mut bits, "mut:0", 1, x);
                    put(&mut bits, "in:0", 1, y);
                    put(&mut bits, "mem:0", 8, byte);
                    bits.insert("control:0".into(), control);
                    cases.push(bits);
                    expected.push((x, y, byte, control));
                }
            }
        }
    }
    program.run_many(&mut cases).unwrap();
    for (bits, (x, y, byte, control)) in cases.iter().zip(expected) {
        assert_eq!(get(bits, "mut:0", 1), x ^ if control { y } else { 0 });
        assert_eq!(get(bits, "in:0", 1), y);
        assert_eq!(get(bits, "mem:0", 8), byte);
        assert_eq!(bits.len(), 11);
    }
}

#[test]
fn an_insufficient_budget_or_noninverse_pair_is_infeasible_without_discarding_inputs() {
    for (op, b) in [("qinplace @g @h b x", 2), ("qinplace @g @g b x", 3)] {
        let object = Object::parse(&action(2, false, op)).unwrap();
        let error = Trace::build(
            &object,
            "p",
            0,
            &json!([[b.to_string(), "0", "0", "0"]]),
            [0; 4],
        )
        .err()
        .expect("invalid inverse-pair obligation accepted");
        assert_eq!(
            error.status,
            bennett_sched::Status::ScheduleInfeasible,
            "{error}"
        );
        assert!(error.reason.contains("inverse"), "{error}");
    }
}

#[test]
fn qinplace_emits_a_bound_exhaustive_inverse_pair_entry_and_private_frames() {
    use bennett_sched::{Outcome, Request, schedule::Document};
    let bytes = action(2, false, "qinplace @g @h b x");
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    let gates=["x","cx","ccx","mcx","swap"].map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}));
    let target = json!({"ptr_widths_supported":[32],"classes":{"fast":{"capacity":"unbounded","qubit_second":1,"dirty_rate":1}},"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"gates":gates,"schemes":[],"qram":null,"tables":{},"imports":{},"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}});
    request
        .set_target(&canonical::to_vec(&target).unwrap())
        .unwrap();
    let hash = request
        .object
        .as_ref()
        .unwrap()
        .closure_hash("p")
        .unwrap()
        .to_owned();
    request.set_region(&hash, "forward", 1).unwrap();
    request.set_params(b"[[\"3\",\"0\",\"0\",\"0\"]]").unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    Document::parse(canonical::require_jcs(&response.schedule).unwrap())
        .unwrap()
        .validate_request(&request)
        .unwrap();
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    let region = &certificate["regions"][&hash];
    let entry = &region["verification"]["inverse_pairs"][0];
    assert_eq!(
        entry["g"],
        request.object.as_ref().unwrap().closure_hash("g").unwrap()
    );
    assert_eq!(
        entry["ginv"],
        request.object.as_ref().unwrap().closure_hash("h").unwrap()
    );
    let claim = &region["claims"][entry["claim"].as_u64().unwrap() as usize];
    assert_eq!(claim["kind"], "exhaustive");
    assert_eq!(claim["expected"], "inverse_pair");
    assert!(claim.get("invocation").is_none());
    assert_eq!(
        claim["domain"],
        json!({"bundles":[{"name":"x","width":2,"lo":0,"hi":"3"}],"borrowed":[],"regions":[],"args":[]})
    );
    assert_eq!(
        region["frame"]["scratch"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["width"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![2, 4]
    );
}

#[test]
fn an_adjoint_request_allocation_resolves_its_emitted_alloc_selector() {
    use bennett_sched::{
        Outcome, Request,
        schedule::{Document, Kind, resolve},
    };
    let p = action(1, false, "qinplace @g @h b x");
    let bytes = common::caller(
        &p,
        "perm_action",
        "outer",
        signature(1, false),
        &header("outer", 1, false),
        "adj action_call @p (x) () () (b)",
    );
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(include_bytes!("../data/k83-target.json"))
        .unwrap();
    let hash = request
        .object
        .as_ref()
        .unwrap()
        .closure_hash("outer")
        .unwrap()
        .to_owned();
    request.set_region(&hash, "forward", 1).unwrap();
    request.set_params(br#"[["3","0","0","0"]]"#).unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let document = Document::parse(canonical::require_jcs(&response.schedule).unwrap()).unwrap();
    let trace = Trace::build(
        request.object.as_ref().unwrap(),
        "outer",
        1,
        &json!([["3", "0", "0", "0"]]),
        [0; 4],
    )
    .unwrap();
    for event in &trace.events {
        if matches!(event.leaf.op, Operation::Alloc { .. }) {
            resolve(&document.entries,&event.location,Kind::Alloc).unwrap().expect("an actual allocation lost its emitted alloc decision after source adjoint formation");
        }
    }
}
