use bennett_sched::{
    expansion::{Binding, Capture, Env, Library, Template, inverse},
    object::Object,
    source_registry::OracleInvocation,
};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
#[path = "common/basis.rs"]
mod basis;
mod common;

fn inc8() -> Arc<Object> {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let source = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n');
    Arc::new(Object::parse(source.as_bytes()).unwrap())
}
fn wires(name: &str, n: usize) -> Binding {
    Binding::Wires((0..n).map(|i| format!("{name}[{i}]")).collect())
}
fn context(b: u64) -> OracleInvocation {
    OracleInvocation::parse(&json!({"token":[b.to_string(),"0","0","0"]})).unwrap()
}
fn setup() -> (Library, Env) {
    let object = inc8();
    let hash = object.closure_hash("inc8").unwrap().to_owned();
    let mut lib = Library::published().unwrap();
    assert_eq!(lib.link_source(object, "inc8").unwrap(), hash);
    lib.recipes.insert("forward".into(),json!({"parameters":[{"name":"S","type":"symbol"},{"name":"A","type":"arguments","symbol":"S"}],"body":[{"call":"S","args":"A"}]}));
    for (name, formal, body) in [
        (
            "hold-body",
            json!({"name":"B","type":"body"}),
            json!({"splice":"B"}),
        ),
        (
            "hold-template",
            json!({"name":"B","type":"templates","length":1}),
            json!({"apply":"B","index":0,"args":[]}),
        ),
    ] {
        lib.recipes.insert(name.into(),json!({"parameters":[formal,{"name":"S","type":"symbol"},{"name":"A2","type":"arguments","symbol":"S"}],"body":[body]}));
    }
    let mut env = Env::new();
    env.insert("S".into(), Binding::Symbol(hash.clone()));
    for b in [2, 3] {
        env.insert(
            format!("A{b}"),
            Binding::Arguments {
                symbol: hash.clone(),
                values: vec![wires("x", 8), wires("t", 8), wires("f", 4)],
                oracle_invocation: Some(context(b)),
            },
        );
    }
    (lib, env)
}

#[test]
fn source_registry_replays_the_actual_k77_two_token_witness_through_direct_and_captured_adapters() {
    let (lib, env) = setup();
    for capture in 0..5 {
        let call = json!({"invoke":"forward","args":["S","A2"]});
        let mut current = env.clone();
        let first = match capture {
            0 => call,
            1 => {
                current.insert(
                    "saved".into(),
                    Binding::Body(Capture {
                        body: vec![call],
                        env: env.clone(),
                        controls: vec![],
                    }),
                );
                current.insert("A2".into(), env["A3"].clone());
                json!({"splice":"saved"})
            }
            2 => {
                current.insert(
                    "saved".into(),
                    Binding::Templates(vec![Template {
                        parameters: vec![],
                        capture: Capture {
                            body: vec![call],
                            env: env.clone(),
                            controls: vec![],
                        },
                    }]),
                );
                current.insert("A2".into(), env["A3"].clone());
                json!({"apply":"saved","index":0,"args":[]})
            }
            3 => json!({"invoke":"hold-body","args":[[call],"S","A3"]}),
            _ => {
                json!({"invoke":"hold-template","args":[[{"parameters":[],"body":[call]}],"S","A3"]})
            }
        };
        let leaves = lib
            .body(
                &[first, json!({"invoke":"forward","args":["S","A3"]})],
                &current,
                &["p".into()],
            )
            .unwrap();
        let mut budgets = BTreeMap::new();
        for leaf in &leaves {
            assert_eq!(leaf.calls.len(), 1);
            let origin = leaf
                .origin
                .iter()
                .find_map(|o| o.source.as_ref())
                .expect("source provenance was flattened away");
            budgets.insert(leaf.calls[0].call_index, origin.trace.invocations[0].budget);
            assert_eq!(
                leaf.ordinal,
                origin.trace.events[origin.event].location.generated
            );
        }
        assert_eq!(
            budgets,
            BTreeMap::from([(0, [2, 0, 0, 0]), (1, [3, 0, 0, 0])])
        );
        assert_eq!(inverse(&inverse(&leaves)), leaves);
        let program = basis::Program::compile(&leaves).unwrap();
        let mut cases = Vec::new();
        for x in 0..256 {
            for p in [false, true] {
                let mut bits = BTreeMap::from([("p".into(), p)]);
                for (name, n, value) in [("x", 8, x), ("t", 8, 0), ("f", 4, 0)] {
                    for i in 0..n {
                        bits.insert(format!("{name}[{i}]"), value & (1 << i) != 0);
                    }
                }
                cases.push(bits);
            }
        }
        program.run_many(&mut cases).unwrap();
        for (i, bits) in cases.iter().enumerate() {
            let x = (i / 2) as u64;
            let p = i % 2 == 1;
            let get = |name: &str, n: usize| {
                (0..n)
                    .map(|j| u64::from(bits[&format!("{name}[{j}]")]) << j)
                    .sum::<u64>()
            };
            assert_eq!(get("x", 8), x);
            assert_eq!(get("t", 8), if p { (x + 1) % 256 } else { 0 });
            assert_eq!(get("f", 4), u64::from(p));
            assert_eq!(bits.len(), 21);
        }
        basis::Program::compile(&inverse(&leaves))
            .unwrap()
            .run_many(&mut cases)
            .unwrap();
        for (i, bits) in cases.iter().enumerate() {
            for j in 0..8 {
                assert_eq!(bits[&format!("x[{j}]")], (i / 2) & (1 << j) != 0);
                assert!(!bits[&format!("t[{j}]")]);
            }
            for j in 0..4 {
                assert!(!bits[&format!("f[{j}]")]);
            }
        }
    }
}

#[test]
fn oracle_context_is_required_typed_closed_and_never_a_positional_parameter() {
    let (lib, mut env) = setup();
    for bad in [
        json!({}),
        json!({"token":["2","0","0"]}),
        json!({"token":["2","0","0","0"],"extra":0}),
        json!({"token":[2,0,0,0]}),
        json!({"token":["18446744073709551616","0","0","0"]}),
    ] {
        assert!(OracleInvocation::parse(&bad).is_err(), "{bad}");
    }
    let mut bad = env["A2"].clone();
    if let Binding::Arguments {
        oracle_invocation, ..
    } = &mut bad
    {
        *oracle_invocation = None;
    }
    env.insert("A2".into(), bad);
    let error = lib
        .body(&[json!({"invoke":"forward","args":["S","A2"]})], &env, &[])
        .unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::Arg);
    assert!(error.reason.contains("oracle_invocation"), "{error}");
    let (lib, mut env) = setup();
    let mut bad = env["A2"].clone();
    if let Binding::Arguments { values, .. } = &mut bad {
        values.push(Binding::Value {
            isa_type: json!("token"),
            value: json!(["2", "0", "0", "0"]),
        });
    }
    env.insert("A2".into(), bad);
    assert!(
        lib.body(&[json!({"invoke":"forward","args":["S","A2"]})], &env, &[])
            .unwrap_err()
            .reason
            .contains("argument")
    );
    let (lib, env) = setup();
    let error = lib
        .body(
            &[json!({"call":"S","args":"A2","oracle_invocation":{"token":["2","0","0","0"]}})],
            &env,
            &[],
        )
        .unwrap_err();
    assert!(error.reason.contains("unknown field"), "{error}");
}

#[test]
fn linked_source_arguments_preserve_typed_values_and_reject_wrong_widths_types_and_contexts() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["i128","f64","token","ptr"]});
    let bytes = common::action(
        "oracle",
        "copy",
        signature,
        "oracle @copy (x:wire in) (t:wire) (f:wire[4]) () (big:i128, angle:f64, inner:token, addr:ptr)",
        "lift.retval x",
    );
    let object = Arc::new(Object::parse(&bytes).unwrap());
    let mut lib = Library::published().unwrap();
    let hash = lib.link_source(object.clone(), "copy").unwrap();
    assert!(
        lib.link_source(object, "copy")
            .unwrap_err()
            .reason
            .contains("duplicate")
    );
    let params = json!([
        "340282366920938463463374607431768211455",
        "0x7ff8000000000001",
        ["9", "0", "0", "0"],
        "4294967295"
    ]);
    let types = [
        json!("i128"),
        json!("f64"),
        json!("token"),
        json!({"ptr":32}),
    ];
    let mut values = vec![wires("x", 1), wires("t", 1), wires("f", 4)];
    values.extend(
        types
            .into_iter()
            .zip(params.as_array().unwrap())
            .map(|(isa_type, value)| Binding::Value {
                isa_type,
                value: value.clone(),
            }),
    );
    let mut env = Env::new();
    env.insert("S".into(), Binding::Symbol(hash.clone()));
    let mut run = |values: Vec<Binding>, invocation: OracleInvocation| {
        env.insert(
            "A".into(),
            Binding::Arguments {
                symbol: hash.clone(),
                values,
                oracle_invocation: Some(invocation),
            },
        );
        lib.body(
            &[json!({"invoke":"lift.oracle_call","args":["S","A"]})],
            &env,
            &[],
        )
    };
    let leaves = run(values.clone(), context(1)).unwrap();
    let origin = leaves[0]
        .origin
        .iter()
        .find_map(|o| o.source.as_ref())
        .unwrap();
    assert_eq!(origin.trace.invocations[0].params, params);
    assert_eq!(origin.trace.invocations[0].budget, [1, 0, 0, 0]);
    for leaf in &leaves {
        let origin = leaf.origin.iter().find_map(|o| o.source.as_ref()).unwrap();
        assert_eq!(leaf.op, origin.trace.events[origin.event].leaf.op);
    }
    let mut bad = values.clone();
    bad[3] = Binding::Value {
        isa_type: json!("i64"),
        value: json!("0"),
    };
    assert!(run(bad, context(1)).unwrap_err().reason.contains("type"));
    let mut bad = values.clone();
    bad.swap(0, 2);
    assert!(run(bad, context(1)).unwrap_err().reason.contains("width"));
    let bad = OracleInvocation::parse(&json!({"token":["1","1","0","0"]})).unwrap();
    assert!(run(values, bad).is_err());
}

#[test]
fn argument_context_survives_an_additional_semantic_call_and_source_host_limits_are_explicit() {
    use bennett_sched::expansion::Symbol;
    let (mut lib, mut env) = setup();
    let wrapper = bennett_sched::canonical::hash(b"typed-constructor-forwarding-fixture");
    let parameters = vec![
        json!({"name":"S","type":"symbol"}),
        json!({"name":"A","type":"arguments","symbol":"S"}),
    ];
    lib.symbols.insert(
        wrapper.clone(),
        Symbol::constructors(parameters, vec![json!({"call":"S","args":"A"})]).unwrap(),
    );
    env.insert("W".into(), Binding::Symbol(wrapper.clone()));
    env.insert(
        "WA".into(),
        Binding::Arguments {
            symbol: wrapper,
            values: vec![env["S"].clone(), env["A2"].clone()],
            oracle_invocation: None,
        },
    );
    let leaves = lib
        .body(&[json!({"call":"W","args":"WA"})], &env, &[])
        .unwrap();
    assert!(leaves.iter().all(|l| l.calls.len() == 2));
    let origin = leaves[0]
        .origin
        .iter()
        .find_map(|o| o.source.as_ref())
        .unwrap();
    assert_eq!(origin.trace.invocations[0].budget, [2, 0, 0, 0]);
    let error = lib
        .body_with_budget(&[json!({"call":"W","args":"WA"})], &env, &[], 1)
        .unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::NoMem);
}

#[test]
fn linked_action_regions_preserve_shape_role_and_physical_cells_without_an_oracle_context() {
    use bennett_sched::expansion::simulate_basis;
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[{"A":0,"W":1,"role":"mut"}],"args":[],"bounds":[]});
    let mut bytes = common::action(
        "perm_action",
        "swapcell",
        sig,
        "perm_action @swapcell (t:wire mut) (x:wire in) (m:region[0,1] mut) () ()",
        "qexchm m () t",
    );
    bytes.extend_from_slice(b"\nsection layout\n{\"A_tab\":0,\"H\":0,\"S_cells\":0,\"globals\":{},\"heap_base\":1,\"heap_limit_max\":0,\"interface_regions\":{},\"null_size\":1,\"ptr_width\":32}\n");
    let object = Arc::new(Object::parse(&bytes).unwrap());
    let mut lib = Library::published().unwrap();
    let hash = lib.link_source(object, "swapcell").unwrap();
    let region = Binding::Region {
        address: 0,
        cell: 1,
        role: "mut".into(),
        origin: "interface:actual".into(),
        wires: vec!["cell".into()],
    };
    let values = vec![wires("t", 1), wires("x", 1), region];
    let mut env = Env::new();
    env.insert("S".into(), Binding::Symbol(hash.clone()));
    env.insert(
        "A".into(),
        Binding::Arguments {
            symbol: hash.clone(),
            values: values.clone(),
            oracle_invocation: None,
        },
    );
    let body = [json!({"invoke":"lift.perm_call","args":["S","A"]})];
    let leaves = lib.body(&body, &env, &["p".into()]).unwrap();
    for bits in 0..16 {
        let mut state = BTreeMap::from([
            ("t[0]".into(), bits & 1 != 0),
            ("x[0]".into(), bits & 2 != 0),
            ("cell".into(), bits & 4 != 0),
            ("p".into(), bits & 8 != 0),
        ]);
        simulate_basis(&leaves, &mut state).unwrap();
        assert_eq!(state["t[0]"], bits & if bits & 8 != 0 { 4 } else { 1 } != 0);
        assert_eq!(state["cell"], bits & if bits & 8 != 0 { 1 } else { 4 } != 0);
        assert_eq!(state["x[0]"], bits & 2 != 0);
        assert_eq!(state.len(), 4);
    }
    let mut bad = values.clone();
    if let Binding::Region { role, .. } = &mut bad[2] {
        *role = "in".into();
    }
    env.insert(
        "A".into(),
        Binding::Arguments {
            symbol: hash.clone(),
            values: bad,
            oracle_invocation: None,
        },
    );
    assert!(
        lib.body(&body, &env, &[])
            .unwrap_err()
            .reason
            .contains("role")
    );
    env.insert(
        "A".into(),
        Binding::Arguments {
            symbol: hash,
            values,
            oracle_invocation: Some(context(0)),
        },
    );
    assert!(
        lib.body(&body, &env, &[])
            .unwrap_err()
            .reason
            .contains("oracle_invocation")
    );
}
