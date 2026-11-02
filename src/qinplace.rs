//! K4 finite inverse-pair verification. No input slice is omitted to fit a host.
use crate::{
    Error, Result, Status,
    expansion::simulate_basis,
    object::{self, Object},
    source::Trace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct Proof {
    pub owner: usize,
    pub g: String,
    pub ginv: String,
    pub budget: [u64; 4],
    pub params: Value,
    pub domain: Value,
    pub arg_tokens: Value,
    pub target_id: String,
    pub fault_id: String,
    pub target: Vec<String>,
    pub inputs: Vec<String>,
    pub controls: Vec<String>,
}
fn memory(reason: &str) -> Error {
    Error::new(
        Status::NoMem,
        format!("inverse-pair proof host workspace: {reason}"),
    )
}
fn table(trace: &Trace, cases: usize, name: &str) -> Result<Vec<usize>> {
    let inputs = trace
        .slots
        .iter()
        .filter(|s| s.role == "in" || s.role == "mem_in")
        .flat_map(|s| s.wires.clone())
        .collect::<Vec<_>>();
    let target = trace
        .slots
        .iter()
        .find(|s| s.role == "tgt")
        .ok_or_else(|| Error::arg("missing inverse-pair target"))?;
    let faults = trace
        .slots
        .iter()
        .find(|s| s.role == "fault")
        .ok_or_else(|| Error::arg("missing inverse-pair fault slot"))?;
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    out.try_reserve_exact(cases)
        .map_err(|_| memory("result table allocation"))?;
    for input in 0..cases {
        let mut bits = trace
            .slots
            .iter()
            .flat_map(|s| s.wires.iter().cloned())
            .map(|w| (w, false))
            .collect::<BTreeMap<_, _>>();
        for (bit, wire) in inputs.iter().enumerate() {
            bits.insert(wire.clone(), input & (1 << bit) != 0);
        }
        simulate_basis(&leaves, &mut bits)?;
        if faults.wires.iter().any(|w| bits[w]) {
            return Err(Error::infeasible(format!(
                "§4.5: inverse pair oracle {name} faults at input {input}"
            )));
        }
        if bits.len() != trace.slots.iter().map(|s| s.wires.len()).sum::<usize>()
            || inputs
                .iter()
                .enumerate()
                .any(|(bit, w)| bits[w] != (input & (1 << bit) != 0))
        {
            return Err(Error::infeasible(
                "§4.5: inverse pair changes read-only inputs or leaves private state",
            ));
        }
        let value = target
            .wires
            .iter()
            .enumerate()
            .map(|(bit, w)| usize::from(bits[w]) << bit)
            .sum();
        out.push(value);
    }
    Ok(out)
}
pub fn verify(
    object: &Object,
    g: &str,
    h: &str,
    budget: [u64; 4],
    params: &Value,
) -> Result<(Value, Value)> {
    let f = &object.functions[g];
    object.validate_params(g, params)?;
    object.validate_params(h, params)?;
    let mut count = 0u64;
    let mut bundles = Vec::new();
    let mut regions = Vec::new();
    let mut args = Vec::new();
    let mut tokens = serde_json::Map::new();
    for p in &f.groups[0] {
        let width = object::wire_width(&p.ty)?;
        count = count
            .checked_add(width)
            .ok_or_else(|| memory("domain size"))?;
        if count > 20 {
            return Err(memory(
                "complete domain exceeds the 16 MiB two-table budget",
            ));
        }
        bundles.push(json!({"name":p.name,"width":width,"lo":0,"hi":((num_bigint::BigUint::from(1u8)<<width as usize)-num_bigint::BigUint::from(1u8)).to_string()}));
    }
    for p in &f.groups[3] {
        let (a, w) = object::region_shape(&p.ty)?;
        let width = u32::try_from(a)
            .ok()
            .and_then(|a| 1u64.checked_shl(a))
            .and_then(|n| n.checked_mul(w))
            .ok_or_else(|| memory("region domain size"))?;
        count = count
            .checked_add(width)
            .ok_or_else(|| memory("region domain size"))?;
        if count > 20 {
            return Err(memory(
                "complete region domain exceeds the 16 MiB two-table budget",
            ));
        }
        regions.push(json!({"name":p.name,"cells":"all"}));
    }
    for (p, value) in f.groups[4].iter().zip(params.as_array().unwrap()) {
        if p.ty == "token" {
            tokens.insert(p.name.clone(), value.clone());
            continue;
        }
        let (width, bits) = match p.ty.as_str() {
            "ptr" => (object.ptr_width, value.clone()),
            "f64" => (
                64,
                json!(
                    u64::from_str_radix(&value.as_str().unwrap()[2..], 16)
                        .map_err(|_| Error::arg("invalid f64 inverse-pair argument"))?
                        .to_string()
                ),
            ),
            ty => (
                crate::schedule::uint(&json!(
                    ty.strip_prefix('i')
                        .ok_or_else(|| Error::arg("invalid inverse-pair argument type"))?
                ))?,
                value.clone(),
            ),
        };
        args.push(json!({"name":p.name,"width":width,"lo":bits,"hi":bits}));
    }
    let cases = 1usize
        .checked_shl(count as u32)
        .ok_or_else(|| memory("domain exponent"))?;
    let gt = Trace::build(object, g, 0, params, budget)?;
    let ht = Trace::build(object, h, 0, params, budget)?;
    if (gt.events.len() as u128 + ht.events.len() as u128) * (cases as u128) > 100_000_000 {
        return Err(memory(
            "complete exhaustive replay exceeds 100000000 leaf work credits",
        ));
    }
    let gv = table(&gt, cases, g)?;
    let hv = table(&ht, cases, h)?;
    let n = object::wire_width(&f.groups[0][0].ty)? as usize;
    let mask = (1usize << n) - 1;
    for i in 0..cases {
        let context = i & !mask;
        let x = i & mask;
        if hv[context | gv[i]] != x || gv[context | hv[i]] != x {
            return Err(Error::infeasible(format!(
                "§4.5: inverse pair {g}/{h} fails a composition at input {i}"
            )));
        }
    }
    Ok((
        json!({"bundles":bundles,"borrowed":[],"regions":regions,"args":args}),
        Value::Object(tokens),
    ))
}
