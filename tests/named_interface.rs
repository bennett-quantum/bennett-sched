#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{
    canonical,
    expansion::{Binding, Env, Leaf, Library, Operation},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn normal(leaves: &[Leaf]) -> Value {
    let mut identities = BTreeMap::new();
    let mut wires = BTreeMap::new();
    let mut result = Vec::new();
    for leaf in leaves {
        match &leaf.op {
            Operation::Alloc {
                identity,
                wires: ws,
                ..
            } => {
                let next = format!("#{}", identities.len());
                let id = identities.entry(identity.clone()).or_insert(next).clone();
                for (i, w) in ws.iter().enumerate() {
                    wires.insert(w.clone(), format!("{id}[{i}]"));
                }
                result.push(json!(["alloc", [id]]));
            }
            Operation::Release { identity, .. } => {
                result.push(json!(["release", [identities[identity]]]))
            }
            Operation::Gate(g) => {
                assert!(g.controls.is_empty());
                result.push(json!([
                    g.op,
                    g.wires
                        .iter()
                        .map(|w| wires.get(w).unwrap_or(w))
                        .collect::<Vec<_>>()
                ]));
            }
            _ => panic!("unexpected classical leaf"),
        }
    }
    json!(result)
}
fn env(v: &Value) -> Env {
    v.as_object()
        .unwrap()
        .iter()
        .map(|(name, width)| {
            (
                name.clone(),
                Binding::Wires(
                    (0..width.as_u64().unwrap())
                        .map(|i| format!("{name}[{i}]"))
                        .collect(),
                ),
            )
        })
        .collect()
}
fn manifest() -> Value {
    serde_json::from_slice(
        &std::fs::read(pins::spec("conformance/fixtures/accepted-k82-09/k82.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn all_k82_binding_cases_match_the_published_leaves_or_rejection() {
    let manifest = manifest();
    assert_eq!(manifest["cases"].as_array().unwrap().len(), 16);
    for case in manifest["cases"].as_array().unwrap() {
        let lib = Library::from_value(
            json!({"isa":"0.9","name":"ref","version":"0.9","recipes":case["recipes"]}),
        )
        .unwrap();
        let result = lib.expand(
            case["root"].as_str().unwrap(),
            &case["args"],
            &env(&case["bundles"]),
        );
        if case["expect"] == "reject" {
            assert!(result.is_err(), "{}", case["id"]);
        } else {
            let leaves = result.unwrap_or_else(|e| panic!("{}: {e}", case["id"]));
            assert_eq!(normal(&leaves), case["expect"], "{}", case["id"]);
        }
    }
}

#[test]
fn every_k82_catalog_interface_and_all_eighty_six_edge_digests_match() {
    let manifest = manifest();
    let lib = Library::published().unwrap();
    assert_eq!(manifest["interfaces"].as_object().unwrap().len(), 54);
    for (name, wires) in manifest["interfaces"].as_object().unwrap() {
        let mut actual = bennett_sched::ref_contract::interface(&lib, name).unwrap();
        actual.sort();
        assert_eq!(json!(actual), *wires, "{name}");
    }
    let mut count = 0;
    for edge in manifest["edges"].as_array().unwrap() {
        for case in edge["instances"].as_array().unwrap() {
            let leaves = lib
                .expand(
                    edge["caller"].as_str().unwrap(),
                    &case["args"],
                    &env(&case["bundles"]),
                )
                .unwrap();
            assert_eq!(leaves.len() as u64, case["leaves"].as_u64().unwrap());
            assert_eq!(
                canonical::hash(&canonical::to_vec(&normal(&leaves)).unwrap()),
                case["sha256"],
                "{} {}",
                edge["caller"],
                case["args"]
            );
            count += 1;
        }
    }
    assert_eq!(count, 86);
}
