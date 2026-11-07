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

#[test]
fn zero_time_allocations_use_generated_lifetime_order_for_peak() {
    let live = vec![
        LiveInterval {
            class: "A".into(),
            width: 2,
            borrowed: false,
            start: d::integer(0),
            end: d::integer(0),
        },
        LiveInterval {
            class: "A".into(),
            width: 3,
            borrowed: false,
            start: d::integer(0),
            end: d::integer(0),
        },
    ];
    let reused = aggregate_ordered(&target(), &[], &live, &[(0, 1), (2, 3)]).unwrap();
    assert_eq!(reused.display["peak_qubits[A]"], "3");
    assert_eq!(reused.display["qubit_second[A]"], "0");
    assert_eq!(
        aggregate_ordered(&target(), &[], &live, &[(0, 3), (1, 2)])
            .unwrap()
            .display["peak_qubits[A]"],
        "5"
    );
    assert!(aggregate_ordered(&target(), &[], &live, &[(1, 0), (2, 3)]).is_err());
}

#[test]
fn interval_sweeps_match_an_independent_quarter_tick_occupancy_grid() {
    let target = target();
    for seed in 0..128u64 {
        let mut random = seed + 1;
        let mut next = || {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            random >> 32
        };
        let mut grid = Vec::new();
        for index in 0..12u64 {
            let first = next() % 9;
            let last = next() % 9;
            grid.push((
                first.min(last),
                first.max(last),
                1 + next() % 3,
                next() % 2 == 0,
                if index % 2 == 0 { "A" } else { "B]" },
            ));
        }
        let live = grid
            .iter()
            .map(|&(start, end, width, borrowed, class)| LiveInterval {
                class: class.into(),
                width,
                borrowed,
                start: d::integer(start) / d::integer(4),
                end: d::integer(end) / d::integer(4),
            })
            .collect::<Vec<_>>();
        let orders = (0..12).map(|i| (2 * i, 2 * i + 1)).collect::<Vec<_>>();
        let plain = aggregate(&target, &[], &live).unwrap();
        let ordered = aggregate_ordered(&target, &[], &live, &orders).unwrap();
        for class in ["A", "B]"] {
            let mut peak = 0;
            let mut half_quarter_occupancy = 0;
            for tick in 0..8 {
                let active = grid
                    .iter()
                    .filter(|&&(start, end, _, _, c)| c == class && start <= tick && tick < end);
                let mut width = 0;
                for &(_, _, demand, borrowed, _) in active {
                    width += demand;
                    half_quarter_occupancy += demand * if borrowed && class == "A" { 1 } else { 2 };
                }
                peak = peak.max(width);
            }
            let mut ordered_peak = 0;
            for tick in 0..=8 {
                for order in 0..24 {
                    let demand: u64 = grid
                        .iter()
                        .zip(&orders)
                        .filter(|&(&(start, end, _, _, c), &(acquire, release))| {
                            c == class
                                && (start, acquire) <= (tick, order)
                                && (tick, order) < (end, release)
                        })
                        .map(|(&(.., width, _, _), _)| width)
                        .sum();
                    ordered_peak = ordered_peak.max(demand);
                }
            }
            let occupancy = d::integer(half_quarter_occupancy) / d::integer(8);
            let suffix = escape(class);
            assert_eq!(
                plain.exact[&format!("peak_qubits[{suffix}]")],
                d::integer(peak),
                "seed {seed}"
            );
            assert_eq!(
                ordered.exact[&format!("peak_qubits[{suffix}]")],
                d::integer(ordered_peak),
                "seed {seed}"
            );
            for rows in [&plain, &ordered] {
                assert_eq!(
                    rows.exact[&format!("qubit_second[{suffix}]")],
                    occupancy,
                    "seed {seed}"
                );
            }
        }
    }
}
