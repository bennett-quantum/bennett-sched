use crate::target::basic_target;
use bennett_sched::{
    Status, canonical, decimal as d,
    target::{Gate, Target},
};
use serde_json::{Value, json};

fn model() -> Value {
    json!({"version":"bennett.native-model/1","kind":"unitary","arity":1,"basis":["t"],"parameter_bindings":{"k":"k","c":"c"},
        "control_models":{"kind":"controlled_U","controls":"c","disabled":"identity"},"operand_map":[0],
        "data":{"matrix":[[[["0","1"],["0","1"]],[["1","1"],["0","1"]]],[[["1","1"],["0","1"]],[["0","1"],["0","1"]]]]}})
}
fn target(m: Value, controls: u64) -> Value {
    let mut t = basic_target();
    t["gates"][0]["controls"] = json!({"min":controls,"max":controls});
    t["gates"][0]["realisation_model"] = m;
    t
}
fn gate(controls: usize) -> Gate {
    Gate {
        op: "x".into(),
        wires: vec!["t".into()],
        controls: (0..controls).map(|i| format!("c{i}")).collect(),
        theta: None,
    }
}

#[test]
fn native_unitary_model_bytes_bind_target_identity_and_resolved_control_arity() {
    let mut m = model();
    for controls in 0..=1 {
        m["operand_map"] = if controls == 0 {
            json!([0])
        } else {
            json!([0, 1])
        };
        let bytes = target(m.clone(), controls);
        let parsed = Target::from_value(bytes.clone()).unwrap();
        assert_eq!(parsed.value["gates"][0]["realisation_model"], m);
        assert_eq!(
            parsed.hash,
            canonical::hash(&canonical::to_vec(&bytes).unwrap())
        );
        let mut legacy = bytes;
        legacy["gates"][0]
            .as_object_mut()
            .unwrap()
            .remove("realisation_model");
        assert_ne!(parsed.hash, Target::from_value(legacy).unwrap().hash);
        let native = parsed
            .native(
                &gate(controls as usize),
                &vec!["fast".into(); controls as usize + 1],
                &d::integer(0),
            )
            .unwrap()
            .unwrap();
        assert_eq!(native.epsilon, d::integer(0));
        assert_eq!(native.duration, d::integer(2));
    }
}

#[test]
fn native_models_reject_malformed_dimensions_constants_permutations_and_nonunitarity() {
    let mut malformed = Vec::new();
    let mut m = model();
    m["operand_map"] = json!([0, 0]);
    malformed.push(m);
    let mut m = model();
    m["basis"] = json!(["t", "t"]);
    malformed.push(m);
    let mut m = model();
    m["data"]["matrix"][0][0][0] = json!(["0", "2"]);
    malformed.push(m);
    let mut m = model();
    m["data"]["matrix"][0][1][0] = json!(["2", "1"]);
    malformed.push(m);
    let mut m = model();
    m["data"]["matrix"][0] = json!([]);
    malformed.push(m);
    let mut m = model();
    m["kind"] = json!("channel");
    malformed.push(m);
    let mut m = model();
    m["data"]["matrix"][0][0][0] = json!({"var":"undeclared"});
    malformed.push(m);
    for m in malformed {
        let e = Target::from_value(target(m, 0)).unwrap_err();
        assert_eq!(e.status, Status::Target, "{e}");
    }
}

#[test]
fn native_binding_rejects_wrong_resolved_map_and_zero_epsilon_phase() {
    for m in [model(), {
        let mut m = model();
        m["data"]["matrix"][0][1][0] = json!(["-1", "1"]);
        m["data"]["matrix"][1][0][0] = json!(["-1", "1"]);
        m["operand_map"] = json!([0, 1]);
        m
    }] {
        let t = Target::from_value(target(m, 1)).unwrap();
        let e = t
            .native(&gate(1), &["fast".into(), "fast".into()], &d::integer(0))
            .unwrap_err();
        assert_eq!(e.status, Status::Target);
    }
}

#[test]
fn legacy_missing_and_future_models_preserve_scheduling_compatibility() {
    for supplied in [
        None,
        Some(
            json!({"version":"bennett.native-model/2","future_representation":{"operation":"external"}}),
        ),
    ] {
        let mut t = basic_target();
        t["gates"][0]["epsilon"] = json!(0.1);
        if let Some(m) = supplied {
            t["gates"][0]["realisation_model"] = m;
        }
        let parsed = Target::from_value(t.clone()).unwrap();
        assert_eq!(parsed.value, t);
        assert!(
            parsed
                .native(&gate(0), &["fast".into()], &d::parse("0.1").unwrap())
                .unwrap()
                .is_some()
        );
    }
}
