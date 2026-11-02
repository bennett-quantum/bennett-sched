use bennett_sched::{canonical, expansion::simulate_basis, object::Object, source::Trace};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "common/basis.rs"]
mod basis;

fn encoded(body: &str) -> Vec<u8> {
    encoded_with_input(body, 2)
}
fn encoded_with_input(body: &str, input_width: u64) -> Vec<u8> {
    encoded_layout(body, input_width, false)
}
fn encoded_layout(body: &str, input_width: u64, zero_global: bool) -> Vec<u8> {
    let signature = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[input_width],"tgt":3,"fault":4,"mem":[],"args":[]});
    let function = format!(
        "oracle @work (x:wire[{input_width}] in) (t:wire[3]) (f:wire[4]) () ()\nregs {{ p:reg[3], q:reg[3] }}\n^entry():\n{body}"
    );
    let mut layout = json!({"ptr_width":3,"H":2,"A_tab":0,"S_cells":0,"null_size":1,"heap_base":1,"heap_limit_max":3,"globals":{},"interface_regions":{}});
    if zero_global {
        layout["A_tab"] = json!(1);
        layout["globals"] = json!({"zero":{"base":1,"size":0,"image":"zero"}});
    }
    let jcs = |v: &Value| String::from_utf8(canonical::to_vec(v).unwrap()).unwrap();
    let mut leaves = layout
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| !["globals", "interface_regions"].contains(&key.as_str()))
        .map(|(key, value)| (jcs(&json!([key])), jcs(value)))
        .collect::<Vec<_>>();
    if zero_global {
        leaves.push((
            jcs(&json!(["globals", "zero"])),
            jcs(&layout["globals"]["zero"]),
        ));
    }
    leaves.sort();
    let suffix = leaves
        .iter()
        .map(|(path, value)| format!("layout {path} {value}\n"))
        .collect::<String>();
    let image = if zero_global { "data zero \n" } else { "" };
    let hash = canonical::hash(
        format!(
            "isa=0.9\nptr_width=3\n{}\n{function}\n{image}{suffix}",
            jcs(&signature)
        )
        .as_bytes(),
    );
    let mut symbols =
        json!({"0":{"name":"work","kind":"oracle","signature":signature,"interface_hash":hash}});
    let data = if zero_global {
        let signature = json!({"abi":"0.9","tier":"-","kind":"data","size":0});
        let hash = canonical::hash(
            format!("isa=0.9\nptr_width=3\n{}\nzero \n", jcs(&signature)).as_bytes(),
        );
        symbols["1"] =
            json!({"name":"zero","kind":"data","signature":signature,"interface_hash":hash});
        "\nsection data\nzero \n"
    } else {
        ""
    };
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=3\n\nsection symbols\n{}\n\nsection functions\n{function}\n{data}\nsection layout\n{}\n",jcs(&symbols),jcs(&layout)).into_bytes()
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
fn check(object: &Object, budget: [u64; 4], expected: impl Fn(u64) -> (u64, u64)) {
    let trace = Trace::build(object, "work", 1, &json!([]), budget).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let program = basis::Program::compile(&leaves).unwrap();
    let mut cases = Vec::new();
    let mut inputs = Vec::new();
    for x in 0..4 {
        for t in 0..8 {
            for f in 0..16 {
                for c in [false, true] {
                    let mut bits = BTreeMap::new();
                    put(&mut bits, "in:0", 2, x);
                    put(&mut bits, "tgt:0", 3, t);
                    put(&mut bits, "fault:0", 4, f);
                    bits.insert("control:0".into(), c);
                    inputs.push((x, t, f, c));
                    cases.push(bits);
                }
            }
        }
    }
    let mut slow = cases[1023].clone();
    simulate_basis(&leaves, &mut slow).unwrap();
    program.run_many(&mut cases).unwrap();
    assert_eq!(slow, cases[1023]);
    for (bits, (x, t, f, c)) in cases.iter().zip(inputs) {
        let (payload, cause) = if c { expected(x) } else { (0, 0) };
        assert_eq!(get(bits, "in:0", 2), x);
        assert_eq!(
            get(bits, "tgt:0", 3),
            t ^ payload,
            "budget{budget:?},x{x},c{c}"
        );
        assert_eq!(
            get(bits, "fault:0", 4),
            f ^ cause,
            "budget{budget:?},x{x},c{c}"
        );
        assert_eq!(
            bits.len(),
            10,
            "every private state/history bit must be released clean"
        );
    }
}

#[test]
fn allocator_location_and_literal_lengths_cover_small_heap_budget_and_complete_interface_domains() {
    for length in ["x", "0", "1", "3"] {
        let object =
            Object::parse(&encoded(&format!("p = lift.alloc {length}\nlift.retval p"))).unwrap();
        for heap in 0..=3 {
            for b in 0..=3 {
                check(&object, [b, heap, 0, 0], |x| {
                    let n = if length == "x" {
                        x
                    } else {
                        length.parse().unwrap()
                    };
                    if b == 0 {
                        (0, 1)
                    } else if n.max(1) > heap {
                        (0, 2)
                    } else if b < 2 {
                        (0, 1)
                    } else {
                        (1, 0)
                    }
                });
            }
        }
    }
}

#[test]
fn free_restores_live_bits_and_double_free_or_exhausted_table_faults_at_the_first_bad_row() {
    for (body, terminal, fault) in [
        ("p = lift.alloc x\nlift.free p\nlift.retval p", 3, 0),
        (
            "p = lift.alloc x\nlift.free p\nlift.free p\nlift.retval p",
            3,
            7,
        ),
        (
            "p = lift.alloc x\nlift.free p\nq = lift.alloc x\nlift.retval q",
            3,
            2,
        ),
    ] {
        let object = Object::parse(&encoded(body)).unwrap();
        for b in 0..=4 {
            check(&object, [b, 3, 0, 0], |_| {
                if b < terminal {
                    (0, 1)
                } else if fault > 0 {
                    (0, fault)
                } else {
                    (1, 0)
                }
            });
        }
    }
}

#[test]
fn source_length_literals_must_fit_pointer_width_even_when_the_runtime_length_would_fault() {
    for instruction in [
        "p = lift.alloc 8",
        "lift.boundscheck p 8",
        "p = lift.alloc -1",
        "lift.boundscheck p -1",
    ] {
        let object = Object::parse(&encoded(&format!("{instruction}\nlift.retval p"))).unwrap();
        let error = Trace::build(&object, "work", 0, &json!([]), [2, 3, 0, 0])
            .err()
            .expect("out-of-range length literal admitted");
        assert_eq!(
            error.status,
            bennett_sched::Status::Arg,
            "{instruction}: {error}"
        );
        assert!(error.reason.contains("literal"), "{error}");
    }
}

#[test]
fn a_location_length_wider_than_the_pointer_is_legal_and_compared_without_wrap() {
    let object = Object::parse(&encoded_with_input("p = lift.alloc x\nlift.retval p", 4)).unwrap();
    let trace = Trace::build(&object, "work", 0, &json!([]), [2, 3, 0, 0]).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for length in 0..16 {
        let mut bits = BTreeMap::new();
        put(&mut bits, "in:0", 4, length);
        put(&mut bits, "tgt:0", 3, 5);
        put(&mut bits, "fault:0", 4, 9);
        simulate_basis(&leaves, &mut bits).unwrap();
        assert_eq!(get(&bits, "tgt:0", 3), 5 ^ if length <= 3 { 1 } else { 0 });
        assert_eq!(
            get(&bits, "fault:0", 4),
            9 ^ if length <= 3 { 0 } else { 2 }
        );
        assert_eq!(get(&bits, "in:0", 4), length);
        assert_eq!(bits.len(), 11);
    }
}

#[test]
fn free_of_a_heap_entry_preserves_a_pinned_zero_size_global_at_the_same_base() {
    let object = Object::parse(&encoded_layout(
        "p = lift.alloc 0\nlift.free p\nlift.boundscheck p 0\nlift.retval p",
        2,
        true,
    ))
    .unwrap();
    for b in 0..=4 {
        check(
            &object,
            [b, 1, 0, 0],
            |_| if b < 4 { (0, 1) } else { (1, 0) },
        );
    }
}

#[test]
fn encoded_allocator_request_emits_admitted_schedule_and_complete_history_certificate() {
    use bennett_sched::{Outcome, Request, schedule::Document};
    let bytes = encoded("p = lift.alloc x\nlift.free p\nlift.retval p");
    let gates=["x","cx","ccx","mcx","swap"].map(|op|json!({"op":op,"classes":["fast"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}}));
    let target = json!({"ptr_widths_supported":[3],"classes":{"fast":{"capacity":"unbounded","qubit_second":1,"dirty_rate":1}},"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"gates":gates,"schemes":[],"qram":null,"tables":{},"imports":{},"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}});
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(&canonical::to_vec(&target).unwrap())
        .unwrap();
    let hash = request
        .object
        .as_ref()
        .unwrap()
        .closure_hash("work")
        .unwrap()
        .to_owned();
    request.set_region(&hash, "forward", 0).unwrap();
    request.budget = Some([3, 1, 0, 0]);
    request.set_params(b"[]").unwrap();
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let schedule = canonical::require_jcs(&response.schedule).unwrap();
    Document::parse(schedule.clone())
        .unwrap()
        .validate_request(&request)
        .unwrap();
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(
        certificate["bindings"]["schedule_hash"],
        canonical::hash(&response.schedule)
    );
    let region = &certificate["regions"][&hash];
    assert_eq!(region["frame"]["history"].as_array().unwrap().len(), 3);
    assert!(
        region["frame"]["history"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["width"] == 21)
    );
    assert_eq!(
        region["error"]["forward:0"]["inclusive"]["operator_norm"],
        "0"
    );
    assert_eq!(
        certificate["bindings"]["invocation"]["budget"],
        json!(["3", "1", "0", "0"])
    );
}
