use crate::common;
use bennett_sched::{
    Outcome, Request, canonical,
    expansion::{Operation, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn source(body: &str) -> Vec<u8> {
    common::action(
        "perm_action",
        "a",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[2],"in":[2,1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @a (t:wire[2] mut) (c:wire[2] in, d:wire in) () () ()",
        body,
    )
}

#[test]
fn bundle_controls_preserve_written_bit_order_cost_arity_and_all_basis_values() {
    for (body, expected) in [
        (
            "mcx (c, d) t[0]",
            vec!["in:0[0]", "in:0[1]", "in:1[0]", "mut:0[0]"],
        ),
        (
            "mcx (d, c) t[0]",
            vec!["in:1[0]", "in:0[0]", "in:0[1]", "mut:0[0]"],
        ),
        ("mcx () t[0]", vec!["mut:0[0]"]),
    ] {
        let bytes = source(body);
        let object = Object::parse(&bytes).unwrap();
        let trace = Trace::build(&object, "a", 1, &json!([]), [0; 4]).unwrap();
        let gates = trace
            .events
            .iter()
            .filter_map(|e| match &e.leaf.op {
                Operation::Gate(g) => Some((e.leaf.clone(), g)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(gates.len(), 1);
        let (leaf, g) = &gates[0];
        assert_eq!(g.wires, expected);
        assert_eq!(g.own_controls(), expected.len() - 1);
        assert_eq!(g.controls, vec!["control:0"]);
        for input in 0..64 {
            let mut bits = BTreeMap::from([
                ("mut:0[0]".into(), input & 1 != 0),
                ("mut:0[1]".into(), input & 2 != 0),
                ("in:0[0]".into(), input & 4 != 0),
                ("in:0[1]".into(), input & 8 != 0),
                ("in:1[0]".into(), input & 16 != 0),
                ("control:0".into(), input & 32 != 0),
            ]);
            let before = bits.clone();
            simulate_basis(&[leaf.clone()], &mut bits).unwrap();
            let enabled = (expected.len() == 1 || input & 28 == 28) && input & 32 != 0;
            assert_eq!(bits["mut:0[0]"], before["mut:0[0]"] ^ enabled);
            for wire in ["mut:0[1]", "in:0[0]", "in:0[1]", "in:1[0]", "control:0"] {
                assert_eq!(bits[wire], before[wire]);
            }
        }
        let mut target: Value =
            serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
        target["gates"] = json!([{"op":"mcx","classes":["*"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":{"op":"+","args":[{"var":"k"},{"var":"c"}]}}}]);
        let mut request = Request::default();
        request.set_object(&bytes).unwrap();
        request
            .set_target(&canonical::to_vec(&target).unwrap())
            .unwrap();
        request
            .set_region(object.closure_hash("a").unwrap(), "forward", 1)
            .unwrap();
        request.set_params(b"[]").unwrap();
        request.budget = Some([0; 4]);
        request.set_accuracy("0").unwrap();
        let response = request.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let cert = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(
            cert["regions"][object.closure_hash("a").unwrap()]["resources"]["time"],
            (2 * (expected.len() - 1) + 1).to_string()
        );
    }
}

#[test]
fn mcx_rejects_repeated_overlapping_controls_and_nonscalar_targets() {
    for body in [
        "mcx (c, c) t[0]",
        "mcx (c, c[0]) t[0]",
        "mcx (t) t[0]",
        "mcx (c) t",
        "mcx () t",
    ] {
        let bytes = source(body);
        let result =
            Object::parse(&bytes).and_then(|o| Trace::build(&o, "a", 0, &json!([]), [0; 4]));
        assert!(result.is_err(), "ill-formed MCX accepted: {body}");
    }
}
