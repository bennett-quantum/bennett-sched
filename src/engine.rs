//! Deterministic sequential scheduler and bound resource/certificate emitter.
use crate::{
    Error, Result, canonical,
    decimal::{self as d, Rational},
    expansion::Operation,
    request::{Outcome, Request, Response},
    resources::{self, LiveInterval, Rows, TimedEvent},
    schedule::{self, Occurrence},
    source::{Event, Slot, Trace},
    target::Target,
};
use num_traits::Zero;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CString,
};
#[derive(Clone, Debug)]
struct Allocation {
    identity: String,
    wires: Vec<String>,
    class: String,
    state: String,
    start: Rational,
    end: Option<Rational>,
    owner: Option<usize>,
    scheme: Option<String>,
    acquire_order:u64,
    release_order:Option<u64>,
}
struct Allocator<'a> {
    target: &'a Target,
    limits: &'a BTreeMap<String, u64>,
    live: BTreeMap<String, (String, Option<u64>)>,
    allocations: Vec<Allocation>,
    active: BTreeMap<String, usize>,
    counts: BTreeMap<String, u64>,
    order:u64,
}
impl Allocator<'_> {
    fn capacity(&self, class: &str) -> Option<u64> {
        match (
            self.target.classes[class].capacity,
            self.limits.get(class).copied(),
        ) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, None) => a,
            (None, b) => b,
        }
    }
    fn free_vertices(&self, class: &str) -> Vec<u64> {
        let used: BTreeSet<_> = self.live.values().filter_map(|(_, v)| *v).collect();
        self.target
            .value
            .get("topology")
            .and_then(|t| t["vertices"].as_array())
            .map(|v| {
                v.iter()
                    .enumerate()
                    .filter(|(i, v)| v["class"] == class && !used.contains(&(*i as u64)))
                    .map(|(i, _)| i as u64)
                    .collect()
            })
            .unwrap_or_default()
    }
    fn fits(&self, class: &str, n: u64) -> bool {
        self.capacity(class).is_none_or(|cap| {
            self.counts
                .get(class)
                .copied()
                .unwrap_or(0)
                .checked_add(n)
                .is_some_and(|need| need <= cap)
        }) && (!self.target.value.get("topology").is_some()
            || self.free_vertices(class).len() as u64 >= n)
    }
    #[allow(clippy::too_many_arguments)]
    fn allocate(
        &mut self,
        identity: String,
        wires: Vec<String>,
        source_class: &str,
        state: &str,
        now: &Rational,
        owner: Option<usize>,
        scheme: Option<String>,
    ) -> Result<String> {
        let n = wires.len() as u64;
        let class = if source_class == "any" {
            let mut names: Vec<_> = self.target.classes.keys().collect();
            names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            names
                .into_iter()
                .find(|c| self.fits(c, n))
                .cloned()
                .ok_or_else(|| {
                    Error::infeasible(format!(
                        "§8.4: capacity exhausted for allocation {identity} ({n} wires)"
                    ))
                })?
        } else {
            if !self.target.classes.contains_key(source_class) {
                return Err(Error::infeasible(format!(
                    "unknown source allocation class {source_class}"
                )));
            }
            if !self.fits(source_class, n) {
                return Err(Error::infeasible(format!(
                    "§8.4: capacity {source_class} exceeded by {identity}"
                )));
            }
            source_class.into()
        };
        if self.active.contains_key(&identity) || wires.iter().any(|w| self.live.contains_key(w)) {
            return Err(Error::infeasible("allocation aliases live wires"));
        }
        let vertices = self.free_vertices(&class);
        for (i, w) in wires.iter().enumerate() {
            self.live
                .insert(w.clone(), (class.clone(), vertices.get(i).copied()));
        }
        *self.counts.entry(class.clone()).or_default() += n;
        self.active.insert(identity.clone(), self.allocations.len());
        self.allocations.push(Allocation {
            identity,
            wires,
            class: class.clone(),
            state: state.into(),
            start: now.clone(),
            end: None,
            owner,
            scheme,
            acquire_order:self.order,
            release_order:None,
        });
        self.order+=1;
        Ok(class)
    }
    fn release(&mut self, identity: &str, now: &Rational) -> Result<()> {
        let i = self
            .active
            .remove(identity)
            .ok_or_else(|| Error::infeasible("release of non-live frame"))?;
        let a = &mut self.allocations[i];
        a.end = Some(now.clone());
        a.release_order=Some(self.order);self.order+=1;
        for w in &a.wires {
            self.live.remove(w);
        }
        *self.counts.get_mut(&a.class).unwrap() -= a.wires.len() as u64;
        Ok(())
    }
    fn classes(&self, wires: &[String]) -> Result<Vec<String>> {
        wires
            .iter()
            .map(|w| {
                self.live
                    .get(w)
                    .map(|(c, _)| c.clone())
                    .ok_or_else(|| Error::infeasible(format!("gate wire {w} is not live")))
            })
            .collect()
    }
    fn check_topology(&self, wires: &[String]) -> Result<()> {
        if self.target.value.get("topology").is_none() {
            return Ok(());
        }
        let vertices = wires
            .iter()
            .map(|w| {
                self.live
                    .get(w)
                    .and_then(|(_, v)| *v)
                    .ok_or_else(|| Error::infeasible("missing placement"))
            })
            .collect::<Result<Vec<_>>>()?;
        if !self.target.adjacent(&vertices) {
            return Err(Error::infeasible(
                "§8.3 / §17 item 15: native leaf operands are not adjacent",
            ));
        }
        Ok(())
    }
}
fn selector_entry(event: &Event, kind: &str, decision: Value) -> Value {
    let mut e = json!({"path":event.location.path.iter().map(Occurrence::json).collect::<Vec<_>>(),"step":"*","phase":"*","generated":event.location.generated,"kind":kind});
    e[kind] = decision;
    e
}
pub fn run(request: &Request) -> Result<Response> {
    let object = request
        .object
        .as_ref()
        .ok_or_else(|| Error::arg("§11: request object is unset"))?;
    let target = request
        .target
        .as_ref()
        .ok_or_else(|| Error::arg("§11: request target is unset"))?;
    let hash = request
        .region
        .as_ref()
        .ok_or_else(|| Error::arg("§11: request region is unset"))?;
    let variant = request
        .variant
        .as_ref()
        .ok_or_else(|| Error::arg("§11: request variant is unset"))?;
    let budget = request
        .budget
        .ok_or_else(|| Error::arg("§11: request budget is unset"))?;
    let params = request
        .params
        .as_ref()
        .ok_or_else(|| Error::arg("§11: request params are unset (use [] for no arguments)"))?;
    let epsilon = request.epsilon.clone().unwrap_or_else(Rational::zero);
    let f = object.region(hash)?;
    object.validate_params(&f.name, params)?;
    if !target.value["ptr_widths_supported"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| schedule::uint(w).ok() == Some(object.ptr_width))
    {
        return Err(Error::infeasible(
            "§8.3: target does not support object pointer width",
        ));
    }
    for class in request.capacities.keys() {
        if !target.classes.contains_key(class) {
            return Err(Error::arg(format!(
                "capacity names undeclared target class {class}"
            )));
        }
    }
    if !object.metadata.contains_key("layout") && budget[1..].iter().any(|n| *n != 0) {
        return Err(Error::new(
            crate::Status::Budget,
            "§7.2: memoryless request requires zero heap/stack/depth",
        ));
    }
    let trace = Trace::build(object, &f.name, request.inherited_controls, params, budget)?;
    let mut allocator = Allocator {
        target,
        limits: &request.capacities,
        live: BTreeMap::new(),
        allocations: vec![],
        active: BTreeMap::new(),
        counts: BTreeMap::new(),
        order:0,
    };
    let mut now = Rational::zero();
    for (i, w) in trace.inherited.iter().enumerate() {
        allocator.allocate(
            format!("inherited:{i}"),
            vec![w.clone()],
            "any",
            "interface",
            &now,
            None,
            None,
        )?;
    }
    for slot in &trace.slots {
        allocator.allocate(
            slot.id.clone(),
            slot.wires.clone(),
            "any",
            "interface",
            &now,
            None,
            None,
        )?;
    }
    let root_placements = if target.value.get("topology").is_some() {
        Some(
            trace
                .inherited
                .iter()
                .map(|w| vec![allocator.live[w].1.unwrap()])
                .chain(trace.slots.iter().map(|s| {
                    s.wires
                        .iter()
                        .map(|w| allocator.live[w].1.unwrap())
                        .collect()
                }))
                .collect::<Vec<Vec<u64>>>(),
        )
    } else {
        None
    };
    let gate_count = trace
        .events
        .iter()
        .filter(|e| matches!(e.leaf.op, Operation::Gate(_)))
        .count()
        .max(1);
    let share = d::share(&epsilon, gate_count as u64)?;
    let mut entries = Vec::new();
    let mut events = Vec::new();
    let mut event_times = Vec::new();
    let mut leaf_owners = Vec::new();
    let mut errors = Vec::new();
    let mut event_sources = Vec::new();
    let mut event_orders=Vec::new();
    for (source_index, event) in trace.events.iter().enumerate() {
        event_times.push(now.clone());
        event_orders.push(allocator.order);
        match &event.leaf.op {
            Operation::Alloc {
                identity,
                wires,
                class,
                state,
            } => {
                let class = allocator.allocate(
                    identity.clone(),
                    wires.clone(),
                    class,
                    state,
                    &now,
                    Some(event.owner),
                    None,
                )?;
                let mut decision = json!({"class":class});
                if target.value.get("topology").is_some() {
                    decision["vertices"] = json!(
                        wires
                            .iter()
                            .map(|w| allocator.live[w].1.unwrap())
                            .collect::<Vec<_>>()
                    );
                }
                entries.push(selector_entry(event, "alloc", decision));
            }
            Operation::Release { identity, .. } => allocator.release(identity, &now)?,
            Operation::Gate(g) => {
                let classes = allocator.classes(&g.all_wires())?;
                if let Some(n) = target.value["capabilities"]["max_predicate_depth"].as_u64() {
                    if g.controls.len() as u64 > n {
                        return Err(Error::infeasible("max_predicate_depth exceeded"));
                    }
                }
                if target.value["capabilities"]["max_parallel"] == 0 {
                    return Err(Error::infeasible(
                        "max_parallel is zero for a nonempty gate stream",
                    ));
                }
                let native = target.native(g, &classes, &share)?;
                let mut ancilla_identity = None;
                let realisation;
                let realized = if let Some(cost) = native {
                    realisation = "native".to_owned();
                    vec![(g.clone(), cost)]
                } else {
                    let identity = format!("scheme/{source_index}");
                    let plan = target.scheme(g, &identity)?.ok_or_else(|| {
                        Error::infeasible(format!(
                            "§8.3: no target entry or scheme covers {} at the requested accuracy",
                            g.op
                        ))
                    })?;
                    if !plan.ancillae.is_empty() {
                        allocator.allocate(
                            identity.clone(),
                            plan.ancillae.clone(),
                            &plan.class,
                            &plan.state,
                            &now,
                            Some(event.owner),
                            Some(plan.name.clone()),
                        )?;
                        ancilla_identity = Some(identity);
                    }
                    let mut realized = Vec::new();
                    for leaf in plan.leaves {
                        let classes = allocator.classes(&leaf.all_wires())?;
                        let cost=target.native(&leaf,&classes,&share)?.ok_or_else(||Error::infeasible("§8.3: scheme leaf does not match a native gates entry; schemes do not nest"))?;
                        realized.push((leaf, cost));
                    }
                    realisation = plan.name;
                    realized
                };
                if realisation != "native" || !epsilon.is_zero() {
                    entries.push(selector_entry(
                        event,
                        "gate",
                        json!({"realisation":realisation,"epsilon_share":d::exact(&share)?}),
                    ));
                }
                for (leaf, cost) in realized {
                    allocator.check_topology(&leaf.all_wires())?;
                    if object.functions[&trace.invocations[event.owner].name].kind == "perm_action"
                        && !cost.epsilon.is_zero()
                    {
                        return Err(Error::infeasible(
                            "§4.4: permutation action requires exact gates",
                        ));
                    }
                    errors.push(cost.epsilon);
                    leaf_owners.push(event.owner);
                    event_sources.push(source_index);
                    events.push(TimedEvent {
                        start: now.clone(),
                        duration: cost.duration.clone(),
                        costs: cost.costs,
                    });
                    now += cost.duration;
                    allocator.order+=1;
                }
                if let Some(identity) = ancilla_identity {
                    allocator.release(&identity, &now)?;
                }
            }
            Operation::Classical { .. } => {
                let duration =
                    d::nonnegative(&target.value["capabilities"]["classical_step_time"])?;
                events.push(TimedEvent {
                    start: now.clone(),
                    duration: duration.clone(),
                    costs: BTreeMap::new(),
                });
                leaf_owners.push(event.owner);
                event_sources.push(source_index);
                errors.push(Rational::zero());
                now += duration;
                allocator.order+=1;
            }
        }
    }
    event_times.push(now.clone());
    event_orders.push(allocator.order);
    let error: Rational = errors.iter().cloned().sum();
    if error > epsilon {
        return Err(Error::infeasible(
            "§11: achieved operator-norm error exceeds requested epsilon",
        ));
    }
    for allocation in &mut allocator.allocations {
        if allocation.end.is_none() {
            if allocation.state != "interface" {
                return Err(Error::infeasible("workspace remains live at region exit"));
            }
            allocation.end = Some(now.clone());
            allocation.release_order=Some(allocator.order+1);
        }
    }
    let live = allocator
        .allocations
        .iter()
        .map(|a| LiveInterval {
            class: a.class.clone(),
            width: a.wires.len() as u64,
            borrowed: a.state == "borrowed" || a.state == "dirty",
            start: a.start.clone(),
            end: a.end.clone().unwrap(),
        })
        .collect::<Vec<_>>();
    let orders=allocator.allocations.iter().map(|a|(a.acquire_order,a.release_order.unwrap())).collect::<Vec<_>>();
    let rows = resources::aggregate_ordered(target, &events, &live,&orders)?;
    let mut schedule = json!({"bindings":{"closure_hash":hash,"target_hash":target.hash,"region":f.name,"variant":variant,"inherited_controls":request.inherited_controls,"budget":budget.iter().map(|n|integer_json(*n)).collect::<Vec<_>>(),"params_hash":canonical::hash(&canonical::to_vec(params)?),"epsilon":d::exact(&epsilon)?,"expansion":schedule::expansion_binding(),"scheduler":{"name":"bennett-sched","version":env!("CARGO_PKG_VERSION")}},"entries":entries});
    if let Some(placements) = root_placements {
        schedule["interface_vertices"] = json!(placements);
    }
    schedule::Document::parse(schedule.clone())?;
    let schedule_bytes = canonical::to_vec(&schedule)?;
    let schedule_hash = canonical::hash(&schedule_bytes);
    let certificate = certificate(
        request,
        &trace,
        &allocator,
        &rows,
        &events,
        &event_times,
        &errors,
        &leaf_owners,
        &event_sources,
        &event_orders,
        &schedule_hash,
    )?;
    Ok(Response {
        outcome: Outcome::Ok,
        reason: CString::new("OK").unwrap(),
        schedule: schedule_bytes,
        certificate: canonical::to_vec(&certificate)?,
    })
}
fn integer_json(n: u64) -> Value {
    if n >= 1 << 53 {
        json!(n.to_string())
    } else {
        json!(n)
    }
}
fn slot_record(slot: &Slot, allocator: &Allocator<'_>) -> Result<Value> {
    let class = allocator
        .allocations
        .iter()
        .find(|a| slot.wires.first().is_some_and(|w| a.wires.contains(w)))
        .map(|a| a.class.as_str())
        .ok_or_else(|| Error::infeasible("interface slot has no recorded class"))?;
    Ok(json!({"id":slot.id,"role":slot.role,"class":class,"width":slot.wires.len()}))
}
#[allow(clippy::too_many_arguments)]
fn certificate(
    request: &Request,
    trace: &Trace,
    allocator: &Allocator<'_>,
    root_rows: &Rows,
    events: &[TimedEvent],
    times: &[Rational],
    errors: &[Rational],
    leaf_owners: &[usize],
    event_sources: &[usize],
    source_orders:&[u64],
    schedule_hash: &str,
) -> Result<Value> {
    let object = request.object.as_ref().unwrap();
    let target = request.target.as_ref().unwrap();
    let variant = request.variant.as_ref().unwrap();
    let root = &trace.invocations[0];
    let mut regions = json!({});
    let mut multiplicities: BTreeMap<String, u64> = BTreeMap::new();
    let mut context_frames: BTreeMap<String, Value> = BTreeMap::new();
    for (i, inv) in trace.invocations.iter().enumerate() {
        let slots = json!(
            inv.slots
                .iter()
                .map(|s| slot_record(s, allocator))
                .collect::<Result<Vec<_>>>()?
        );
        if let Some(previous) = context_frames.insert(inv.hash.clone(), slots.clone()) {
            if previous != slots || allocator.allocations.iter().any(|a| a.owner == Some(i)) {
                return Err(Error::new(
                    crate::Status::GateUnsupported,
                    "§9.4/§12: context-varying frame descriptors are postponed (bennett-sched-ark); no single-context frame is substituted",
                ));
            }
        }
    }
    for (i, inv) in trace.invocations.iter().enumerate() {
        let key = format!("{variant}:{}", inv.controls.len());
        *multiplicities
            .entry(format!("{}:{key}", inv.hash))
            .or_default() += 1;
        if regions.get(&inv.hash).is_none() {
            let f = &object.functions[&inv.name];
            let slots = inv
                .slots
                .iter()
                .map(|s| slot_record(s, allocator))
                .collect::<Result<Vec<_>>>()?;
            let scratch=allocator.allocations.iter().filter(|a|a.owner==Some(i)&&a.scheme.is_none()).map(|a|json!({"id":a.identity,"class":a.class,"state":if a.state=="borrowed"{"dirty"}else{"clean"},"width":a.wires.len()})).collect::<Vec<_>>();
            let ancillae=allocator.allocations.iter().filter(|a|a.owner==Some(i)&&a.scheme.is_some()).map(|a|json!({"id":a.identity,"scheme":a.scheme,"class":a.class,"state":a.state,"width":a.wires.len()})).collect::<Vec<_>>();
            regions[&inv.hash] = json!({"kind":f.kind,"signature":f.signature,"bounds":f.signature["bounds"],"frame":{"slots":slots,"scratch":scratch,"history":[],"borrow":[],"ancillae":ancillae},"resources":{},"gates_used":{},"claims":[],"verification":{"oracle":null,"borrow_frames":[],"inverse_pairs":[],"unitary":[],"channel":[],"fault_rate":{"domain":{"bundles":[],"borrowed":[],"regions":[]},"faults_by_code":{},"undefined_excluded":0,"layout_dependent_excluded":0}},"cleanup":{},"error":{},"definedness_assumptions":[]});
        }
        let local: Rational = errors
            .iter()
            .zip(leaf_owners)
            .filter(|(_, owner)| **owner == i)
            .map(|(e, _)| e.clone())
            .sum();
        let inclusive: Rational = event_sources
            .iter()
            .zip(errors)
            .filter(|(source, _)| **source >= inv.start && **source < inv.end)
            .map(|(_, e)| e.clone())
            .sum();
        let record = &mut regions[&inv.hash];
        record["cleanup"][&key] = json!([]);
        record["error"][&key] = json!({"local":{"operator_norm":d::upper(&local)?},"inclusive":{"operator_norm":d::upper(&inclusive)?}});
        let claim = json!({"kind":"numerical","subject":{"closure_hash":inv.hash,"variant":variant,"inherited_controls":inv.controls.len(),"schedule_hash":schedule_hash},"metric":"operator_norm","precision_bits":128,"bound":d::upper(&inclusive)?});
        let index = record["claims"].as_array().unwrap().len();
        record["claims"].as_array_mut().unwrap().push(claim);
        record["verification"]["unitary"]
            .as_array_mut()
            .unwrap()
            .push(json!(index));
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        for e in &trace.events[inv.start..inv.end] {
            if let Operation::Gate(g) = &e.leaf.op {
                *counts.entry(g.op.clone()).or_default() += 1;
            }
        }
        record["gates_used"] = Value::Object(
            counts
                .into_iter()
                .map(|(op, n)| (op, json!({"count":n})))
                .collect(),
        );
    }
    regions[&root.hash]["resources"] = root_rows.display_json();
    regions[&root.hash]["estimate"] =
        json!({"precision_digits":34,"exact_resources":root_rows.exact_json()});
    // Each subregion report charges its interface and controls for its invocation duration.
    for (inv_index,inv) in trace.invocations.iter().enumerate().skip(1) {
        let start = &times[inv.start];
        let end = &times[inv.end];
        let mut wires: BTreeSet<String> = inv.controls.iter().cloned().collect();
        for s in &inv.slots {
            wires.extend(s.wires.clone());
        }
        let mut live = Vec::new();
        let mut orders=Vec::new();
        for a in &allocator.allocations {
            let count = a.wires.iter().filter(|w| wires.contains(*w)).count();
            if count > 0 {
                live.push(LiveInterval {
                    class: a.class.clone(),
                    width: count as u64,
                    borrowed: false,
                    start: start.clone(),
                    end: end.clone(),
                });
                orders.push((source_orders[inv.start],source_orders[inv.end].max(source_orders[inv.start]+1)));
            }else if a.owner.is_some_and(|owner|owner>=inv_index&&trace.invocations[owner].start>=inv.start&&trace.invocations[owner].end<=inv.end){
                live.push(LiveInterval{class:a.class.clone(),width:a.wires.len() as u64,borrowed:a.state=="borrowed"||a.state=="dirty",start:a.start.clone(),end:a.end.clone().unwrap()});orders.push((a.acquire_order,a.release_order.unwrap()));
            }
        }
        for l in &mut live {
            l.start -= start;
            l.end -= start;
        }
        let subset = events
            .iter()
            .zip(event_sources)
            .filter(|(_, source)| **source >= inv.start && **source < inv.end)
            .map(|(e, _)| {
                let mut e = e.clone();
                e.start -= start;
                e
            })
            .collect::<Vec<_>>();
        let rows = resources::aggregate_ordered(target, &subset, &live,&orders)?;
        regions[&inv.hash]["resources"] = rows.display_json();
    }
    let total: Rational = errors.iter().cloned().sum();
    Ok(
        json!({"versions":object.versions,"bindings":{"object_hash":object.object_hash,"interface_hash":object.symbols[&root.name].interface_hash,"closure_hash":root.hash,"reference_program_hash":object.symbols[&root.name].interface_hash,"dialect_mapping_version":"isa-0.9","target_hash":target.hash,"schedule_hash":schedule_hash,"invocation":{"args":request.params.as_ref().unwrap(),"budget":request.budget.unwrap().iter().map(u64::to_string).collect::<Vec<_>>() }},"producer":{"name":"bennett-sched","version":env!("CARGO_PKG_VERSION"),"materialisation":{"llvm_version":"none","pipeline":"ref-sequential","normaliser_version":"0.9"}},"regions":regions,"composition":{"multiplicities":multiplicities,"epsilon_total":{"diamond":d::upper(&(d::integer(2)*total))?},"method":"sum_over_multiplicities"}}),
    )
}
