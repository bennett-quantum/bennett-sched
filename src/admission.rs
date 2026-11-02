//! Source-aware §11 path and binding admission, separate from leaf realisation.
use crate::{
    Error, Request, Result, Status, canonical, decimal,
    object::{Node, Object},
    schedule::{Document, Kind, Selector, array, text, uint},
};
use num_traits::Zero;
use std::collections::{BTreeMap, BTreeSet};

fn fail(reason: impl Into<String>) -> Error {
    Error::infeasible(format!("§11: {}", reason.into()))
}
fn instruction(node: &Node) -> (&str, Vec<&str>, bool) {
    let mut op = node.op.as_str();
    let mut args = node.operands.iter().map(String::as_str).collect::<Vec<_>>();
    let mut adjoint = false;
    while op == "adj" || op == "ctl" {
        if op == "adj" {
            adjoint = !adjoint;
        } else if !args.is_empty() {
            args.remove(0);
        }
        if args.is_empty() {
            return (op, args, adjoint);
        }
        op = args.remove(0);
    }
    (op, args, adjoint)
}
fn call_targets(node: &Node, orientation: bool) -> (Vec<String>, bool) {
    let (op, args, adj) = instruction(node);
    let targets = match op {
        "action_call" | "oracle_call" | "lift.oracle_call" | "lift.oracle_uncall"
        | "lift.perm_call" => args
            .first()
            .into_iter()
            .filter_map(|a| a.strip_prefix('@'))
            .map(str::to_owned)
            .collect(),
        "qinplace" => {
            let mut t = args
                .iter()
                .take(2)
                .filter_map(|a| a.strip_prefix('@'))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if orientation ^ adj {
                t.reverse();
            }
            t
        }
        _ => vec![],
    };
    (targets, orientation ^ adj)
}
fn oracle_scope(object: &Object, root: &str) -> Result<BTreeSet<String>> {
    let mut scope = BTreeSet::from([root.to_owned()]);
    let mut pending = vec![root.to_owned()];
    while let Some(name) = pending.pop() {
        let function = object
            .functions
            .get(&name)
            .ok_or_else(|| fail("unlinked call target"))?;
        if !["oracle", "lproc"].contains(&function.kind.as_str()) {
            continue;
        }
        for node in crate::object::flattened(&function.body) {
            let (op, args, _) = instruction(node);
            if op == "lift.call" {
                let callee = args
                    .first()
                    .and_then(|a| a.strip_prefix('@'))
                    .ok_or_else(|| fail("invalid lifted call"))?;
                if scope.insert(callee.into()) {
                    pending.push(callee.into());
                }
            }
        }
    }
    Ok(scope)
}
fn source_point<'a>(nodes: &'a [Node], point: &str, depth: usize) -> Option<(&'a Node, usize)> {
    for node in nodes {
        if node.point == point {
            return Some((node, depth));
        }
        for body in &node.bodies {
            if let Some(found) = source_point(body, point, depth + usize::from(node.op == "qloop"))
            {
                return Some(found);
            }
        }
    }
    None
}

impl Document {
    /// Every selector path must identify source points and actual call edges,
    /// including entries which happen not to match a generated leaf this run.
    pub fn validate_paths(&self, object: &Object) -> Result<()> {
        let root = object
            .region(text(&self.value["bindings"]["closure_hash"])?)
            .map_err(|e| fail(e.reason))?;
        if !["oracle", "action", "perm_action"].contains(&root.kind.as_str()) {
            return Err(fail("bound root closure is not a quantum region"));
        }
        if text(&self.value["bindings"]["region"])? != root.name {
            return Err(fail("region name does not match closure binding"));
        }
        let root_scope = oracle_scope(object, &root.name)?;
        let mut index_spaces = BTreeMap::new();
        let recompute = self
            .entries
            .iter()
            .any(|e| e.kind == Kind::Cleanup && e.decision["realisation"] == "recompute");
        for entry in &self.entries {
            let mut scope = root_scope.clone();
            let mut orientation = false;
            let mut current_oracles = if root.kind == "oracle" {
                BTreeSet::from([root.name.clone()])
            } else {
                BTreeSet::new()
            };
            for (index, point) in entry.path.iter().enumerate() {
                let f = object
                    .functions
                    .values()
                    .find(|f| {
                        object.symbols[&f.name].interface_hash == point.symbol
                            && scope.contains(&f.name)
                    })
                    .ok_or_else(|| {
                        fail(format!(
                            "path symbol {} is outside the source invocation",
                            point.symbol
                        ))
                    })?;
                let reserved = point.point.starts_with('$');
                let (node, depth) = if reserved {
                    if f.kind != "oracle"
                        || !["$construct", "$step", "$commit", "$release"]
                            .contains(&point.point.as_str())
                    {
                        return Err(fail("reserved support point is not in an oracle"));
                    }
                    (None, 0)
                } else {
                    let (node, depth) =
                        source_point(&f.body, &point.point, 0).ok_or_else(|| {
                            fail(format!("path point {} is not in {}", point.point, f.name))
                        })?;
                    (Some(node), depth)
                };
                if let Selector::Exact(iteration) = &point.iteration {
                    if iteration.len() != depth {
                        return Err(fail("source iteration selector has wrong loop depth"));
                    }
                }
                if let Selector::Exact(instance) = &point.instance {
                    if f.kind == "lproc" {
                        let mut formed = false;
                        for oracle in &current_oracles {
                            if !index_spaces.contains_key(oracle) {
                                index_spaces.insert(
                                    oracle.clone(),
                                    crate::lproc::IndexSpace::form(object, oracle)
                                        .map_err(|e| fail(e.reason))?,
                                );
                            }
                            formed |= index_spaces[oracle]
                                .instances
                                .iter()
                                .any(|i| i.name == f.name && i.key == *instance);
                        }
                        if !formed {
                            return Err(fail(
                                "concrete lproc instance is absent from the source binding closure",
                            ));
                        }
                    } else if !instance.is_empty() {
                        return Err(fail(
                            "non-lproc source point requires the empty instance key",
                        ));
                    }
                }
                let (targets, next_orientation) = node
                    .map(|n| call_targets(n, orientation))
                    .unwrap_or_default();
                let selected = if let Selector::Exact(call_index) = point.call_index {
                    if let Some(target) = targets.get(call_index as usize) {
                        vec![target.clone()]
                    } else {
                        if recompute {
                            return Err(Error::new(
                                Status::GateUnsupported,
                                "§11 K80/K81: recompute call occurrence admission requires source cleanup partition (bennett-sched-ark)",
                            ));
                        }
                        return Err(fail("call_index is not in the bound source expansion"));
                    }
                } else {
                    targets
                };
                if index + 1 < entry.path.len() {
                    if selected.is_empty() {
                        return Err(fail("path descends through a non-call source point"));
                    }
                    scope = BTreeSet::new();
                    current_oracles = BTreeSet::new();
                    for target in selected {
                        if object
                            .functions
                            .get(&target)
                            .is_some_and(|f| f.kind == "oracle")
                        {
                            current_oracles.insert(target.clone());
                        }
                        scope.extend(oracle_scope(object, &target)?);
                    }
                    orientation = if scope.iter().any(|n| object.functions[n].kind == "oracle") {
                        false
                    } else {
                        next_orientation
                    };
                }
            }
        }
        Ok(())
    }
    pub fn validate_request(&self, request: &Request) -> Result<()> {
        let object = request
            .object
            .as_ref()
            .ok_or_else(|| Error::arg("request object is unset"))?;
        let target = request
            .target
            .as_ref()
            .ok_or_else(|| Error::arg("request target is unset"))?;
        let b = &self.value["bindings"];
        if Some(text(&b["closure_hash"])?) != request.region.as_deref()
            || Some(text(&b["variant"])?) != request.variant.as_deref()
            || text(&b["target_hash"])? != target.hash
            || uint(&b["inherited_controls"])? != request.inherited_controls as u64
        {
            return Err(fail("schedule/request identity binding mismatch"));
        }
        let budget = array(&b["budget"])?
            .iter()
            .map(uint)
            .collect::<Result<Vec<_>>>()?;
        if request.budget.as_ref().map(|b| b.as_slice()) != Some(budget.as_slice()) {
            return Err(fail("schedule/request budget binding mismatch"));
        }
        let params = request
            .params
            .as_ref()
            .ok_or_else(|| Error::arg("request params are unset"))?;
        if text(&b["params_hash"])? != canonical::hash(&canonical::to_vec(params)?)
            || decimal::value(&b["epsilon"])?
                != request
                    .epsilon
                    .clone()
                    .unwrap_or_else(decimal::Rational::zero)
        {
            return Err(fail(
                "schedule/request parameter or accuracy binding mismatch",
            ));
        }
        if target.value.get("topology").is_none()
            && (self.value.get("interface_vertices").is_some()
                || self
                    .entries
                    .iter()
                    .any(|e| e.kind == Kind::Alloc && e.decision.get("vertices").is_some()))
        {
            return Err(fail("placements must be absent without a target topology"));
        }
        self.validate_paths(object)
    }
}
