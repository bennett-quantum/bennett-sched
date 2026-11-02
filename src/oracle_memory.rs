//! Source bindings for the published lifted store/allocator/free recipes.
use crate::{
    Error, Result, Status,
    expansion::Env,
    object::Node,
    oracle::{Source, read, slice},
};
use serde_json::{Value, json};

pub(crate) fn literal_length(text: &str, pointer_width: u64) -> Result<num_bigint::BigUint> {
    let value = crate::layout::integer(&json!(text))
        .map_err(|e| Error::arg(format!("§3.3: invalid length literal: {}", e.reason)))?;
    if value.bits() > pointer_width {
        return Err(Error::arg("§3.3: length literal exceeds pointer width"));
    }
    Ok(value)
}

fn invoke(name: &str, args: Vec<Value>) -> Value {
    json!({"invoke":name,"args":args})
}

pub(crate) fn row(
    node: &Node,
    source: &Source<'_>,
    pc: u64,
    fault: u64,
    slots: &[u64],
    env: &Env,
) -> Result<Vec<Value>> {
    let layout = source
        .layout
        .ok_or_else(|| Error::arg("lifted memory row requires layout"))?;
    let ptr = source.object.ptr_width;
    let count = u32::try_from(layout.a_tab)
        .ok()
        .and_then(|a| 1u64.checked_shl(a))
        .ok_or_else(|| Error::new(Status::NoMem, "allocation table exceeds host size"))?;
    let atop_width = layout.a_tab + 1;
    let flag = slice(pc + slots.iter().sum::<u64>(), 1);
    let width = |name: &str| read(name, env).map(|w| w.len() as u64);
    let mut body = Vec::new();
    if node.op == "lift.alloc" {
        if node.operands.len() != 1 {
            return Err(Error::arg("lift.alloc requires one length"));
        }
        let destination = node
            .lhs
            .as_deref()
            .ok_or_else(|| Error::arg("lift.alloc requires a result"))?;
        if width(destination)? != ptr {
            return Err(Error::arg("lift.alloc result requires pointer width"));
        }
        let mut literal = None;
        let (length, actual) = if env.contains_key(&node.operands[0]) {
            (width(&node.operands[0])?, json!(node.operands[0]))
        } else {
            let value = literal_length(&node.operands[0], ptr)?;
            let length = value.bits().max(1);
            body.push(
                json!({"alloc":"allocation.length","width":length,"class":"any","state":"clean"}),
            );
            let initialization = invoke(
                "value.literal",
                vec![
                    json!(length),
                    serde_json::from_str(&value.to_string())?,
                    json!("allocation.length"),
                ],
            );
            body.push(initialization.clone());
            literal = Some(initialization);
            (length, json!("allocation.length"))
        };
        let heap_width = source
            .plan
            .base
            .iter()
            .find(|s| s.name == "heap_used")
            .ok_or_else(|| Error::arg("missing heap counter"))?
            .width;
        let extended = heap_width
            .max(length)
            .checked_add(1)
            .ok_or_else(|| Error::new(Status::NoMem, "heap comparison width exceeds host size"))?;
        let entry = ptr
            .checked_mul(2)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| Error::new(Status::NoMem, "table entry exceeds host size"))?;
        if slots.len() < 4
            || slots[0] < ptr
            || slots[1] < entry
            || slots[2] < heap_width
            || slots[3] < atop_width
        {
            return Err(Error::arg(
                "allocator displacement slots do not match source state",
            ));
        }
        body.push(invoke(
            "lift.alloc",
            vec![
                json!(ptr),
                json!(length),
                json!(heap_width),
                json!(atop_width),
                json!(extended),
                json!(ptr.min(heap_width)),
                json!(ptr.min(length)),
                json!(count),
                serde_json::from_str(&layout.heap_base.to_string())?,
                json!(source.budget[1]),
                json!(fault),
                actual,
                json!("heap_used"),
                json!("atop"),
                json!("alloctab"),
                json!(destination),
                slice(pc, ptr),
                slice(pc + slots[0], entry),
                slice(pc + slots[0] + slots[1], heap_width),
                slice(pc + slots[0] + slots[1] + slots[2], atop_width),
                flag,
                json!("faulted"),
                json!("code"),
            ],
        ));
        if let Some(initialization) = literal {
            body.push(json!({"inverse":true,"body":[initialization]}));
            body.push(json!({"release":"allocation.length"}));
        }
    } else if node.op == "lift.free" {
        if node.lhs.is_some() || node.operands.len() != 1 || width(&node.operands[0])? != ptr {
            return Err(Error::arg(
                "lift.free requires one pointer-width operand and no result",
            ));
        }
        // The catalog's free helper retains the scan signature's n/L/E/size
        // formals, but its free predicate never reads n. Bind n to p without
        // inventing a length allocation or adding leaves to the source row.
        body.push(invoke(
            "lift.free",
            vec![
                json!(ptr),
                json!(ptr),
                json!(ptr + 1),
                json!(count),
                json!(atop_width),
                json!(fault),
                json!(node.operands[0]),
                json!(node.operands[0]),
                json!("table.base"),
                json!("table.size"),
                json!("table.live"),
                json!("table.pinned"),
                json!("atop"),
                flag,
                json!("faulted"),
                json!("code"),
            ],
        ));
    } else if let Some(suffix) = node.op.strip_prefix("lift.store.i") {
        let bits = crate::schedule::uint(&json!(suffix))?;
        if node.lhs.is_some()
            || node.operands.len() != 2
            || bits == 0
            || bits > 128
            || bits % 8 != 0
            || width(&node.operands[0])? != ptr
            || width(&node.operands[1])? != bits
        {
            return Err(Error::arg(
                "lift.store requires a pointer, a matching 8..128-bit whole-byte value and no result",
            ));
        }
        let bytes = bits / 8;
        let length = u64::from(64 - bytes.leading_zeros());
        let extended = ptr.max(length).checked_add(1).ok_or_else(|| {
            Error::new(Status::NoMem, "memory comparison width exceeds host size")
        })?;
        body.push(invoke(
            "lift.store",
            vec![
                json!(ptr),
                json!(length),
                json!(extended),
                json!(count),
                json!(atop_width),
                json!(fault),
                json!(bits),
                json!(bytes),
                json!(node.operands[0]),
                json!("table.base"),
                json!("table.size"),
                json!("table.live"),
                json!("table.pinned"),
                json!("atop"),
                flag,
                json!("faulted"),
                json!("code"),
                json!(node.operands[1]),
                slice(pc, bits),
                json!("ByteLayout"),
            ],
        ));
    } else {
        return Err(Error::arg("unknown lifted memory adapter operation"));
    }
    Ok(body)
}
