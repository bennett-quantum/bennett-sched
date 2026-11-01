//! §8.2 aggregation from realised native leaves and wire lifetimes.
use crate::{
    Error, Result,
    decimal::{self as d, Rational},
    target::Target,
};
use num_traits::{Signed, Zero};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct TimedEvent {
    pub start: Rational,
    pub duration: Rational,
    pub costs: BTreeMap<String, Rational>,
}
#[derive(Clone, Debug)]
pub struct LiveInterval {
    pub class: String,
    pub width: u64,
    pub borrowed: bool,
    pub start: Rational,
    pub end: Rational,
}
#[derive(Clone, Debug)]
pub struct Rows {
    pub exact: BTreeMap<String, Rational>,
    pub display: BTreeMap<String, String>,
}
impl Rows {
    pub fn exact_json(&self) -> Value {
        Value::Object(self.exact.iter().map(|(k,v)|(k.clone(),json!({"numerator":v.numer().to_string(),"denominator":v.denom().to_string()}))).collect())
    }
    pub fn display_json(&self) -> Value {
        json!(self.display)
    }
}
pub fn escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
pub fn aggregate(target: &Target, events: &[TimedEvent], live: &[LiveInterval]) -> Result<Rows> {
    let mut makespan = Rational::zero();
    for event in events {
        if event.start.is_negative()
            || event.duration.is_negative()
            || event.costs.values().any(Signed::is_negative)
        {
            return Err(Error::infeasible(
                "§8.2: negative scheduled event cost/time",
            ));
        }
        makespan = makespan.max(&event.start + &event.duration);
    }
    for interval in live {
        if interval.start.is_negative()
            || interval.end < interval.start
            || !target.classes.contains_key(&interval.class)
        {
            return Err(Error::infeasible("§8.2: invalid wire live interval"));
        }
        makespan = makespan.max(interval.end.clone());
    }
    let mut exact = BTreeMap::new();
    for (name, resource) in target.value["resources"]
        .as_object()
        .expect("validated resources")
    {
        match resource["kind"].as_str().expect("validated resource kind") {
            "time" => {
                exact.insert(escape(name), makespan.clone());
            }
            "count" => {
                exact.insert(
                    escape(name),
                    events
                        .iter()
                        .map(|e| e.costs.get(name).cloned().unwrap_or_else(Rational::zero))
                        .sum(),
                );
            }
            "occupancy" | "peak" => {
                for class in target.classes.keys() {
                    let intervals: Vec<_> = live.iter().filter(|l| &l.class == class).collect();
                    let value = if resource["kind"] == "occupancy" {
                        let rate = resource
                            .get("rate")
                            .and_then(|v| v.get(class))
                            .map(d::nonnegative)
                            .transpose()?
                            .unwrap_or_else(|| d::integer(1));
                        intervals
                            .iter()
                            .map(|l| {
                                let factor = if l.borrowed {
                                    target.classes[class].dirty_rate.clone()
                                } else {
                                    d::integer(1)
                                };
                                (&l.end - &l.start) * d::integer(l.width) * &rate * factor
                            })
                            .sum()
                    } else {
                        let mut changes = Vec::new();
                        for l in intervals {
                            if l.end > l.start {
                                changes.push((l.start.clone(), 1, l.width));
                                changes.push((l.end.clone(), 0, l.width));
                            }
                        }
                        changes.sort();
                        let mut count = 0u64;
                        let mut peak = 0;
                        for (_, kind, width) in changes {
                            if kind == 0 {
                                count = count
                                    .checked_sub(width)
                                    .ok_or_else(|| Error::infeasible("invalid peak sweep"))?;
                            } else {
                                count = count
                                    .checked_add(width)
                                    .ok_or_else(|| Error::infeasible("peak exceeds host u64"))?;
                                peak = peak.max(count);
                            }
                        }
                        d::integer(peak)
                    };
                    exact.insert(format!("{}[{}]", escape(name), escape(class)), value);
                }
            }
            _ => unreachable!(),
        }
    }
    let display = exact
        .iter()
        .map(|(name, q)| Ok((name.clone(), d::upper(q)?)))
        .collect::<Result<_>>()?;
    Ok(Rows { exact, display })
}

/// Aggregate a realised stream with allocation/release occurrence order. This
/// preserves live demand when several lifetime boundaries share one timestamp,
/// including gates with zero duration. Reuse is permitted after its release.
pub fn aggregate_ordered(
    target: &Target,
    events: &[TimedEvent],
    live: &[LiveInterval],
    orders: &[(u64, u64)],
) -> Result<Rows> {
    if live.len() != orders.len() {
        return Err(Error::infeasible("live interval/order count mismatch"));
    }
    let mut rows = aggregate(target, events, live)?;
    for (l, (start, end)) in live.iter().zip(orders) {
        if (l.start.clone(), *start) >= (l.end.clone(), *end) {
            return Err(Error::infeasible(
                "allocation must precede release in scheduled event order",
            ));
        }
    }
    for (name, r) in target.value["resources"].as_object().unwrap() {
        if r["kind"] != "peak" {
            continue;
        }
        for class in target.classes.keys() {
            let mut changes = Vec::new();
            for (l, (start, end)) in live.iter().zip(orders).filter(|(l, _)| &l.class == class) {
                changes.push((l.start.clone(), *start, 1, l.width));
                changes.push((l.end.clone(), *end, 0, l.width));
            }
            changes.sort();
            let mut current = 0u64;
            let mut peak = 0u64;
            for (_, _, kind, width) in changes {
                if kind == 0 {
                    current = current
                        .checked_sub(width)
                        .ok_or_else(|| Error::infeasible("invalid ordered lifetime sweep"))?;
                } else {
                    current = current
                        .checked_add(width)
                        .ok_or_else(|| Error::infeasible("ordered peak exceeds host u64"))?;
                    peak = peak.max(current);
                }
            }
            let key = format!("{}[{}]", escape(name), escape(class));
            rows.exact.insert(key.clone(), d::integer(peak));
            rows.display.insert(key, peak.to_string());
        }
    }
    Ok(rows)
}
