use bennett_sched::{
    canonical, decimal as d,
    resources::{LiveInterval, TimedEvent, aggregate},
    target::{self, Target},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
fn cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bennett-isa-spec/conformance/cases")
}
fn read(path: impl AsRef<Path>) -> Value {
    canonical::parse(&std::fs::read(path).unwrap()).unwrap()
}
#[test]
fn published_cost_ast_vectors_match_their_exact_outcomes() {
    let mut count = 0;
    for id in [
        "E-ast-division-by-zero",
        "E-ast-eps-out-of-range",
        "E-ast-log2inv",
        "E-ast-unbound-variable",
        "E-table-index",
    ] {
        let dir = cases().join("E").join(id);
        let case = read(dir.join("case.json"));
        let s = &case["scenario"];
        let expr = if let Some(file) = s["expression_file"].as_str() {
            read(dir.join(file))["expr"].clone()
        } else {
            s["expression"].clone()
        };
        let mut vars = BTreeMap::new();
        if let Some(bindings) = s["bindings"].as_object() {
            for (k, v) in bindings {
                if k != "context" {
                    vars.insert(k.clone(), d::value(v).unwrap());
                }
            }
        }
        if let Some(k) = s.get("k") {
            vars.insert("k".into(), d::value(k).unwrap());
        }
        let tables = s.get("tables").cloned().unwrap_or_else(|| json!({}));
        let result = target::eval(&expr, &vars, &tables);
        if case["expected_outcome"]["load"] == "reject" {
            assert!(result.is_err(), "{id}: {result:?}");
        } else {
            assert_eq!(
                result.unwrap(),
                d::value(&case["expected_outcome"]["value"]).unwrap(),
                "{id}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 5);
}
#[test]
fn published_target_hash_and_topology_vectors_are_checked_directly() {
    for id in [
        "E-jcs-target-hash",
        "E-epsilon-zero-exact",
        "E-matching-first-entry",
        "E-topology-grid-09",
    ] {
        let dir = cases().join("E").join(id);
        let case = read(dir.join("case.json"));
        let filename = case["scenario"]["target_file"]
            .as_str()
            .or_else(|| case["scenario"]["target"].as_str())
            .unwrap();
        let t = Target::parse(&std::fs::read(dir.join(filename)).unwrap()).unwrap();
        if let Some(h) = case["expected_outcome"]["target_hash"].as_str() {
            assert_eq!(t.hash, h, "{id}");
        }
        if id == "E-topology-grid-09" {
            assert!(t.adjacent(&[0, 1]));
            assert!(!t.adjacent(&[0, 3]));
        }
    }
    for id in ["E-epsilon-zero-entry", "E-mbu-must-be-false"] {
        let dir = cases().join("E").join(id);
        assert!(
            Target::parse(&std::fs::read(dir.join("target.json")).unwrap()).is_err(),
            "{id}"
        );
    }
}
#[test]
fn published_aggregation_leaf_list_is_counted_once_with_classical_time() {
    let dir = cases().join("E/E-aggregation");
    let case = read(dir.join("case.json"));
    let data = read(dir.join("events.json"));
    let target =
        Target::parse(&std::fs::read(cases().join("E/E-epsilon-zero-exact/target.json")).unwrap())
            .unwrap();
    let mut value = target.value;
    value["resources"]["gate_count"] = json!({"kind":"count","unit":"gate"});
    let target = Target::from_value(value).unwrap();
    let events = data["leaves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| TimedEvent {
            start: d::value(&e["start"]).unwrap(),
            duration: d::value(&e["duration"]).unwrap(),
            costs: e["cost"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), d::value(v).unwrap()))
                .collect(),
        })
        .collect::<Vec<_>>();
    let live = data["live_intervals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| LiveInterval {
            class: l["class"].as_str().unwrap().into(),
            width: 1,
            borrowed: false,
            start: d::value(&l["start"]).unwrap(),
            end: d::value(&l["end"]).unwrap(),
        })
        .collect::<Vec<_>>();
    let rows = aggregate(&target, &events, &live).unwrap();
    for name in ["gate_count", "time"] {
        assert_eq!(
            rows.exact[name],
            d::value(&case["expected_outcome"][name]).unwrap()
        );
    }
    for name in ["peak_qubits", "qubit_second"] {
        assert_eq!(
            rows.exact[&format!("{name}[fast]")],
            d::value(&case["expected_outcome"][name]["fast"]).unwrap()
        );
    }
}

#[test]
fn published_import_quantity_is_mapped_exactly_and_bound_to_model_version() {
    let case = read(cases().join("E/E-imports/case.json"));
    let s = &case["scenario"];
    let mut value =
        Target::parse(&std::fs::read(cases().join("E/E-epsilon-zero-exact/target.json")).unwrap())
            .unwrap()
            .value;
    value["resources"]["gates"] = json!({"kind":"count","unit":"gate"});
    value["imports"] =
        json!({"model":{"version":s["model_version"],"map":{"quantity":s["mapping"]}}});
    let target = Target::from_value(value.clone()).unwrap();
    let (name, amount) = target
        .import_quantity(
            "model",
            s["model_version"].as_str().unwrap(),
            "quantity",
            &d::value(&s["quantity"]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        amount,
        d::value(&case["expected_outcome"]["resources"][&name]).unwrap()
    );
    assert!(
        target
            .import_quantity("model", "other", "quantity", &d::integer(3))
            .is_err()
    );
    assert!(
        target
            .import_quantity("missing", "0.8", "quantity", &d::integer(3))
            .is_err()
    );
    value["imports"]["model"]["map"]["quantity"] = json!(["not-declared", 2]);
    assert!(Target::from_value(value).is_err());
}
