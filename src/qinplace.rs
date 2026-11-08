//! K4 finite inverse-pair verification. No input slice is omitted to fit a host.
use crate::{
    Error, Result, Status,
    expansion::{Binding, Env, simulate_basis},
    object::{self, Function, Object},
    source::Trace,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, str::FromStr};
#[derive(Clone, Debug)]
pub struct Proof {
    pub owner: usize,
    pub g: String,
    pub ginv: String,
    pub budget: [u64; 4],
    pub params: Value,
    pub domain: Value,
    pub arg_tokens: Value,
    pub token_ids: Vec<u64>,
    pub execution: Option<Execution>,
}
#[derive(Clone, Debug)]
pub struct Execution {
    pub target_id: String,
    pub fault_id: String,
    pub target: Vec<String>,
    pub inputs: Vec<String>,
    pub controls: Vec<String>,
}
pub(crate) fn bind(
    object: &Object,
    g: &Function,
    args: &[String],
    actuals: &[String],
    env: &Env,
) -> Result<([u64; 4], Value, Vec<u64>)> {
    let Some(Binding::Value {
        isa_type, value, ..
    }) = env.get(&args[2])
    else {
        return Err(Error::arg("qinplace requires a token"));
    };
    if isa_type != &json!("token") {
        return Err(Error::arg("qinplace requires a token"));
    }
    let budget: [u64; 4] = crate::schedule::array(value)?
        .iter()
        .map(crate::schedule::uint)
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| Error::arg("invalid qinplace token"))?;
    object.validate_budget(budget)?;
    let mut token_ids = vec![crate::token_aliases::binding_identity(&env[&args[2]])?];
    let mut params = Vec::new();
    for (p, a) in g.groups[4].iter().zip(actuals) {
        let Some(Binding::Value {
            isa_type, value, ..
        }) = env.get(a)
        else {
            return Err(Error::arg("qinplace classical argument is not a value"));
        };
        if isa_type != &json!(p.ty) {
            return Err(Error::arg("qinplace classical type mismatch"));
        }
        params.push(value.clone());
        if p.ty == "token" {
            token_ids.push(crate::token_aliases::binding_identity(&env[a])?);
        }
    }
    let params = json!(params);
    Ok((budget, params, token_ids))
}

mod inventory;
pub(crate) use inventory::complete;
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
pub(crate) fn verify_with_ids(
    object: &Object,
    g: &str,
    h: &str,
    budget: [u64; 4],
    params: &Value,
    token_ids: &[u64],
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
        bundles.push(json!({"name":p.name,"width":width,"lo":0,"hi":crate::canonical::unsigned_integer(&((num_bigint::BigUint::from(1u8)<<width as usize)-num_bigint::BigUint::from(1u8)))}));
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
        let bits =
            num_bigint::BigUint::from_str(bits.as_str().expect("validated argument bit encoding"))
                .map_err(|_| Error::arg("invalid inverse-pair argument bits"))?;
        let encoded = crate::canonical::unsigned_integer(&bits);
        args.push(json!({"name":p.name,"width":width,"lo":encoded,"hi":encoded}));
    }
    let cases = 1usize
        .checked_shl(count as u32)
        .ok_or_else(|| memory("domain exponent"))?;
    let gt = Trace::build_with_token_ids(object, g, 0, params, budget, false, token_ids)?;
    let ht = Trace::build_with_token_ids(object, h, 0, params, budget, false, token_ids)?;
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
