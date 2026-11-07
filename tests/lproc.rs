#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{lproc::IndexSpace, object::Object};
fn example(index: usize) -> Object {
    let spec = std::fs::read_to_string(pins::spec("spec/Bennett-ISA-Spec-0.9.md")).unwrap();
    let text = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("## 17.")
        .next()
        .unwrap();
    let blocks = text
        .split("```")
        .skip(1)
        .step_by(2)
        .map(|s| s.trim_start_matches('\n'))
        .filter(|s| s.starts_with("bennett-object "))
        .collect::<Vec<_>>();
    Object::parse(blocks[index - 1].as_bytes()).unwrap()
}
#[test]
fn normative_forwarded_bindings_form_the_exact_instance_key_row_range_and_activation() {
    let object = example(10);
    let index = IndexSpace::form(&object, "through").unwrap();
    assert_eq!(index.halt, 8);
    assert_eq!(index.pc_width, 4);
    assert_eq!(index.instances.len(), 1);
    let worker = &index.instances[0];
    assert_eq!(worker.name, "worker");
    assert_eq!(worker.key, [0, 1, 0]);
    assert_eq!(
        (worker.first, worker.rows, worker.activation_cells),
        (2, 6, 8)
    );
    assert_eq!(
        worker
            .locations
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        ["x", "r", "fr", "zero", "bad"]
    );
    assert_eq!(
        worker.locations.iter().map(|l| l.id).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(worker.labels["entry"], 2);
}
#[test]
fn recursive_empty_binding_key_closes_once_and_declared_locations_keep_their_order() {
    let object = example(2);
    let index = IndexSpace::form(&object, "sum_db").unwrap();
    assert_eq!(index.instances.len(), 1);
    let sum = &index.instances[0];
    assert_eq!(sum.name, "sum");
    assert!(sum.key.is_empty());
    assert_eq!(sum.first, 4);
    assert_eq!(sum.activation_cells, 16);
    assert_eq!(sum.locations.len(), 13);
    assert_eq!(
        sum.locations
            .iter()
            .take(4)
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        ["n", "acc", "base", "r"]
    );
    assert_eq!(sum.locations[0].id, 3);
    assert_eq!(index.pc_width, 64 - index.halt.leading_zeros() as u64);
}

#[test]
fn fixed_arguments_reject_locations_input_wires_literals_and_omitted_nonempty_lists() {
    for replacement in ["(r)", "(x)", "(0)"] {
        let mut object = example(10);
        object.functions.get_mut("through").unwrap().body[0].operands[2] = replacement.into();
        assert!(
            IndexSpace::form(&object, "through").is_err(),
            "{replacement}"
        );
    }
    let mut object = example(10);
    object.functions.get_mut("through").unwrap().body[0]
        .operands
        .truncate(2);
    assert!(IndexSpace::form(&object, "through").is_err());
}

#[test]
fn fixed_binding_keys_keep_distinct_argument_positions_independent_of_runtime_values() {
    // Construct typed source IR with two forwards of the same type. Runtime
    // values are deliberately absent from IndexSpace::form and cannot merge keys.
    let mut object = example(10);
    let root = object.functions.get_mut("through").unwrap();
    let mut argument = root.groups[4][1].clone();
    argument.name = "b".into();
    root.groups[4].push(argument);
    let mut call = root.body[0].clone();
    call.point = "entry:1".into();
    call.operands[3] = "(b)".into();
    root.body[1].point = "entry:2".into();
    root.body.insert(1, call);
    let space = IndexSpace::form(&object, "through").unwrap();
    assert_eq!(
        space
            .instances
            .iter()
            .map(|i| i.key.clone())
            .collect::<Vec<_>>(),
        [vec![0, 1, 0], vec![0, 2, 0]]
    );
    assert_eq!(
        space.instances.iter().map(|i| i.first).collect::<Vec<_>>(),
        [3, 9]
    );
}

#[test]
fn resolved_instance_values_preserve_the_nested_token_tuple_and_classical_value() {
    use serde_json::json;
    let object = example(10);
    let space = IndexSpace::form(&object, "through").unwrap();
    let params = json!([["6", "0", "0", "0"], "3"]);
    let bindings = space
        .resolve_bindings(&object, &space.instances[0], &params)
        .unwrap();
    assert_eq!(bindings.tokens, vec![json!(["6", "0", "0", "0"])]);
    assert_eq!(bindings.classical, vec![json!("3")]);
    assert_eq!(bindings.regions, vec![0]);
    assert!(
        object
            .validate_params(
                "through",
                &json!([["18446744073709551616", "0", "0", "0"], "3"])
            )
            .is_err()
    );
    assert!(
        object
            .validate_params("through", &json!([["6", "33", "0", "0"], "3"]))
            .is_err()
    );
    assert!(
        object
            .validate_params("through", &json!([["6", "0", "17", "0"], "3"]))
            .is_err()
    );
}

#[test]
fn call_targets_resolve_fixed_keys_in_the_callers_instance_and_recursive_calls_keep_their_key() {
    let object = example(10);
    let space = IndexSpace::form(&object, "through").unwrap();
    let target = space
        .call_target(&object, None, &object.functions["through"].body[0])
        .unwrap();
    assert_eq!(
        (target.name.as_str(), target.key.as_slice()),
        ("worker", [0, 1, 0].as_slice())
    );
    let object = example(2);
    let space = IndexSpace::form(&object, "sum_db").unwrap();
    let sum = &space.instances[0];
    let node = object.functions["sum"]
        .body
        .iter()
        .find(|n| n.op == "lift.call")
        .unwrap();
    let target = space.call_target(&object, Some(sum), node).unwrap();
    assert_eq!(target.first, sum.first);
    assert!(target.key.is_empty());
    let mut bad = node.clone();
    bad.operands[0] = "@missing".into();
    assert!(space.call_target(&object, Some(sum), &bad).is_err());
}
