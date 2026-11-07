#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{
    Request, Status, canonical,
    expansion::{Leaf, Operation, inverse, simulate_basis},
    object::Object,
    schedule::{Document, Location, expansion_binding},
    source::Trace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
struct Names {
    owners: BTreeMap<String, String>,
    wires: BTreeMap<String, (String, u64)>,
    widths: BTreeMap<String, u64>,
}
impl Names {
    fn row(&mut self, leaf: &Leaf, location: &Location, complete: bool) -> Option<Value> {
        let point = location.path.last().unwrap();
        assert_eq!(leaf.ordinal, location.generated);
        assert_eq!(location.root, Default::default());
        assert!(location.path.iter().all(|p| p.entered_oracle.is_none()));
        let (op, owner, bit) = match &leaf.op {
            Operation::Alloc {
                identity, wires, ..
            } => {
                let n = self.owners.len();
                let owner = self
                    .owners
                    .entry(identity.clone())
                    .or_insert_with(|| format!("a{n}"))
                    .clone();
                self.widths.insert(owner.clone(), wires.len() as u64);
                for (bit, wire) in wires.iter().enumerate() {
                    self.wires.insert(wire.clone(), (owner.clone(), bit as u64));
                }
                ("alloc", owner, None)
            }
            Operation::Release { identity, .. } => ("release", self.owners[identity].clone(), None),
            Operation::Gate(g) => {
                if point.point.starts_with('$') {
                    assert!(g.controls.is_empty(), "K83 support is unconditional");
                }
                if !complete && !point.point.starts_with('$') {
                    return None;
                }
                assert_eq!(g.op, "x");
                assert!(g.control_wires().is_empty());
                let (owner, bit) = self.wires[&g.wires[0]].clone();
                ("x", owner, Some(bit))
            }
            Operation::Classical { .. } => panic!("classical event in K83 action"),
        };
        let caller = location
            .path
            .iter()
            .rev()
            .nth(1)
            .map(|p| json!(p.iteration))
            .unwrap_or(Value::Null);
        for p in location.path.iter().take(location.path.len() - 1) {
            assert_eq!(p.call_index, Some(0));
        }
        Some(json!([
            op,
            point.point,
            leaf.ordinal,
            owner,
            bit,
            point.iteration,
            point.instance,
            caller
        ]))
    }
}
fn manifest() -> Value {
    serde_json::from_slice(
        &std::fs::read(pins::spec("conformance/fixtures/accepted-k83-09").join("k83.json"))
            .unwrap(),
    )
    .unwrap()
}
fn reference(rows: &[Value], widths: &Value, bits: &mut BTreeMap<String, bool>) {
    for row in rows {
        let owner = row[3].as_str().unwrap();
        match row[0].as_str().unwrap() {
            "alloc" => {
                for bit in 0..widths[owner].as_u64().unwrap() {
                    assert!(bits.insert(format!("{owner}[{bit}]"), false).is_none());
                }
            }
            "release" => {
                for bit in 0..widths[owner].as_u64().unwrap() {
                    assert_eq!(bits.remove(&format!("{owner}[{bit}]")), Some(false));
                }
            }
            "x" => {
                let bit = bits
                    .get_mut(&format!("{owner}[{}]", row[4].as_u64().unwrap()))
                    .unwrap();
                *bit = !*bit;
            }
            _ => panic!("unknown published K83 operation"),
        }
    }
}

#[test]
fn every_published_k83_source_trace_and_external_basis_value_matches_including_raw_inverse() {
    let m = manifest();
    assert_eq!(m["ref_hash"], bennett_sched::schedule::EXPANSION_HASH);
    assert_eq!(m["cases"].as_array().unwrap().len(), 6);
    for case in m["cases"].as_array().unwrap() {
        let bytes = std::fs::read(
            pins::spec("conformance/fixtures/accepted-k83-09")
                .join(case["object"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(canonical::hash(&bytes), case["sha256"]);
        let object = Object::parse(&bytes).unwrap();
        let trace = Trace::build(
            &object,
            case["entry"].as_str().unwrap(),
            0,
            &json!([]),
            [0; 4],
        )
        .unwrap();
        let mut names = Names::default();
        let mut externals = Vec::new();
        assert_eq!(trace.slots.len(), case["actuals"].as_array().unwrap().len());
        for (slot, name) in trace.slots.iter().zip(case["actuals"].as_array().unwrap()) {
            for (bit, wire) in slot.wires.iter().enumerate() {
                let label = format!("{}[{bit}]", name.as_str().unwrap());
                names
                    .wires
                    .insert(wire.clone(), (name.as_str().unwrap().into(), bit as u64));
                externals.push((wire.clone(), label));
            }
        }
        let complete = case["complete_trace"].as_bool().unwrap();
        let rows = trace
            .events
            .iter()
            .filter_map(|e| names.row(&e.leaf, &e.location, complete))
            .collect::<Vec<_>>();
        assert_eq!(json!(rows), case["trace"], "{}", case["name"]);
        assert_eq!(json!(names.widths), case["allocation_widths"]);
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let reversed = inverse(&leaves);
        let actual_inverse = reversed
            .iter()
            .zip(trace.events.iter().rev())
            .filter_map(|(leaf, event)| names.row(leaf, &event.location, complete))
            .collect::<Vec<_>>();
        let expected_inverse = case["trace"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|row| {
                let mut row = row.clone();
                row[0] = match row[0].as_str().unwrap() {
                    "alloc" => json!("release"),
                    "release" => json!("alloc"),
                    _ => row[0].clone(),
                };
                row
            })
            .collect::<Vec<_>>();
        assert_eq!(actual_inverse, expected_inverse);
        for tuple in 0..1usize << externals.len() {
            let before = externals
                .iter()
                .enumerate()
                .map(|(i, (wire, _))| (wire.clone(), tuple & (1 << i) != 0))
                .collect::<BTreeMap<_, _>>();
            let mut bits = before.clone();
            let mut expected = externals
                .iter()
                .enumerate()
                .map(|(i, (_, label))| (label.clone(), tuple & (1 << i) != 0))
                .collect();
            reference(
                case["trace"].as_array().unwrap(),
                &case["allocation_widths"],
                &mut expected,
            );
            if !complete {
                // This manifest intentionally lists only support. The printed
                // qif arms contain zero-trip loops; the final `x t` still runs.
                assert_eq!(case["name"], "qif-arms");
                let body = &object.functions[case["entry"].as_str().unwrap()].body;
                assert_eq!(body.last().unwrap().op, "x");
                assert_eq!(body.last().unwrap().operands, vec!["t"]);
                *expected.get_mut("t[0]").unwrap() ^= true;
            }
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(bits.len(), externals.len());
            for (wire, label) in &externals {
                assert_eq!(bits[wire], expected[label], "{} tuple{tuple}", case["name"]);
            }
            simulate_basis(&reversed, &mut bits).unwrap();
            assert_eq!(bits, before);
        }
    }
}

#[test]
fn published_k83_schedule_polarities_hold_after_explicit_library_and_target_rebinding() {
    let m = manifest();
    let target = canonical::require_jcs(include_bytes!("../data/k83-target.json")).unwrap();
    let mut accepted = 0;
    let mut rejected = 0;
    for case in m["schedule_cases"].as_array().unwrap() {
        let bytes = std::fs::read(
            pins::spec("conformance/fixtures/accepted-k83-09")
                .join(case["object"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(canonical::hash(&bytes), case["sha256"]);
        let text = std::str::from_utf8(&bytes).unwrap();
        let (source, document) = text.split_once("\nsection schedule\n").unwrap();
        let mut value = canonical::require_jcs(document.trim_end_matches('\n').as_bytes()).unwrap();
        // These published portable objects omit a mandatory binding. Record
        // strict failure before adapting them; do not relax the §11 parser.
        let error = Document::parse(value.clone()).err().expect(
            "upstream K83 fixture format changed; remove the explicit missing-binding exception",
        );
        assert_eq!(error.status, Status::Arg);
        assert!(error.reason.contains("token_aliases"), "{error}");
        // Upgrade only the token-free equality binding and separately prove
        // that the original missing expansion binding still rejects.
        value["bindings"]["token_aliases"] = json!([]);
        let error = Document::parse(value.clone()).unwrap_err();
        assert!(error.reason.contains("expansion"), "{error}");
        let mut request = Request::default();
        request.set_object(source.as_bytes()).unwrap();
        request
            .set_target(&canonical::to_vec(&target).unwrap())
            .unwrap();
        request
            .set_region(
                value["bindings"]["closure_hash"].as_str().unwrap(),
                value["bindings"]["variant"].as_str().unwrap(),
                value["bindings"]["inherited_controls"].as_u64().unwrap() as u32,
            )
            .unwrap();
        request.set_params(b"[]").unwrap();
        request.budget = Some([0; 4]);
        request.set_accuracy("0").unwrap();
        // Explicitly upgrade the pre-K91 token-free portable template.
        value["bindings"]["token_aliases"] = json!([]);
        value["bindings"]["expansion"] = expansion_binding();
        value["bindings"]["target_hash"] = json!(request.target.as_ref().unwrap().hash);
        let result = Document::parse(value).unwrap().validate_request(&request);
        if case["accepted"] == true {
            result.unwrap_or_else(|e| panic!("{}: {e}", case["name"]));
            accepted += 1;
        } else {
            let error = result.err().expect("invalid K83 support schedule admitted");
            assert_eq!(
                error.status,
                Status::ScheduleInfeasible,
                "{}: {error}",
                case["name"]
            );
            rejected += 1;
        }
    }
    assert_eq!((accepted, rejected), (2, 6));
}
