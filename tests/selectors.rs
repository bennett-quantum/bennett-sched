use bennett_sched::schedule::*;
use serde_json::{Value, json};

fn entry(path: Value, step: Value, kind: &str, generated: Value) -> Value {
    let mut e = json!({"path":path,"step":step,"phase":"*","generated":generated,"kind":kind});
    e[kind] = match kind {
        "gate" => json!({"realisation":"native"}),
        "alloc" => json!({"class":"fast"}),
        "cleanup" => json!({"realisation":"reverse"}),
        _ => json!({"time":0}),
    };
    e
}
fn location() -> Location {
    Location {
        root: Coordinate {
            step: Some(1),
            phase: Some(Phase::Fwd),
        },
        path: vec![
            Occurrence {
                symbol: "a".repeat(64),
                point: "entry:0".into(),
                iteration: vec![],
                instance: vec![0],
                call_index: Some(1),
                entered_oracle: Some(Coordinate {
                    step: Some(3),
                    phase: Some(Phase::Fwd),
                }),
            },
            Occurrence {
                symbol: "b".repeat(64),
                point: "$step".into(),
                iteration: vec![],
                instance: vec![],
                call_index: None,
                entered_oracle: None,
            },
        ],
        generated: 0,
        kind: Kind::Gate,
    }
}
fn path() -> Value {
    json!([{"symbol":"a".repeat(64),"point":"entry:0","iteration":"*","instance":[0],"call_index":1}])
}

#[test]
fn k52_nested_step_uses_last_oracle_edge_and_root_empty_path() {
    let loc = location();
    assert!(
        parse_entry(&entry(path(), json!(3), "gate", json!(0)))
            .unwrap()
            .matches(&loc)
    );
    assert!(
        !parse_entry(&entry(path(), json!(1), "gate", json!(0)))
            .unwrap()
            .matches(&loc)
    );
    assert!(
        parse_entry(&entry(json!([]), json!(1), "gate", json!(0)))
            .unwrap()
            .matches(&loc)
    );
    let mut caller = loc.clone();
    caller.path.pop();
    assert!(
        parse_entry(&entry(path(), json!(1), "gate", json!(0)))
            .unwrap()
            .matches(&caller)
    );
}

#[test]
fn maximal_ties_only_and_k63_selectors_do_not_increase_specificity() {
    let a = parse_entry(&entry(json!([]), json!("*"), "gate", json!("*"))).unwrap();
    let b = a.clone();
    let c = parse_entry(&entry(path(), json!(3), "gate", json!(0))).unwrap();
    assert_eq!(
        resolve(&[a.clone(), b.clone(), c.clone()], &location(), Kind::Gate)
            .unwrap()
            .unwrap()
            .0,
        2
    );
    assert!(
        resolve(&[a, b], &location(), Kind::Gate)
            .unwrap_err()
            .reason
            .contains("multiple maximal")
    );
    let mut less = c.clone();
    less.path[0].instance = Selector::Any;
    less.path[0].call_index = Selector::Any;
    assert!(resolve(&[less, c], &location(), Kind::Gate).is_err());
}

#[test]
fn independent_kinds_carriers_and_reserved_points_are_strict() {
    let e = parse_entry(&entry(path(), json!(3), "alloc", json!(0))).unwrap();
    assert!(!e.matches(&location()));
    let mut carrier = location();
    carrier.kind = Kind::Cleanup;
    let c = parse_entry(&entry(path(), json!(3), "cleanup", json!(0))).unwrap();
    assert!(c.matches(&carrier));
    assert!(
        !parse_entry(&entry(path(), json!(3), "start", json!(0)))
            .unwrap()
            .matches(&carrier)
    );
    let mut bad = entry(path(), json!(3), "alloc", json!(0));
    bad["alloc"]["class"] = json!("any");
    assert!(parse_entry(&bad).is_err());
    let mut bad = entry(path(), json!(3), "gate", json!(0));
    bad["path"][0]["point"] = json!("$bogus");
    assert!(parse_entry(&bad).is_err());
    let mut bad = entry(path(), json!(3), "gate", json!(0));
    bad["surprise"] = json!(0);
    assert!(parse_entry(&bad).is_err());
    let mut bad = entry(path(), json!(3), "cleanup", json!(0));
    bad["cleanup"]["realisation"] = json!("measure");
    assert!(parse_entry(&bad).is_err());
}

#[test]
fn published_specificity_tuple_is_lexicographic_and_default_share_is_exact() {
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(
            "../bennett-isa-spec/conformance/cases/H/H-specificity-incomparable/schedule.json",
        )
        .unwrap(),
    )
    .unwrap();
    let tuples = fixture["specificities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(tuples[0] > tuples[1]);
    let mut loc = location();
    loc.path.truncate(1);
    let mut a = entry(path(), json!("*"), "gate", json!("*"));
    a["path"][0]["iteration"] = json!([]);
    let b = entry(path(), json!(1), "gate", json!("*"));
    let entries = [parse_entry(&a).unwrap(), parse_entry(&b).unwrap()];
    assert_eq!(resolve(&entries, &loc, Kind::Gate).unwrap().unwrap().0, 0);
    assert_eq!(
        bennett_sched::decimal::exact(
            &bennett_sched::decimal::share(&bennett_sched::decimal::parse("0.01").unwrap(), 4)
                .unwrap()
        )
        .unwrap(),
        "0.0025"
    );
}

#[test]
fn a_schedule_cannot_bind_an_unavailable_expansion_library() {
    let mut v: Value = serde_json::from_slice(
        &std::fs::read(
            "../bennett-isa-spec/conformance/fixtures/controlled-action-09/schedule.json",
        )
        .unwrap(),
    )
    .unwrap();
    // The published token-free template predates K91.
    v["bindings"]["token_aliases"] = json!([]);
    v["bindings"]["expansion"]["name"] = json!("unavailable");
    assert_eq!(
        Document::parse(v).unwrap_err().status,
        bennett_sched::Status::ScheduleInfeasible
    );
}

#[test]
fn action_root_and_support_outside_steps_only_match_wildcards() {
    let mut loc = location();
    loc.root = Coordinate::default();
    loc.path.truncate(1);
    assert!(
        !parse_entry(&entry(json!([]), json!(0), "gate", json!(0)))
            .unwrap()
            .matches(&loc)
    );
    assert!(
        parse_entry(&entry(json!([]), json!("*"), "gate", json!(0)))
            .unwrap()
            .matches(&loc)
    );
}
