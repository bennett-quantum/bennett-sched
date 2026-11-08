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
            let occurrence = |p| json!([{"symbol":object.symbols["a"].interface_hash,"point":format!("entry:{p}"),"iteration":[],"instance":[]}]);
            assert_eq!(claim["borrow_frame"]["first"], occurrence(point));
            assert_eq!(claim["borrow_frame"]["last"], occurrence(point));
            assert_eq!(
                claim["domain"]["workspace"],
                json!([{"allocation":occurrence(0),"offset":0,"width":1,"lo":0,"hi":0}])
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
