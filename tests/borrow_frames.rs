use crate::common;
use bennett_sched::{Outcome, Request, Status, canonical, object::Object, source::Trace};
use serde_json::{Value, json};

fn source(prefix: &str, frame: &str) -> Vec<u8> {
    let bounds = if frame.starts_with("qloop 2 {") {
        vec![2]
    } else {
        vec![]
    };
    common::action(
        "perm_action",
        "a",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1,1],"mem":[],"args":[],"bounds":bounds}),
        "perm_action @a (t:wire mut) (x:wire in, y:wire in) () () ()",
        &format!("c = qalloc any 1\n{prefix}b = qborrow any 1\n{frame}\nqreturn b\nqfree c"),
    )
}
fn schedule(bytes: &[u8], inherited: u32) -> Value {
    let object = Object::parse(bytes).unwrap();
    let mut r = Request::default();
    r.set_object(bytes).unwrap();
    r.set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    r.set_region(object.closure_hash("a").unwrap(), "forward", inherited)
        .unwrap();
    r.set_params(b"[]").unwrap();
    r.set_accuracy("0").unwrap();
    r.budget = Some([0; 4]);
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    canonical::require_jcs(&response.certificate).unwrap()["regions"]
        [object.closure_hash("a").unwrap()]
    .clone()
}

#[test]
fn clean_workspace_frame_and_causal_x_family_prefix_emit_bound_exhaustive_relations() {
    for (prefix, point) in [
        ("", 2),
        ("cx x c\nmcx (x) c\n", 4),
        ("qxor x c\nqxor x c\n", 4),
    ] {
        let bytes = source(prefix, "cx c b");
        let object = Object::parse(&bytes).unwrap();
        for inherited in [0, 1] {
            let record = schedule(&bytes, inherited);
            let index = record["verification"]["borrow_frames"][0].as_u64().unwrap() as usize;
            let claim = &record["claims"][index];
            assert_eq!(claim["kind"], "exhaustive");
            assert_eq!(claim["expected"], "borrow_frame");
            assert_eq!(claim["borrow_frame"]["lifetime"], 0);
            assert_eq!(claim["borrow_frame"]["context_key"], json!([]));
            let occurrence = |p| json!({"context_key":[],"symbol":object.symbols["a"].interface_hash,"point":format!("entry:{p}"),"iteration":[],"instance":[]});
            assert_eq!(claim["borrow_frame"]["first"], occurrence(point));
            assert_eq!(claim["borrow_frame"]["last"], occurrence(point));
            assert_eq!(
                claim["domain"]["workspace"],
                json!([{"allocation":{"source":occurrence(0),"allocation_ordinal":0},"offset":0,"width":1,"lo":0,"hi":0}])
            );
            assert_eq!(
                claim["domain"]["borrowed"],
                json!([{"name":"borrowed:0","width":1,"lo":0,"hi":1}])
            );
            assert_eq!(
                claim["domain"]["bundles"].as_array().unwrap().len(),
                3 + inherited as usize
            );
            for b in claim["domain"]["bundles"].as_array().unwrap() {
                assert_eq!(b["lo"], 0);
                assert_eq!(b["hi"], 1);
            }
            assert_eq!(claim["subject"]["inherited_controls"], inherited);
            assert!(claim["subject"]["schedule_hash"].is_string());
        }
    }
}

#[test]
fn polynomial_or_reader_interleaved_prefix_never_supplies_a_false_workspace_constant() {
    for prefix in [
        "cx x c\ncx y c\ncx x c\ncx y c\n",
        "cx x c\ncx c t\ncx x c\n",
    ] {
        let bytes = source(prefix, "cx c b");
        let object = Object::parse(&bytes).unwrap();
        let e = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap_err();
        assert_eq!(e.status, Status::ScheduleInfeasible, "{e}");
        assert!(e.reason.contains("borrow frame"), "{e}");
    }
}

#[test]
fn nontranslation_identity_replays_complete_borrowed_fibers_and_rejects_a_changed_borrower() {
    let record = schedule(&source("", "cx b t\ncx b t"), 0);
    let i = record["verification"]["borrow_frames"][0].as_u64().unwrap() as usize;
    assert_eq!(record["claims"][i]["expected"], "borrow_frame");
    for frame in ["cx b t", "cx t b"] {
        let bytes = source("", frame);
        let object = Object::parse(&bytes).unwrap();
        let e = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap_err();
        assert_eq!(e.status, Status::ScheduleInfeasible, "{e}");
    }
}

#[test]
fn repeated_visits_use_the_complete_union_in_normative_visit_order() {
    let bytes = source("", "qloop 2 {\ncx b t\ncx b t\nx c\n}");
    let record = schedule(&bytes, 0);
    let indices = record["verification"]["borrow_frames"].as_array().unwrap();
    assert_eq!(indices.len(), 2);
    for index in indices {
        let claim = &record["claims"][index.as_u64().unwrap() as usize];
        let products = claim["domain"]["entry_products"].as_array().unwrap();
        assert_eq!(products.len(), 2);
        for (i, product) in products.iter().enumerate() {
            assert_eq!(product["workspace"][0]["lo"], i);
            assert_eq!(product["workspace"][0]["hi"], i);
        }
    }
}

#[test]
fn entry_products_follow_visit_order_after_decimal_iteration_nine() {
    let counter = common::action(
        "perm_action",
        "increment",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[4],"in":[],"mem":[],"args":[],"bounds":[]}),
        "perm_action @increment (c:wire[4] mut) () () () ()",
        "mcx (c[0], c[1], c[2]) c[3]\nccx c[0] c[1] c[2]\ncx c[0] c[1]\nx c[0]",
    );
    let bytes = common::caller(
        &counter,
        "perm_action",
        "a",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1,1],"mem":[],"args":[],"bounds":[16,16]}),
        "perm_action @a (t:wire mut) (x:wire in, y:wire in) () () ()",
        "c = qalloc any 4\nb = qborrow any 1\nqloop 16 {\ncx b t\ncx b t\naction_call @increment (c) () () ()\n}\nqreturn b\nqloop 16 {\nadj action_call @increment (c) () () ()\n}\nqfree c",
    );
    let record = schedule(&bytes, 0);
    assert_eq!(
        record["verification"]["borrow_frames"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    let claim =
        &record["claims"][record["verification"]["borrow_frames"][0].as_u64().unwrap() as usize];
    let products = claim["domain"]["entry_products"].as_array().unwrap();
    assert_eq!(products.len(), 16);
    for (visit, product) in products.iter().enumerate() {
        for bit in 0..4 {
            assert_eq!(
                product["workspace"][bit]["lo"],
                (visit >> bit) & 1,
                "visit {visit}, bit {bit}"
            );
        }
    }
}

#[test]
fn selected_frame_certificate_metadata_is_admitted_by_the_pinned_reference() {
    #[path = "support/pins.rs"]
    mod pins;
    let bytes = source("", "cx c b");
    let object = Object::parse(&bytes).unwrap();
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    r.set_region(object.closure_hash("a").unwrap(), "forward", 0)
        .unwrap();
    r.set_params(b"[]").unwrap();
    r.set_accuracy("0").unwrap();
    r.budget = Some([0; 4]);
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let path = std::env::temp_dir().join(format!("borrow-frame-{}.bisa", std::process::id()));
    std::fs::write(
        &path,
        format!(
            "{}\nsection schedule\n{}\n\nsection certificate\n{}\n",
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(&response.schedule).unwrap(),
            std::str::from_utf8(&response.certificate).unwrap()
        ),
    )
    .unwrap();
    let result = std::process::Command::new(pins::reference())
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn sequential_lifetimes_keep_global_borrowed_positions_and_their_distinct_widths() {
    let bytes = source("", "cx c b\nqreturn b\nb = qborrow any 2\ncx c b[0]");
    let record = schedule(&bytes, 0);
    let indices = record["verification"]["borrow_frames"].as_array().unwrap();
    assert_eq!(indices.len(), 2);
    let mut lifetimes = Vec::new();
    for index in indices {
        let claim = &record["claims"][index.as_u64().unwrap() as usize];
        lifetimes.push(claim["borrow_frame"]["lifetime"].as_u64().unwrap());
        assert_eq!(
            claim["domain"]["borrowed"],
            json!([{"name":"borrowed:0","width":1,"lo":0,"hi":1},{"name":"borrowed:1","width":2,"lo":0,"hi":3}])
        );
    }
    lifetimes.sort();
    assert_eq!(lifetimes, vec![0, 1]);
}

#[test]
fn callee_borrow_lifetimes_precede_parent_lifetimes_in_the_complete_tuple() {
    let child = common::action(
        "perm_action",
        "idle",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[],"mem":[],"args":[],"bounds":[]}),
        "perm_action @idle () () () () ()",
        "b = qborrow any 1\nqreturn b",
    );
    let bytes = common::caller(
        &child,
        "perm_action",
        "a",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1,1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @a (t:wire mut) (x:wire in, y:wire in) () () ()",
        "c = qalloc any 1\naction_call @idle () () () ()\nb = qborrow any 2\ncx c b[0]\nqreturn b\nqfree c",
    );
    let record = schedule(&bytes, 0);
    let indices = record["verification"]["borrow_frames"].as_array().unwrap();
    assert_eq!(indices.len(), 1);
    let claim = &record["claims"][indices[0].as_u64().unwrap() as usize];
    assert_eq!(claim["borrow_frame"]["lifetime"], 1);
    assert_eq!(
        claim["domain"]["borrowed"],
        json!([{"name":"borrowed:0","width":1,"lo":0,"hi":1},{"name":"borrowed:1","width":2,"lo":0,"hi":3}])
    );
}

#[test]
fn generated_workspace_allocations_use_acquisition_ordinals_at_one_closed_source_occurrence() {
    let bytes = source("qregion r 0 1\nqregion s 0 1\n", "cx c b");
    let object = Object::parse(&bytes).unwrap();
    let record = schedule(&bytes, 0);
    let claim =
        &record["claims"][record["verification"]["borrow_frames"][0].as_u64().unwrap() as usize];
    let workspace = claim["domain"]["workspace"].as_array().unwrap();
    assert_eq!(workspace.len(), 3);
    let source = json!({"context_key":[],"symbol":object.symbols["a"].interface_hash,"point":"$construct","iteration":[],"instance":[]});
    for ordinal in 0..2 {
        assert_eq!(
            workspace[ordinal]["allocation"],
            json!({"source":source,"allocation_ordinal":ordinal})
        );
        assert_eq!(workspace[ordinal]["lo"], 0);
        assert_eq!(workspace[ordinal]["hi"], 0);
    }
    assert_eq!(workspace[2]["allocation"]["source"]["point"], "entry:0");
    assert_eq!(workspace[2]["allocation"]["allocation_ordinal"], 0);
}

#[test]
fn nested_frame_occurrences_bind_the_complete_call_context_without_terminal_call_indices() {
    let bytes = common::caller(
        &source("", "cx c b"),
        "perm_action",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1,1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @outer (t:wire mut) (x:wire in, y:wire in) () () ()",
        "action_call @a (t) (x, y) () ()\naction_call @a (t) (x, y) () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "outer", 0, &json!([]), [0; 4]).unwrap();
    assert_eq!(trace.borrow_proofs.len(), 2);
    for proof in &trace.borrow_proofs {
        let bennett_sched::borrow::Evidence::Exhaustive { domain, frame } = &proof.evidence else {
            panic!("finite frame")
        };
        let context = trace.invocations[proof.owner].context_key();
        assert_eq!(context.as_array().unwrap().len(), 1);
        assert!(context[0]["call_index"].is_u64());
        assert_eq!(frame["first"]["context_key"], context);
        assert_eq!(frame["context_key"], context);
        assert_eq!(frame["first"]["symbol"], object.symbols["a"].interface_hash);
        assert_eq!(frame["first"]["point"], "entry:2");
        assert!(frame["first"].get("call_index").is_none());
        assert_eq!(
            domain["workspace"][0]["allocation"]["source"]["context_key"],
            context
        );
    }
}

#[test]
fn raw_inverse_frame_workspace_retains_forward_acquisition_coordinates() {
    let bytes = common::caller(
        &source("qregion r 0 1\nqregion s 0 1\n", "cx c b"),
        "oracle",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1,1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @outer (x:wire in, y:wire in) (t:wire) (f:wire[4]) () ()\nregs { r:reg[1] }",
        "lift.perm_call @a (r) (x, y) () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "outer", 0, &json!([]), [1, 0, 0, 0]).unwrap();
    assert_eq!(trace.borrow_proofs.len(), 2);
    assert!(trace.invocations.iter().any(|inv| inv.raw_inverse));
    for proof in &trace.borrow_proofs {
        let bennett_sched::borrow::Evidence::Exhaustive { domain, frame } = &proof.evidence else {
            panic!("finite frame")
        };
        let workspace = domain["workspace"].as_array().unwrap();
        assert_eq!(workspace.len(), 3);
        let allocation = &workspace
            .iter()
            .find(|item| item["allocation"]["source"]["point"] == "entry:0")
            .unwrap()["allocation"];
        assert_eq!(allocation["source"]["point"], "entry:0");
        assert_eq!(allocation["allocation_ordinal"], 0);
        assert_eq!(
            allocation["source"]["context_key"],
            trace.invocations[proof.owner].context_key()
        );
        assert_eq!(frame["first"]["point"], "entry:4");
        let mut support_ordinals = workspace
            .iter()
            .filter(|item| item["allocation"]["source"]["point"] == "$construct")
            .map(|item| item["allocation"]["allocation_ordinal"].as_u64().unwrap())
            .collect::<Vec<_>>();
        support_ordinals.sort();
        assert_eq!(support_ordinals, vec![0, 1]);
    }
}
