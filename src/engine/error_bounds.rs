//! Common invocation bounds and decimal enclosures are separate from resource
//! displays. Only exact target-derived bounds decide requested feasibility.
use crate::{
    Error, Result, Status,
    decimal::{self as d, Rational},
    source::Trace,
};
use num_traits::Zero;
use std::collections::BTreeMap;

type Key = (String, String);
type Values = BTreeMap<Key, (Rational, Rational)>;
pub(super) struct Bounds {
    pub values: Values,
    pub exact_total: Rational,
    pub declared_total: Rational,
}
struct Tree {
    keys: Vec<Key>,
    children: Vec<Vec<usize>>,
    multiplicities: BTreeMap<Key, u64>,
}
impl Tree {
    fn compose(&self, local: &BTreeMap<Key, Rational>) -> (Values, Rational) {
        let mut values: Values = local
            .iter()
            .map(|(key, q)| (key.clone(), (q.clone(), q.clone())))
            .collect();
        let mut settled = false;
        for _ in 0..=values.len() {
            let previous = values.clone();
            for (i, key) in self.keys.iter().enumerate() {
                let inclusive = &previous[key].0
                    + self.children[i]
                        .iter()
                        .map(|child| previous[&self.keys[*child]].1.clone())
                        .sum::<Rational>();
                let bound = &mut values.get_mut(key).unwrap().1;
                *bound = bound.clone().max(inclusive);
            }
            if values == previous {
                settled = true;
                break;
            }
        }
        assert!(settled, "source call graph is acyclic");
        let sum: Rational = local
            .iter()
            .map(|(key, q)| q * d::integer(self.multiplicities[key]))
            .sum();
        let total = sum.max(values[&self.keys[0]].1.clone());
        (values, total)
    }
}
pub(super) fn form(
    trace: &Trace,
    variant: &str,
    errors: &[Rational],
    owners: &[usize],
    epsilon: &Rational,
) -> Result<Bounds> {
    let mut tree = Tree {
        keys: trace
            .invocations
            .iter()
            .map(|i| (i.hash.clone(), format!("{variant}:{}", i.controls.len())))
            .collect(),
        children: vec![vec![]; trace.invocations.len()],
        multiplicities: BTreeMap::new(),
    };
    let mut per_owner = vec![Rational::zero(); trace.invocations.len()];
    for (error, owner) in errors.iter().zip(owners) {
        per_owner[*owner] += error;
    }
    let mut local = BTreeMap::<Key, Rational>::new();
    for (i, inv) in trace.invocations.iter().enumerate() {
        if let Some(parent) = inv.parent {
            tree.children[parent].push(i);
        }
        *tree.multiplicities.entry(tree.keys[i].clone()).or_default() += 1;
        local
            .entry(tree.keys[i].clone())
            .and_modify(|q| *q = q.clone().max(per_owner[i].clone()))
            .or_insert_with(|| per_owner[i].clone());
    }
    let (_, exact_total) = tree.compose(&local);
    let mut digits = 34usize;
    loop {
        // Bound retained decimal/BigInt copies, not a source or domain width.
        if digits.saturating_mul(local.len()).saturating_mul(8) > 32 * 1024 * 1024 {
            return Err(Error::new(
                Status::NoMem,
                "error decimal enclosures exhaust host workspace",
            ));
        }
        let enclosed = local
            .iter()
            .map(|(key, q)| {
                let spelling = match d::exact_decimal(q) {
                    Some(spelling) => spelling,
                    None => d::upper_digits(q, digits)?,
                };
                Ok((key.clone(), d::parse(&spelling)?))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let (values, declared_total) = tree.compose(&enclosed);
        if declared_total <= *epsilon || exact_total >= *epsilon {
            return Ok(Bounds {
                values,
                exact_total,
                declared_total,
            });
        }
        digits = digits.saturating_mul(2);
    }
}
