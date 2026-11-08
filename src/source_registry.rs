//! Source-body builders and K77's explicit, captured, non-positional channel.
use crate::{
    Error, Result,
    expansion::{Binding, Library, Operation, Symbol},
    object::{Object, region_shape, wire_width},
    source::Trace,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleInvocation {
    token: [u64; 4],
}
impl OracleInvocation {
    pub fn parse(value: &Value) -> Result<Self> {
        crate::schedule::fields(value, &["token"], &[])?;
        crate::expansion::validate_value(&json!("token"), &value["token"])?;
        let mut token = [0; 4];
        for (out, value) in token.iter_mut().zip(value["token"].as_array().unwrap()) {
            *out = crate::schedule::uint(value)?;
        }
        Ok(Self { token })
    }
    pub fn token(&self) -> [u64; 4] {
        self.token
    }
}
#[derive(Clone)]
pub struct SourceOrigin {
    pub trace: Arc<Trace>,
    pub event: usize,
}
impl std::fmt::Debug for SourceOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceOrigin")
            .field("closure", &self.trace.invocations[0].hash)
            .field("event", &self.event)
            .finish()
    }
}
impl PartialEq for SourceOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.event == other.event && Arc::ptr_eq(&self.trace, &other.trace)
    }
}
impl Eq for SourceOrigin {}
#[derive(Clone, Debug)]
pub(crate) struct Definition {
    object: Arc<Object>,
    name: String,
}
impl Library {
    /// The linker supplies parsed source and its independently bound closure.
    /// No source context or constructor is added to the ref catalog.
    pub fn link_source(&mut self, object: Arc<Object>, name: &str) -> Result<String> {
        let function = object
            .functions
            .get(name)
            .ok_or_else(|| Error::arg("unresolved linked source symbol"))?;
        if !["oracle", "action", "perm_action"].contains(&function.kind.as_str()) {
            return Err(Error::arg(
                "linked source registry requires a region closure",
            ));
        }
        let hash = object.closure_hash(name)?.to_owned();
        if self.symbols.contains_key(&hash) {
            return Err(Error::arg("duplicate source closure registration"));
        }
        let oracle = function.kind == "oracle";
        let wire_groups = if oracle { 3 } else { 2 };
        let mut parameters = Vec::new();
        for parameter in function.groups[..wire_groups].iter().flatten() {
            parameters.push(
                json!({"name":parameter.name,"type":"wire","width":wire_width(&parameter.ty)?}),
            );
        }
        for p in &function.groups[wire_groups] {
            let (address, cell) = region_shape(&p.ty)?;
            parameters.push(
                json!({"name":p.name,"type":"region","address":address,"cell":cell,"role":p.role}),
            );
        }
        for p in &function.groups[wire_groups + 1] {
            let isa_type = if p.ty == "ptr" {
                json!({"ptr":object.ptr_width})
            } else {
                json!(p.ty)
            };
            parameters.push(json!({"name":p.name,"type":"value","isa_type":isa_type}));
        }
        self.symbols.insert(
            hash.clone(),
            Symbol {
                parameters,
                body: vec![],
                source: Some(Definition {
                    object,
                    name: name.into(),
                }),
            },
        );
        Ok(hash)
    }
}
impl Definition {
    pub(crate) fn build(
        &self,
        values: &[Binding],
        context: Option<&OracleInvocation>,
        oracle_identity: Option<u64>,
        controls: &[String],
        identity: &str,
    ) -> Result<Arc<Trace>> {
        let f = &self.object.functions[&self.name];
        let oracle = f.kind == "oracle";
        let wire_groups = if oracle { 3 } else { 2 };
        let count = f.groups.iter().map(Vec::len).sum::<usize>();
        if values.len() != count {
            return Err(Error::arg("linked source argument count mismatch"));
        }
        let budget = if oracle {
            context
                .ok_or_else(|| Error::arg("K77: missing explicit oracle_invocation channel"))?
                .token
        } else {
            if context.is_some() {
                return Err(Error::arg(
                    "oracle_invocation channel on an action source builder",
                ));
            }
            [0; 4]
        };
        self.object.validate_budget(budget)?;
        let mut token_ids = if oracle {
            vec![oracle_identity.ok_or_else(|| {
                Error::arg("K91: linked oracle requires an explicit resolved primary identity")
            })?]
        } else {
            if oracle_identity.is_some() {
                return Err(Error::arg(
                    "primary oracle identity on an action source builder",
                ));
            }
            vec![]
        };
        let wires = f.groups[..wire_groups].iter().map(Vec::len).sum::<usize>();
        let regions = f.groups[wire_groups].len();
        let params = values[wires + regions..]
            .iter()
            .map(|b| match b {
                Binding::Value { value, .. } => Ok(value.clone()),
                _ => Err(Error::arg("linked source classical argument type mismatch")),
            })
            .collect::<Result<Vec<_>>>()?;
        for (parameter, binding) in f.groups[wire_groups + 1]
            .iter()
            .zip(&values[wires + regions..])
        {
            if parameter.ty == "token" {
                token_ids.push(crate::token_aliases::binding_identity(binding)?);
            }
        }
        let mut trace = Trace::build_with_token_ids(
            &self.object,
            &self.name,
            u32::try_from(controls.len())
                .map_err(|_| Error::arg("source control count exceeds uint32"))?,
            &json!(params),
            budget,
            false,
            &token_ids,
        )?;
        trace.object = self.object.clone();
        let mut names = BTreeMap::new();
        for (old, new) in trace.inherited.iter().zip(controls) {
            names.insert(old.clone(), new.clone());
        }
        let mut used = BTreeSet::new();
        for (slot, actual) in trace.slots.iter().zip(&values[..wires + regions]) {
            let actual = match actual {
                Binding::Wires(w)
                | Binding::Allocation { wires: w, .. }
                | Binding::Region { wires: w, .. } => w,
                _ => return Err(Error::arg("linked source quantum argument type mismatch")),
            };
            if slot.wires.len() != actual.len() {
                return Err(Error::arg("linked source quantum argument width mismatch"));
            }
            for (old, new) in slot.wires.iter().zip(actual) {
                if !used.insert(new.clone())
                    || (["mut", "tgt", "fault", "mem_mut"].contains(&slot.role.as_str())
                        && controls.contains(new))
                {
                    return Err(Error::arg(
                        "linked source interfaces overlap or write a control",
                    ));
                }
                names.insert(old.clone(), new.clone());
            }
        }
        for e in &trace.events {
            if let Operation::Alloc {
                identity: old,
                wires,
                ..
            } = &e.leaf.op
            {
                names.insert(old.clone(), format!("{identity}/{old}"));
                for w in wires {
                    names.insert(w.clone(), format!("{identity}/{w}"));
                }
            }
        }
        let map = |w: &str| names.get(w).cloned().unwrap_or_else(|| w.into());
        trace.inherited = trace.inherited.iter().map(|w| map(w)).collect();
        for slot in &mut trace.slots {
            slot.wires = slot.wires.iter().map(|w| map(w)).collect();
        }
        for inv in &mut trace.invocations {
            inv.controls = inv.controls.iter().map(|w| map(w)).collect();
            for slot in &mut inv.slots {
                slot.wires = slot.wires.iter().map(|w| map(w)).collect();
            }
        }
        for event in &mut trace.events {
            match &mut event.leaf.op {
                Operation::Gate(g) => {
                    for w in g.wires.iter_mut().chain(&mut g.controls) {
                        *w = map(w);
                    }
                }
                Operation::Alloc {
                    identity, wires, ..
                }
                | Operation::Release {
                    identity, wires, ..
                } => {
                    *identity = map(identity);
                    for w in wires {
                        *w = map(w);
                    }
                }
                Operation::Classical { .. } => {
                    return Err(Error::arg(
                        "classical operation in linked source quantum body",
                    ));
                }
            }
        }
        for proof in &mut trace.qinplaces {
            proof.target_id = map(&proof.target_id);
            proof.fault_id = map(&proof.fault_id);
            for wires in [&mut proof.target, &mut proof.inputs, &mut proof.controls] {
                for w in wires {
                    *w = map(w);
                }
            }
        }
        for pair in &mut trace.cleanups {
            for w in &mut pair.targets {
                *w = map(w);
            }
        }
        for proof in &mut trace.borrow_proofs {
            if let crate::borrow::Evidence::Translation { witness, .. } = &mut proof.evidence {
                map_strings(witness, &names);
            }
        }
        Ok(Arc::new(trace))
    }
}
fn map_strings(value: &mut Value, names: &BTreeMap<String, String>) {
    match value {
        Value::String(w) => {
            if let Some(new) = names.get(w) {
                *w = new.clone();
            }
        }
        Value::Array(a) => {
            for v in a {
                map_strings(v, names);
            }
        }
        Value::Object(o) => {
            for v in o.values_mut() {
                map_strings(v, names);
            }
        }
        _ => {}
    }
}

/// Non-emitting source call metadata also covers zero-native-leaf callees.
#[derive(Clone)]
pub struct SourceCall {
    pub trace: Arc<Trace>,
    pub start: usize,
    pub end: usize,
    pub reverse: bool,
    pub calls: Vec<crate::expansion::CallEdge>,
    pub origin: Vec<crate::expansion::Origin>,
}
impl std::fmt::Debug for SourceCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceCall")
            .field("closure", &self.trace.invocations[0].hash)
            .field("start", &self.start)
            .field("end", &self.end)
            .field("reverse", &self.reverse)
            .finish()
    }
}
#[derive(Clone, Debug)]
pub struct Expanded {
    pub leaves: Vec<crate::expansion::Leaf>,
    pub source_calls: Vec<SourceCall>,
}
