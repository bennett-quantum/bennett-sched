#[path = "support/pins.rs"]
mod pins;
use crate::common;
use bennett_sched::{
    Outcome, Request, canonical,
    expansion::{Operation, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::json;
use std::collections::BTreeMap;

fn example() -> Vec<u8> {
    let spec = std::fs::read_to_string(pins::spec("spec/Bennett-ISA-Spec-0.9.md")).unwrap();
    spec.split("**Example 3**")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n')
        .as_bytes()
        .to_vec()
}

#[test]
fn normative_translation_frame_is_universal_and_emits_a_separate_borrow_record() {
    let bytes = example();
    let object = Object::parse(&bytes).unwrap();
    for inherited in 0..=1 {
        let trace = Trace::build(&object, "borrow_demo", inherited, &json!([]), [0; 4]).unwrap();
        let borrowed = trace
            .events
            .iter()
            .find_map(|e| match &e.leaf.op {
                Operation::Alloc { wires, state, .. } if state == "borrowed" => {
                    Some(wires[0].clone())
                }
                _ => None,
            })
            .unwrap();
        let gates = trace
            .events
            .iter()
            .filter(|e| matches!(e.leaf.op, Operation::Gate(_)))
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for bits in 0..1u64 << (3 + inherited) {
            let mut state = BTreeMap::from([
                ("mut:0[0]".into(), bits & 1 != 0),
                ("in:0[0]".into(), bits & 2 != 0),
                (borrowed.clone(), bits & 4 != 0),
            ]);
            if inherited > 0 {
                state.insert("control:0".into(), bits & 8 != 0);
            }
            let before = state.clone();
            simulate_basis(&gates, &mut state).unwrap();
            assert_eq!(state[&borrowed], before[&borrowed]);
            assert_eq!(state["in:0[0]"], before["in:0[0]"]);
            assert_eq!(
                state["mut:0[0]"],
                before["mut:0[0]"] ^ (before["in:0[0]"] && (inherited == 0 || before["control:0"]))
            );
        }
        let mut request = Request::default();
        request.set_object(&bytes).unwrap();
        request
            .set_region(
                object.closure_hash("borrow_demo").unwrap(),
                "forward",
                inherited,
            )
            .unwrap();
        request.budget = Some([0; 4]);
        request.set_params(b"[]").unwrap();
        request.set_accuracy("0").unwrap();
        let mut target: serde_json::Value = serde_json::from_slice(
            &std::fs::read(pins::spec(
                "conformance/fixtures/controlled-action-09/target.json",
            ))
            .unwrap(),
        )
        .unwrap();
        target["classes"]["fast"]["capacity"] = json!(4);
        target["gates"][0]["op"] = json!("cx");
        target["gates"][0]["controls"] = json!({"min":1,"max":2});
        request
            .set_target(&canonical::to_vec(&target).unwrap())
            .unwrap();
        let result = request.schedule().unwrap();
        assert_eq!(result.outcome, Outcome::Ok, "{:?}", result.reason);
        let c = canonical::parse(&result.certificate).unwrap();
        let region = &c["regions"][object.closure_hash("borrow_demo").unwrap()];
        assert_eq!(region["frame"]["borrow"].as_array().unwrap().len(), 1);
        assert_eq!(region["frame"]["scratch"], json!([]));
        let index = region["verification"]["borrow_frames"][0].as_u64().unwrap() as usize;
        assert_eq!(region["claims"][index]["rule"], "translation_lemma");
        assert_eq!(
            region["claims"][index]["witness"]["V"][0],
            json!([object.symbols["borrow_demo"].interface_hash, "entry", 2])
        );
        assert_eq!(
            region["claims"][index]["witness"]["X"]
                .as_array()
                .unwrap()
                .len(),
            1 + inherited as usize
        );
    }
}

#[test]
fn translation_lemma_never_certifies_any_of_the_four_normative_negative_patterns() {
    let bodies = [
        "b = qborrow any 1\ncx b t[0]\ncx t[0] t[1]\ncx b t[0]\nqreturn b",
        "b = qborrow any 2\ncx b[1] b[0]\ncx b[0] t[0]\ncx b[1] b[0]\ncx b[0] t[0]\nqreturn b",
        "b = qborrow any 1\nd = qalloc any 1\ncx b d\ncx d t[0]\ncx b d\ncx d t[0]\nqfree d\nqreturn b",
        "b = qborrow any 1\ncx x b\nswap t[0] t[1]\ncx b t[0]\ncx x b\nswap t[0] t[1]\ncx b t[0]\nqreturn b",
    ];
    for body in bodies {
        let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[2],"in":[1],"mem":[],"args":[],"bounds":[]});
        let source = common::action(
            "perm_action",
            "a",
            signature,
            "perm_action @a (t:wire[2] mut) (x:wire in) () () ()",
            body,
        );
        assert!(
            Trace::build(&Object::parse(&source).unwrap(), "a", 0, &json!([]), [0; 4]).is_err(),
            "{body}"
        );
    }
}
