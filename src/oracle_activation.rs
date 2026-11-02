//! Source-validated arguments for the catalog's §3.7 activation recipes.
use crate::{
    Error, Result, Status,
    expansion::Env,
    object,
    oracle::{Source, read, slice},
};
use serde_json::{Value, json};
fn invoke(name: &str, args: Vec<Value>) -> Value {
    json!({"invoke":name,"args":args})
}
fn locations(list: &[crate::oracle_state::Location]) -> Value {
    json!(
        list.iter()
            .map(|l| json!({"id":l.id,"width":l.width,"slot":l.slot,"location":l.storage}))
            .collect::<Vec<_>>()
    )
}
pub(crate) fn row(
    node: &object::Node,
    source: &Source<'_>,
    p: u64,
    f: u64,
    slots: &[u64],
    env: &Env,
) -> Result<(Value, u64, bool)> {
    let plan = source.plan;
    let layout = source
        .layout
        .ok_or_else(|| Error::arg("activation requires layout"))?;
    let sc = layout.s_cells + 1;
    let dc = plan
        .base
        .iter()
        .find(|r| r.name == "depth")
        .ok_or_else(|| Error::arg("missing depth counter"))?
        .width;
    let count = 1u64
        .checked_shl(
            u32::try_from(layout.s_cells)
                .map_err(|_| Error::new(Status::NoMem, "stack exponent exceeds host size"))?,
        )
        .ok_or_else(|| Error::new(Status::NoMem, "stack exceeds host size"))?;
    let flag = slice(p + slots.iter().sum::<u64>(), 1);
    let succ = slice(p + slots.iter().sum::<u64>() + 1, p);
    if node.op == "lift.call" {
        let call = plan.call(source.object, source.instance, source.original)?;
        let target = &plan.index.instances[call.callee];
        // Reading immutable pseudo-locations needs the separately tracked K25
        // binding decision; never silently materialize a classical value here.
        for capture in &call.captures {
            if read(&capture.source, env)?.len() as u64 != capture.width {
                return Err(Error::arg("capture source width mismatch"));
            }
        }
        let captures=json!(call.captures.iter().map(|c|json!({"position":c.position,"width":c.width,"source":c.source,"parameter":c.parameter,"capture":slice(c.frame_offset,c.width)})).collect::<Vec<_>>());
        let m = call.captures.len();
        let offset = p + slots[..m].iter().sum::<u64>();
        let extended = sc.max(64 - target.activation_cells.leading_zeros() as u64) + 1;
        let body = invoke(
            "lift.call",
            vec![
                json!(dc),
                json!(sc),
                json!(extended),
                json!(target.activation_cells),
                json!(count),
                json!(plan.cell_width),
                json!(p),
                json!(plan.id_width),
                json!(f),
                json!(source.budget[3]),
                json!(source.budget[2]),
                json!(call.row + 1),
                json!(call.destination_id),
                json!("depth"),
                json!("sp"),
                json!("fp"),
                json!("stack"),
                slice(offset, sc),
                slice(offset + slots[m], sc),
                slice(offset + slots[m] + slots[m + 1], dc),
                flag,
                json!("faulted"),
                json!("code"),
                locations(&call.locations),
                captures,
            ],
        );
        Ok((body, target.first, false))
    } else {
        let index = source
            .instance
            .ok_or_else(|| Error::arg("lift.ret outside lproc"))?;
        if node.operands.len() != 1 {
            return Err(Error::arg("lift.ret needs one value"));
        }
        let value = &node.operands[0];
        let width = read(value, env)?.len() as u64;
        if width != object::wire_width(&source.function.groups[1][0].ty)? {
            return Err(Error::arg("lift.ret value does not match result width"));
        }
        let ids = &plan.index.instances[index].locations;
        let selected = plan
            .locations
            .iter()
            .filter(|l| ids.iter().any(|id| id.id == l.id))
            .cloned()
            .collect::<Vec<_>>();
        let offset = p + slots[0];
        let body = invoke(
            "lift.ret",
            vec![
                json!(dc),
                json!(sc),
                json!(count),
                json!(plan.cell_width),
                json!(p),
                json!(plan.id_width),
                json!(f),
                json!(slots[0]),
                json!(width),
                json!("depth"),
                json!("fp"),
                json!("stack"),
                slice(offset, sc),
                slice(offset + slots[1], dc),
                flag,
                json!("faulted"),
                json!("code"),
                json!(value),
                slice(p, slots[0]),
                succ,
                locations(&selected),
                locations(&plan.locations),
            ],
        );
        Ok((body, 0, true))
    }
}
