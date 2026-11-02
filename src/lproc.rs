//! §3.7 finite binding closure and §3.6 canonical row/location index space.
use crate::{
    Error, Result,
    object::{self, Function, Object},
    schedule::uint,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone, Debug)]
pub struct Location {
    pub name: String,
    pub width: u64,
    pub id: u64,
}
#[derive(Clone, Debug)]
pub struct Instance {
    pub name: String,
    pub key: Vec<u64>,
    pub first: u64,
    pub rows: u64,
    pub locations: Vec<Location>,
    pub labels: BTreeMap<String, u64>,
    pub activation_cells: u64,
}
#[derive(Clone, Debug)]
pub struct IndexSpace {
    pub oracle: String,
    pub halt: u64,
    pub pc_width: u64,
    pub oracle_locations: Vec<Location>,
    pub instances: Vec<Instance>,
}
#[derive(Clone, Debug)]
pub struct FixedValues {
    pub tokens: Vec<serde_json::Value>,
    pub classical: Vec<serde_json::Value>,
    pub regions: Vec<u64>,
}
#[derive(Clone, Debug)]
enum Origin {
    Argument { index: u64, ty: String },
    Region { index: u64, address: u64, cell: u64 },
}
type Bindings = BTreeMap<String, Origin>;
fn classical_width(ty: &str, ptr: u64) -> Result<Option<u64>> {
    Ok(match ty {
        "token" => None,
        "ptr" => Some(ptr),
        "f64" => Some(64),
        _ => {
            let n = ty
                .strip_prefix('i')
                .ok_or_else(|| Error::arg("invalid classical parameter type"))?;
            let n = uint(&json!(n))?;
            if n == 0 {
                return Err(Error::arg("zero-width classical parameter"));
            }
            Some(n)
        }
    })
}
fn locations(f: &Function) -> Result<Vec<(String, u64)>> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for p in f.groups.iter().flatten().chain(&f.regs) {
        if !seen.insert(p.name.clone()) {
            return Err(Error::arg("duplicate function binding name"));
        }
    }
    for p in (if f.kind == "lproc" {
        f.groups[..2].iter().flatten().collect::<Vec<_>>()
    } else {
        vec![]
    })
    .into_iter()
    .chain(f.regs.iter())
    {
        if !p.ty.starts_with("reg[") || p.role.is_some() {
            return Err(Error::arg("§3.7: locations require reg[N] without roles"));
        }
        result.push((p.name.clone(), object::wire_width(&p.ty)?));
    }
    Ok(result)
}
fn validate_lproc(f: &Function) -> Result<()> {
    if f.kind != "lproc" || f.groups.len() != 5 || f.groups[1].len() != 1 {
        return Err(Error::arg(
            "lift.call must name a complete five-group lproc",
        ));
    }
    locations(f)?;
    if json!(
        f.groups[0]
            .iter()
            .map(|p| object::wire_width(&p.ty))
            .collect::<Result<Vec<_>>>()?
    ) != f.signature["params"]
        || json!(object::wire_width(&f.groups[1][0].ty)?) != f.signature["result"]
    {
        return Err(Error::arg("lproc location signature mismatch"));
    }
    for (index, role) in [(2, "token"), (3, "classical")] {
        for p in &f.groups[index] {
            if p.role.as_deref().is_some_and(|r| r != role)
                || (index == 2 && p.ty != "token")
                || (index == 3 && p.ty == "token")
            {
                return Err(Error::arg("lproc token/classical type or role mismatch"));
            }
        }
    }
    if json!(f.groups[2].iter().map(|p| &p.ty).collect::<Vec<_>>()) != f.signature["tokens"]
        || json!(f.groups[3].iter().map(|p| &p.ty).collect::<Vec<_>>()) != f.signature["args"]
    {
        return Err(Error::arg("lproc fixed parameter signature mismatch"));
    }
    if f.groups[4].len()
        != f.signature["mem"]
            .as_array()
            .ok_or_else(|| Error::arg("lproc region signature missing"))?
            .len()
    {
        return Err(Error::arg("lproc region count mismatch"));
    }
    for (p, sig) in f.groups[4]
        .iter()
        .zip(f.signature["mem"].as_array().unwrap())
    {
        let (a, w) = object::region_shape(&p.ty)?;
        if p.role.as_deref() != Some("in")
            || sig["role"] != "in"
            || a != uint(&sig["A"])?
            || w != uint(&sig["W"])?
        {
            return Err(Error::arg("lproc region role or shape mismatch"));
        }
    }
    Ok(())
}
fn labels(f: &Function, first: u64) -> Result<BTreeMap<String, u64>> {
    let mut labels = BTreeMap::new();
    for (i, node) in f.body.iter().enumerate() {
        if !node.bodies.is_empty() {
            return Err(Error::arg("structured source inside lproc index rows"));
        }
        let (block, ordinal) = node
            .point
            .rsplit_once(':')
            .ok_or_else(|| Error::arg("invalid source point"))?;
        if ordinal == "0" && labels.insert(block.into(), first + i as u64).is_some() {
            return Err(Error::arg("duplicate source block"));
        }
    }
    Ok(labels)
}

impl IndexSpace {
    pub fn resolve_bindings(
        &self,
        object: &Object,
        instance: &Instance,
        params: &serde_json::Value,
    ) -> Result<FixedValues> {
        object.validate_params(&self.oracle, params)?;
        if !self
            .instances
            .iter()
            .any(|i| i.name == instance.name && i.key == instance.key)
        {
            return Err(Error::arg(
                "instance is outside this oracle's binding closure",
            ));
        }
        let f = &object.functions[&instance.name];
        let tokens = f.groups[2].len();
        let classical = f.groups[3].len();
        let args = params.as_array().expect("validated argument array");
        let values = instance.key[..tokens + classical]
            .iter()
            .map(|i| {
                args.get(*i as usize)
                    .cloned()
                    .ok_or_else(|| Error::arg("fixed argument position is absent"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(FixedValues {
            tokens: values[..tokens].to_vec(),
            classical: values[tokens..].to_vec(),
            regions: instance.key[tokens + classical..].to_vec(),
        })
    }
    pub fn form(object: &Object, oracle: &str) -> Result<Self> {
        let root = object
            .functions
            .get(oracle)
            .filter(|f| f.kind == "oracle")
            .ok_or_else(|| Error::arg("index-space root must be an oracle"))?;
        let mut root_bindings = Bindings::new();
        for (i, p) in root.groups[4].iter().enumerate() {
            classical_width(&p.ty, object.ptr_width)?;
            root_bindings.insert(
                p.name.clone(),
                Origin::Argument {
                    index: i as u64,
                    ty: p.ty.clone(),
                },
            );
        }
        for (i, p) in root.groups[3].iter().enumerate() {
            let (address, cell) = object::region_shape(&p.ty)?;
            root_bindings.insert(
                p.name.clone(),
                Origin::Region {
                    index: i as u64,
                    address,
                    cell,
                },
            );
        }
        let mut queue = VecDeque::from([(oracle.to_owned(), Vec::new(), root_bindings)]);
        let mut instances = BTreeMap::<(String, Vec<u64>), Bindings>::new();
        let mut graph = BTreeMap::<String, BTreeSet<String>>::new();
        while let Some((name, _key, bindings)) = queue.pop_front() {
            let f = &object.functions[&name];
            let writable = locations(f)?.into_iter().collect::<BTreeMap<_, _>>();
            let mut readable = writable.clone();
            if f.kind == "oracle" {
                for p in &f.groups[0] {
                    readable.insert(p.name.clone(), object::wire_width(&p.ty)?);
                }
            }
            for (name, origin) in &bindings {
                if let Origin::Argument { ty, .. } = origin {
                    if let Some(width) = classical_width(ty, object.ptr_width)? {
                        readable.insert(name.clone(), width);
                    }
                }
            }
            labels(f, 0)?;
            for node in f.body.iter().filter(|n| n.op == "lift.call") {
                if ![2, 5].contains(&node.operands.len()) {
                    return Err(Error::arg("§3.7: lift.call requires one or four lists"));
                }
                let callee_name = node.operands[0]
                    .strip_prefix('@')
                    .ok_or_else(|| Error::arg("lift.call requires a symbol reference"))?;
                let callee = object
                    .functions
                    .get(callee_name)
                    .ok_or_else(|| Error::arg("unresolved lproc call"))?;
                validate_lproc(callee)?;
                graph
                    .entry(name.clone())
                    .or_default()
                    .insert(callee_name.into());
                if node.operands.len() == 2 && callee.groups[2..].iter().any(|g| !g.is_empty()) {
                    return Err(Error::arg(
                        "nonempty lproc fixed groups require all four argument lists",
                    ));
                }
                let actuals = object::list(&node.operands[1])?;
                if actuals.len() != callee.groups[0].len() {
                    return Err(Error::arg("lproc location argument count mismatch"));
                }
                for (p, actual) in callee.groups[0].iter().zip(actuals) {
                    if readable.get(&actual) != Some(&object::wire_width(&p.ty)?) {
                        return Err(Error::arg(
                            "lproc location argument width or scope mismatch",
                        ));
                    }
                }
                let destination = node
                    .lhs
                    .as_ref()
                    .and_then(|n| writable.get(n))
                    .ok_or_else(|| Error::arg("lproc result must write a containing location"))?;
                if *destination != object::wire_width(&callee.groups[1][0].ty)? {
                    return Err(Error::arg("lproc result width mismatch"));
                }
                let mut child = Bindings::new();
                let mut key = Vec::new();
                for (group, operand) in [(2, 2), (3, 3), (4, 4)] {
                    let actuals = if node.operands.len() == 5 {
                        object::list(&node.operands[operand])?
                    } else {
                        vec![]
                    };
                    if actuals.len() != callee.groups[group].len() {
                        return Err(Error::arg("lproc fixed argument count mismatch"));
                    }
                    for (parameter, actual) in callee.groups[group].iter().zip(actuals) {
                        let origin=bindings.get(&actual).ok_or_else(||Error::arg("§3.7: fixed arguments must forward entry bindings, not locations or literals"))?;
                        let index = match (group, origin) {
                            (2 | 3, Origin::Argument { index, ty }) if ty == &parameter.ty => {
                                *index
                            }
                            (
                                4,
                                Origin::Region {
                                    index,
                                    address,
                                    cell,
                                },
                            ) if object::region_shape(&parameter.ty)? == (*address, *cell) => {
                                *index
                            }
                            _ => {
                                return Err(Error::arg(
                                    "lproc fixed argument type/region-shape mismatch",
                                ));
                            }
                        };
                        key.push(index);
                        child.insert(parameter.name.clone(), origin.clone());
                    }
                }
                if instances
                    .insert((callee_name.into(), key.clone()), child.clone())
                    .is_none()
                {
                    queue.push_back((callee_name.into(), key, child));
                }
            }
        }
        let names = instances
            .keys()
            .map(|(n, _)| n.clone())
            .collect::<BTreeSet<_>>();
        fn reachable(
            name: &str,
            graph: &BTreeMap<String, BTreeSet<String>>,
            seen: &mut BTreeSet<String>,
        ) {
            if seen.insert(name.into()) {
                if let Some(next) = graph.get(name) {
                    for n in next {
                        reachable(n, graph, seen);
                    }
                }
            }
        }
        let reach = names
            .iter()
            .map(|n| {
                let mut seen = BTreeSet::new();
                reachable(n, &graph, &mut seen);
                (n.clone(), seen)
            })
            .collect::<BTreeMap<_, _>>();
        let mut assigned = BTreeSet::new();
        let mut components = Vec::new();
        for name in &names {
            if assigned.contains(name) {
                continue;
            }
            let group = names
                .iter()
                .filter(|n| reach[name].contains(*n) && reach[*n].contains(name))
                .cloned()
                .collect::<Vec<_>>();
            assigned.extend(group.clone());
            components.push((object.dispatch_component_key(&group)?, group));
        }
        components.sort_by(|a, b| a.0.cmp(&b.0));
        let mut next_id = 0;
        let mut assign_locations = |f: &Function| -> Result<Vec<Location>> {
            let mut result = Vec::new();
            for (name, width) in locations(f)? {
                result.push(Location {
                    name,
                    width,
                    id: next_id,
                });
                next_id += 1;
            }
            Ok(result)
        };
        let oracle_locations = assign_locations(root)?;
        let mut first = root.body.len() as u64;
        let mut ordered = Vec::new();
        for (_, group) in components {
            for name in group {
                for (_, key) in instances.keys().filter(|(n, _)| n == &name) {
                    let f = &object.functions[&name];
                    let local = assign_locations(f)?;
                    let rows = f.body.len() as u64;
                    ordered.push(Instance {
                        name: name.clone(),
                        key: key.clone(),
                        first,
                        rows,
                        activation_cells: 3 + local.len() as u64,
                        locations: local,
                        labels: labels(f, first)?,
                    });
                    first = first
                        .checked_add(rows)
                        .ok_or_else(|| Error::arg("index-space row count overflow"))?;
                }
            }
        }
        if first == 0 {
            return Err(Error::arg("empty oracle index space"));
        }
        Ok(Self {
            oracle: oracle.into(),
            halt: first,
            pc_width: u64::from(64 - first.leading_zeros()),
            oracle_locations,
            instances: ordered,
        })
    }
}
