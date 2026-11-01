use bennett_sched::{decimal as d, resources::*, target::Target};
use serde_json::json;
use std::collections::BTreeMap;

fn target() -> Target {
    Target::from_value(json!({
 "classes":{"A":{"capacity":8,"qubit_second":1,"dirty_rate":0.5},"B]":{"capacity":8,"qubit_second":1,"dirty_rate":1},"idle":{"capacity":8,"qubit_second":1,"dirty_rate":1}},
 "resources":{"time":{"kind":"time","unit":"s"},"peak_qubits":{"kind":"peak","unit":"q"},"qubit_second":{"kind":"occupancy","unit":"qs"},"gates":{"kind":"count","unit":"gate"}},
 "gates":[],"schemes":[],"qram":null,"tables":{},"imports":{},"ptr_widths_supported":[32],
 "capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}})).unwrap()
}

#[test]
fn k12_exact_per_class_rows_escape_names_and_upward_round_only_at_display() {
    let third = d::integer(1) / d::integer(3);
    let events = vec![TimedEvent {
        start: d::integer(0),
        duration: third.clone(),
        costs: BTreeMap::from([("gates".into(), d::integer(1))]),
    }];
    let live = vec![
        LiveInterval {
            class: "A".into(),
            width: 2,
            borrowed: false,
            start: d::integer(0),
            end: third.clone(),
        },
        LiveInterval {
            class: "B]".into(),
            width: 1,
            borrowed: false,
            start: d::integer(0),
            end: third.clone(),
        },
    ];
    let rows = aggregate(&target(), &events, &live).unwrap();
    assert_eq!(rows.exact["time"], third);
    assert_eq!(
        rows.display["qubit_second[A]"],
        "0.6666666666666666666666666666666667"
    );
    assert_eq!(
        rows.display["qubit_second[B%5D]"],
        "0.3333333333333333333333333333333334"
    );
    assert_eq!(rows.display["peak_qubits[idle]"], "0");
    assert_eq!(escape("a%[é]"), "a%25%5B%C3%A9%5D");
}

#[test]
fn scheduled_overlap_sums_demands_and_half_open_reuse_does_not() {
    let live = vec![
        LiveInterval {
            class: "A".into(),
            width: 2,
            borrowed: false,
            start: d::integer(0),
            end: d::integer(1),
        },
        LiveInterval {
            class: "A".into(),
            width: 2,
            borrowed: false,
            start: d::integer(0),
            end: d::integer(1),
        },
    ];
    let rows = aggregate(&target(), &[], &live).unwrap();
    assert_eq!(rows.display["peak_qubits[A]"], "4");
    assert_eq!(rows.display["qubit_second[A]"], "4");
    let mut reused = live.clone();
    reused[1].start = d::integer(1);
    reused[1].end = d::integer(2);
    assert_eq!(
        aggregate(&target(), &[], &reused).unwrap().display["peak_qubits[A]"],
        "2"
    );
}
