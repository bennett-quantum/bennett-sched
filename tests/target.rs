#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{decimal as d, target::*};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn exact_ast_k9_decades_and_context_restrictions() {
    let tables = json!({"s":[2,3,5,7,11]});
    for (eps, index) in [
        ("1", 0),
        ("0.1", 1),
        ("0.1001", 1),
        ("0.0999", 2),
        ("0.001", 3),
        ("0.000999", 4),
    ] {
        let vars = BTreeMap::from([("eps".into(), d::parse(eps).unwrap())]);
        let e = json!({"table":"s","index":"eps_decade"});
        assert_eq!(
            eval(&e, &vars, &tables).unwrap(),
            d::value(&tables["s"][index]).unwrap()
        );
    }
    let vars = BTreeMap::from([("eps".into(), d::parse("0.1").unwrap())]);
    assert_eq!(
        eval(&json!({"op":"log2inv_eps","args":[]}), &vars, &tables).unwrap(),
        d::integer(4)
    );
    assert!(eval(&json!({"var":"theta"}), &vars, &tables).is_err());
    assert!(eval(&json!({"op":"/","args":[1,0]}), &vars, &tables).is_err());
    assert_eq!(
        eval(
            &json!({"op":"ceil","args":[{"op":"/","args":[2,3]}]}),
            &vars,
            &tables
        )
        .unwrap(),
        d::integer(1)
    );
}

#[test]
fn target_validation_rejects_exact_eps_references_and_topology_errors() {
    let mut t = basic_target();
    t["gates"][0]["cost"]["time"] = json!({"var":"eps"});
    assert!(Target::from_value(t).is_err());
    let mut t = basic_target();
    t["capabilities"]["mbu"] = json!(true);
    assert!(Target::from_value(t).is_err());
    let mut t = basic_target();
    t["topology"] = json!({"vertices":[{"class":"fast"},{"class":"fast"}],"edges":[[1,0]]});
    assert!(Target::from_value(t).is_err());
}

#[test]
fn matching_continues_after_unachievable_accuracy_and_counts_effective_controls() {
    let mut t = basic_target();
    let mut approximate = t["gates"][0].clone();
    approximate["epsilon"] = json!(0.1);
    approximate["cost"]["time"] = json!(1);
    t["gates"].as_array_mut().unwrap().insert(0, approximate);
    let t = Target::from_value(t).unwrap();
    let g = Gate {
        op: "x".into(),
        wires: vec!["t".into()],
        controls: vec!["c".into()],
        theta: None,
    };
    assert_eq!(
        t.native(
            &g,
            &["fast".into(), "fast".into()],
            &d::parse("0.01").unwrap()
        )
        .unwrap()
        .unwrap()
        .duration,
        d::integer(2)
    );
    assert_eq!(
        t.native(
            &g,
            &["fast".into(), "fast".into()],
            &d::parse("0.2").unwrap()
        )
        .unwrap()
        .unwrap()
        .duration,
        d::integer(1)
    );
}

#[test]
fn published_scheme_repeat_emits_three_leaves_without_charging_a_parent() {
    let mut t = basic_target();
    t["schemes"] = json!([serde_json::from_slice::<serde_json::Value>(
        &std::fs::read(pins::spec(
            "conformance/cases/E/E-scheme-expansion/scheme.json"
        ))
        .unwrap()
    )
    .unwrap()]);
    let t = Target::from_value(t).unwrap();
    let g = Gate {
        op: "mcx".into(),
        wires: vec!["c0".into(), "c1".into(), "c2".into(), "target".into()],
        controls: vec![],
        theta: None,
    };
    let plan = t.scheme(&g, "test").unwrap().unwrap();
    assert_eq!(plan.leaves.len(), 3);
    assert_eq!(plan.ancillae.len(), 0);
    assert_eq!(
        plan.leaves
            .iter()
            .map(|g| g.wires.clone())
            .collect::<Vec<_>>(),
        vec![
            vec!["c0", "target"],
            vec!["c1", "target"],
            vec!["c2", "target"]
        ]
    );
}

pub fn basic_target() -> serde_json::Value {
    json!({
 "classes":{"fast":{"qubit_second":1,"dirty_rate":0.5,"capacity":16}},
 "resources":{"time":{"kind":"time","unit":"second"},"peak_qubits":{"kind":"peak","unit":"qubit"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"gates":{"kind":"count","unit":"gate"}},
 "gates":[{"op":"x","classes":["*"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":2,"gates":1}}],
 "schemes":[],"qram":null,"tables":{},"imports":{},"ptr_widths_supported":[32,64],
 "capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},
 "objective":{"weighted":{"time":1}} })
}
