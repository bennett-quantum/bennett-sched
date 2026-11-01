//! K82 named wire interfaces. Typed recipes and templates remain closed.
use crate::{
    Error, Result,
    expansion::{Binding, Env, Library},
};
use serde_json::Value;
use std::collections::BTreeSet;

fn named(recipe: &Value) -> bool {
    recipe["parameters"]
        .as_array()
        .is_some_and(|p| p.iter().all(Value::is_string))
}
fn use_wire(v: &Value, bound: &BTreeSet<String>, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let base = s.split('[').next().unwrap();
            if !bound.contains(base) && !out.iter().any(|n| n == base) {
                out.push(base.into());
            }
        }
        Value::Array(a) => {
            for v in a {
                use_wire(v, bound, out);
            }
        }
        Value::Object(o) => {
            if let Some(v) = o.get("range") {
                use_wire(v, bound, out);
            }
        }
        _ => {}
    }
}
fn scan(
    lib: &Library,
    body: &[Value],
    bound: &mut BTreeSet<String>,
    out: &mut Vec<String>,
    active: &mut BTreeSet<String>,
) -> Result<()> {
    for c in body {
        if c.get("gate").is_some() {
            use_wire(&c["wires"], bound, out);
        }
        if c.get("image").is_some() {
            use_wire(&c["wires"], bound, out);
        }
        for key in ["release", "acquire", "relinquish", "under"] {
            if let Some(v) = c.get(key) {
                use_wire(v, bound, out);
            }
        }
        if c.get("each").is_some() {
            use_wire(&c["in"], bound, out);
        }
        if let Some(name) = c.get("invoke").and_then(Value::as_str) {
            let recipe = lib
                .recipes
                .get(name)
                .ok_or_else(|| Error::arg("unbound named-interface dependency"))?;
            for name in collect(lib, name, active)? {
                use_wire(&Value::String(name), bound, out);
            }
            if let Some(args) = c["args"].as_array() {
                for (formal, actual) in recipe["parameters"].as_array().unwrap().iter().zip(args) {
                    match formal.get("type").and_then(Value::as_str) {
                        Some("wire" | "region" | "environment" | "allocation") => {
                            use_wire(actual, bound, out)
                        }
                        Some("body") => {
                            if let Some(body) = actual.as_array() {
                                scan(lib, body, &mut bound.clone(), out, active)?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(name) = c.get("alloc").and_then(Value::as_str) {
            bound.insert(name.into());
        }
        if let Some(body) = c.get("body").and_then(Value::as_array) {
            let mut local = bound.clone();
            for key in ["for", "each", "declare"] {
                if let Some(name) = c.get(key).and_then(Value::as_str) {
                    local.insert(name.into());
                }
            }
            scan(lib, body, &mut local, out, active)?;
        }
    }
    Ok(())
}
fn collect(lib: &Library, name: &str, active: &mut BTreeSet<String>) -> Result<Vec<String>> {
    let recipe = lib
        .recipes
        .get(name)
        .ok_or_else(|| Error::arg("unknown named-interface recipe"))?;
    if !named(recipe) {
        return Ok(vec![]);
    }
    if !active.insert(name.into()) {
        return Err(Error::arg("cyclic named-interface dependency"));
    }
    let mut bound = recipe["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let mut out = Vec::new();
    scan(
        lib,
        recipe["body"].as_array().unwrap(),
        &mut bound,
        &mut out,
        active,
    )?;
    active.remove(name);
    Ok(out)
}
pub fn interface(lib: &Library, name: &str) -> Result<Vec<String>> {
    collect(lib, name, &mut BTreeSet::new())
}

pub(crate) fn named_scope(
    lib: &Library,
    recipe: &Value,
    mut bound: Env,
    caller: &Env,
    direct: bool,
) -> Result<Env> {
    if !named(recipe) {
        return Ok(bound);
    }
    let names = interface(lib, recipe["kind"].as_str().unwrap())?;
    if direct && (caller.len() != names.len() || caller.keys().any(|k| !names.contains(k))) {
        return Err(Error::arg(
            "§11.1 K82: direct named-interface instantiation requires exactly its wire map",
        ));
    }
    for name in names {
        let binding = caller
            .get(&name)
            .filter(|b| {
                matches!(
                    b,
                    Binding::Wires(_) | Binding::Region { .. } | Binding::Allocation { .. }
                )
            })
            .ok_or_else(|| Error::arg(format!("§11.1 K82: unbound named-interface wire {name}")))?;
        bound.insert(name, binding.clone());
    }
    Ok(bound)
}
