use bennett_sched::expansion::*;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn published_bound_qxor_all_sixteen_tuples_and_inverse_ordinals() {
    let lib = Library::published().unwrap();
    let leaves = lib
        .expand(
            "bind.qxor",
            &json!([2, ["s0", "s1"], ["t0", "t1"]]),
            &Env::new(),
        )
        .unwrap();
    assert_eq!(leaves.len(), 2);
    assert_eq!(leaves.iter().map(|l| l.ordinal).collect::<Vec<_>>(), [0, 1]);
    for s in 0..4 {
        for t in 0..4 {
            let mut bits = BTreeMap::from([
                ("s0".into(), s & 1 != 0),
                ("s1".into(), s & 2 != 0),
                ("t0".into(), t & 1 != 0),
                ("t1".into(), t & 2 != 0),
            ]);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(usize::from(bits["t0"]) + 2 * usize::from(bits["t1"]), t ^ s);
            simulate_basis(&inverse(&leaves), &mut bits).unwrap();
            assert_eq!(usize::from(bits["t0"]) + 2 * usize::from(bits["t1"]), t);
        }
    }
    assert_eq!(
        inverse(&leaves)
            .iter()
            .map(|l| l.ordinal)
            .collect::<Vec<_>>(),
        [1, 0]
    );
}

#[test]
fn k21_images_are_unconditional_and_k19_types_are_checked() {
    let lib = Library::published().unwrap();
    let b = vec![json!({"image":"I","wires":"w"})];
    let env = Env::from([
        ("I".into(), Binding::Image("8102".into())),
        (
            "w".into(),
            Binding::Wires((0..16).map(|i| format!("w{i}")).collect()),
        ),
    ]);
    let leaves = lib.body(&b, &env, &["external".into()]).unwrap();
    let gates: Vec<_> = leaves
        .iter()
        .map(|l| match &l.op {
            Operation::Gate(g) => (g.wires.clone(), g.controls.clone()),
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        gates,
        vec![
            (vec!["w0".into()], vec![]),
            (vec!["w7".into()], vec![]),
            (vec!["w9".into()], vec![])
        ]
    );
    assert!(
        lib.expand("bind.qxor", &json!([2, ["a"], ["b", "c"]]), &Env::new())
            .is_err()
    );
    assert!(
        lib.expand("value.literal", &json!([2, -1, ["a", "b"]]), &Env::new())
            .is_err()
    );
}

#[test]
fn captured_templates_and_fresh_scratch_do_not_capture_callee_locals() {
    let lib = Library::published().unwrap();
    let env = Env::from([
        ("outer".into(), Binding::Wires(vec!["a".into()])),
        ("w".into(), Binding::Wires(vec!["b".into(), "c".into()])),
    ]);
    let body = json!([{"invoke":"qloop","args":[2,[{"alloc":"tmp","width":1,"class":"any","state":"clean"},{"gate":"cx","wires":["outer","tmp"]},{"gate":"cx","wires":["outer","tmp"]},{"release":"tmp"}]]}]);
    let leaves = lib.body(body.as_array().unwrap(), &env, &[]).unwrap();
    assert_ne!(leaves[0].wires(), leaves[4].wires());
    for a in [false, true] {
        let mut bits = BTreeMap::from([("a".into(), a)]);
        simulate_basis(&leaves, &mut bits).unwrap();
        assert_eq!(bits, BTreeMap::from([("a".into(), a)]));
    }
}

#[test]
fn undeclared_helper_operands_are_reported_with_the_contract_gap() {
    let lib = Library::published().unwrap();
    let err = lib
        .expand(
            "bind.value.add",
            &json!([1, ["a"], ["b"], ["t"]]),
            &Env::new(),
        )
        .unwrap_err();
    assert!(err.reason.contains("unbound"), "{err}");
    assert!(err.reason.contains("bennett-sched-enz"),"{err}");
    assert_eq!(bennett_sched::ref_contract::unbound_wires(&lib.recipes["qxor"]),std::collections::BTreeSet::from(["s".to_owned(),"t".to_owned()]));
    assert!(bennett_sched::ref_contract::unbound_wires(&lib.recipes["bind.qxor"]).is_empty());
}

#[test]
fn declaration_reacquisition_is_fresh_and_library_scratch_is_lifo() {
    let lib = Library::published().unwrap();
    let body = json!([{"declare":"d","width":1,"class":{"class":"fast"},"state":"clean","body":[{"acquire":"d"},{"relinquish":"d"},{"acquire":"d"},{"relinquish":"d"}]}]);
    let leaves = lib
        .body(body.as_array().unwrap(), &Env::new(), &[])
        .unwrap();
    assert_ne!(leaves[0].wires(), leaves[2].wires());
    let leaked = json!([{"declare":"d","width":1,"class":{"class":"fast"},"state":"clean","body":[{"acquire":"d"}]}]);
    assert!(
        lib.body(leaked.as_array().unwrap(), &Env::new(), &[])
            .is_err()
    );
    let bad = json!([{"alloc":"a","width":1,"class":"any","state":"clean"},{"alloc":"b","width":1,"class":"any","state":"clean"},{"release":"a"},{"release":"b"}]);
    assert!(lib.body(bad.as_array().unwrap(), &Env::new(), &[]).is_err());
}
