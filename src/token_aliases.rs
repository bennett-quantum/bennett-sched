//! K91 equality partitions contain identities, never process-local handles.
use crate::{Error, Result, canonical, object::Function, schedule};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn parse(bytes: &[u8]) -> Result<Vec<u64>> {
    let value = canonical::require_jcs(bytes)?;
    let aliases = schedule::array(&value)?
        .iter()
        .map(|v| {
            v.as_u64()
                .ok_or_else(|| Error::arg("token_aliases labels must be JSON nonnegative integers"))
        })
        .collect::<Result<Vec<_>>>()?;
    validate_partition(&aliases)?;
    Ok(aliases)
}
pub fn validate_partition(aliases: &[u64]) -> Result<()> {
    let mut next = 0;
    for label in aliases {
        if *label > next {
            return Err(Error::arg(
                "token_aliases labels must follow first occurrence from zero without gaps",
            ));
        }
        if *label == next {
            next += 1;
        }
    }
    Ok(())
}
fn tuples(function: &Function, params: &Value, budget: [u64; 4]) -> Result<Vec<[u64; 4]>> {
    let oracle = function.kind == "oracle";
    if !oracle && !["action", "perm_action"].contains(&function.kind.as_str()) {
        return Err(Error::arg(
            "token_aliases scheduling subject must be a region",
        ));
    }
    let mut tuples = if oracle { vec![budget] } else { vec![] };
    for (param, value) in function.groups[if oracle { 4 } else { 3 }]
        .iter()
        .zip(schedule::array(params)?)
    {
        if param.ty == "token" {
            let token = schedule::array(value)?
                .iter()
                .map(schedule::uint)
                .collect::<Result<Vec<_>>>()?;
            tuples.push(
                token
                    .try_into()
                    .map_err(|_| Error::arg("token_aliases token tuple arity"))?,
            );
        }
    }
    Ok(tuples)
}
pub fn validate_ids(
    function: &Function,
    params: &Value,
    budget: [u64; 4],
    ids: &[u64],
) -> Result<()> {
    let tuples = tuples(function, params, budget)?;
    if ids.len() != tuples.len() {
        return Err(Error::arg(
            "token_aliases arity does not match primary and argument tokens",
        ));
    }
    let mut identities = BTreeMap::new();
    for (id, tuple) in ids.iter().zip(tuples) {
        if identities
            .insert(*id, tuple)
            .is_some_and(|previous| previous != tuple)
        {
            return Err(Error::arg(
                "token_aliases equal identities have different token tuples",
            ));
        }
    }
    Ok(())
}
pub fn resolve(
    function: &Function,
    params: &Value,
    budget: [u64; 4],
    aliases: Option<&[u64]>,
) -> Result<Vec<u64>> {
    let ids = if let Some(aliases) = aliases {
        validate_partition(aliases)?;
        aliases.to_vec()
    } else {
        let primary = usize::from(function.kind == "oracle");
        if tuples(function, params, budget)?.len() != primary {
            return Err(Error::arg(
                "K91: token arguments require an explicit token_aliases binding",
            ));
        }
        (0..primary as u64).collect()
    };
    validate_ids(function, params, budget, &ids)?;
    Ok(ids)
}
pub fn partition(ids: &[u64]) -> Vec<u64> {
    let mut labels = BTreeMap::new();
    ids.iter()
        .map(|id| {
            let next = labels.len() as u64;
            *labels.entry(*id).or_insert(next)
        })
        .collect()
}
