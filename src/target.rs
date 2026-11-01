//! Target 0.9 validation, exact cost expressions and native gate matching.
use crate::{
    Error, Result, canonical,
    decimal::{self as d, Rational},
    schedule::{array, fields, text, uint},
};
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub type Variables = BTreeMap<String, Rational>;

pub fn eval(expr: &Value, vars: &Variables, tables: &Value) -> Result<Rational> {
    if expr.is_number() {
        return d::value(expr);
    }
    if expr.get("var").is_some() {
        fields(expr, &["var"], &[])?;
        let name = text(&expr["var"])?;
        let value = vars
            .get(name)
            .cloned()
            .ok_or_else(|| Error::target(format!("§8.1: unbound variable {name}")))?;
        if name == "eps" && (value <= Rational::zero() || value > Rational::one()) {
            return Err(Error::target("§8.1: eps outside (0,1]"));
        }
        return Ok(value);
    }
    if expr.get("table").is_some() {
        fields(expr, &["table", "index"], &[])?;
        let index = match text(&expr["index"])? {
            "k" => vars
                .get("k")
                .cloned()
                .ok_or_else(|| Error::target("unbound table index k"))?,
            "eps_decade" => d::integer(log_inverse(vars.get("eps"), 10)?),
            _ => return Err(Error::target("§8.1: unknown table index")),
        };
        if !index.is_integer() || index.is_negative() {
            return Err(Error::target("non-integer table index"));
        }
        let i = index
            .to_integer()
            .to_usize()
            .ok_or_else(|| Error::target("table index exceeds host usize"))?;
        return d::value(
            tables
                .get(text(&expr["table"])?)
                .and_then(|v| v.get(i))
                .ok_or_else(|| Error::target("§8.1: table index outside array"))?,
        );
    }
    fields(expr, &["op", "args"], &[])?;
    let op = text(&expr["op"])?;
    let a = array(&expr["args"])?;
    let arity = match op {
        "+" | "-" | "*" | "/" | "max" | "min" => 2,
        "ceil" => 1,
        "log2inv_eps" => 0,
        _ => return Err(Error::target("§8.1: unknown cost operator")),
    };
    if a.len() != arity {
        return Err(Error::target("§8.1: cost operator arity mismatch"));
    }
    if op == "log2inv_eps" {
        return Ok(d::integer(log_inverse(vars.get("eps"), 2)?));
    }
    let x = eval(&a[0], vars, tables)?;
    if op == "ceil" {
        return Ok(Rational::from_integer(d::ceil(&x)));
    }
    let y = eval(&a[1], vars, tables)?;
    Ok(match op {
        "+" => x + y,
        "-" => x - y,
        "*" => x * y,
        "/" => {
            if y.is_zero() {
                return Err(Error::target("§8.1: division by zero"));
            }
            x / y
        }
        "max" => x.max(y),
        "min" => x.min(y),
        _ => unreachable!(),
    })
}
fn log_inverse(eps: Option<&Rational>, base: u64) -> Result<u64> {
    let mut q = eps
        .cloned()
        .ok_or_else(|| Error::target("§8.1: unbound eps"))?;
    if q <= Rational::zero() || q > Rational::one() {
        return Err(Error::target("§8.1: eps outside (0,1]"));
    }
    let mut n = 0;
    while q < Rational::one() {
        q *= d::integer(base);
        n += 1;
    }
    Ok(n)
}
fn expression(expr: &Value, allowed: &[&str], tables: &Value) -> Result<()> {
    if expr.is_number() {
        d::value(expr)?;
        return Ok(());
    }
    if expr.get("var").is_some() {
        fields(expr, &["var"], &[])?;
        if !allowed.contains(&text(&expr["var"])?) {
            return Err(Error::target(
                "§8.1: variable not bound in expression context",
            ));
        }
        return Ok(());
    }
    if expr.get("table").is_some() {
        fields(expr, &["table", "index"], &[])?;
        let index = text(&expr["index"])?;
        if !((index == "k" && allowed.contains(&"k"))
            || (index == "eps_decade" && allowed.contains(&"eps")))
        {
            return Err(Error::target("§8.1: invalid table context"));
        }
        array(&tables[text(&expr["table"])?])?;
        return Ok(());
    }
    fields(expr, &["op", "args"], &[])?;
    let op = text(&expr["op"])?;
    let a = array(&expr["args"])?;
    let n = match op {
        "+" | "-" | "*" | "/" | "max" | "min" => 2,
        "ceil" => 1,
        "log2inv_eps" if allowed.contains(&"eps") => 0,
        _ => return Err(Error::target("§8.1: invalid operator in context")),
    };
    if a.len() != n {
        return Err(Error::target("§8.1: operator arity mismatch"));
    }
    for e in a {
        expression(e, allowed, tables)?;
    }
    Ok(())
}
fn refs_eps(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.iter().any(refs_eps),
        Value::Object(o) => {
            o.get("var") == Some(&json!("eps"))
                || o.get("op") == Some(&json!("log2inv_eps"))
                || o.get("index") == Some(&json!("eps_decade"))
                || o.values().any(refs_eps)
        }
        _ => false,
    }
}
pub const GATES: &[&str] = &[
    "x", "h", "s", "sdg", "t", "tdg", "rx", "ry", "rz", "cx", "ccx", "mcx", "swap", "cz",
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate {
    pub op: String,
    pub wires: Vec<String>,
    pub controls: Vec<String>,
    pub theta: Option<String>,
}
impl Gate {
    pub fn own_controls(&self) -> usize {
        match self.op.as_str() {
            "cx" => 1,
            "ccx" => 2,
            "mcx" => self.wires.len().saturating_sub(1),
            _ => 0,
        }
    }
    pub fn control_wires(&self) -> Vec<String> {
        let mut wires = self.wires[..self.own_controls()].to_vec();
        for w in &self.controls {
            if !wires.contains(w) {
                wires.push(w.clone());
            }
        }
        wires
    }
    pub fn all_wires(&self) -> Vec<String> {
        let mut w = self.control_wires();
        w.extend_from_slice(&self.wires[self.own_controls()..]);
        w
    }
    pub fn validate(&self) -> Result<()> {
        if !GATES.contains(&self.op.as_str()) {
            return Err(Error::arg(format!("§4.4: unknown gate {}", self.op)));
        }
        let n = match self.op.as_str() {
            "cx" | "swap" | "cz" => 2,
            "ccx" => 3,
            "mcx" => self.wires.len().max(1),
            _ => 1,
        };
        if self.wires.len() != n || self.wires.iter().collect::<BTreeSet<_>>().len() != n {
            return Err(Error::arg(
                "§3.3: invalid gate arity or coincident operands",
            ));
        }
        if self
            .controls
            .iter()
            .any(|w| self.wires[self.own_controls()..].contains(w))
        {
            return Err(Error::arg(
                "§3.3: effective control coincides with gate target",
            ));
        }
        if ["rx", "ry", "rz"].contains(&self.op.as_str()) != self.theta.is_some() {
            return Err(Error::arg("§4.4: gate angle mismatch"));
        }
        if let Some(t) = &self.theta {
            if t.len() != 18
                || !t.starts_with("0x")
                || !t[2..]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Error::arg("angle must be binary64 bit-pattern hex"));
            }
            let n = u64::from_str_radix(&t[2..], 16).map_err(|_| Error::arg("invalid angle"))?;
            if !f64::from_bits(n).is_finite() {
                return Err(Error::arg("nonfinite angle"));
            }
        }
        Ok(())
    }
    pub fn adjoint(&self) -> Self {
        let mut g = self.clone();
        g.op = match self.op.as_str() {
            "s" => "sdg",
            "sdg" => "s",
            "t" => "tdg",
            "tdg" => "t",
            _ => &self.op,
        }
        .into();
        if let Some(t) = &self.theta {
            let n = u64::from_str_radix(&t[2..], 16).expect("validated angle");
            g.theta = Some(format!("0x{:016x}", n ^ (1 << 63)));
        }
        g
    }
}
#[derive(Clone, Debug)]
pub struct Class {
    pub capacity: Option<u64>,
    pub qubit_second: Rational,
    pub dirty_rate: Rational,
}
#[derive(Clone, Debug)]
pub struct NativeCost {
    pub duration: Rational,
    pub epsilon: Rational,
    pub costs: BTreeMap<String, Rational>,
    pub entry: usize,
}
#[derive(Clone, Debug)]
pub struct SchemePlan {
    pub name: String,
    pub class: String,
    pub state: String,
    pub ancillae: Vec<String>,
    pub leaves: Vec<Gate>,
}
#[derive(Clone, Debug)]
pub struct Target {
    pub value: Value,
    pub classes: BTreeMap<String, Class>,
    pub hash: String,
}
fn bound(v: &Value) -> Result<Option<u64>> {
    if v == "unbounded" {
        Ok(None)
    } else {
        Ok(Some(uint(v)?))
    }
}
fn control_range(v: &Value, c: u64) -> Result<bool> {
    fields(v, &["min", "max"], &[])?;
    let min = uint(&v["min"])?;
    let max = bound(&v["max"])?;
    if max.is_some_and(|m| m < min) {
        return Err(Error::target("invalid controls range"));
    }
    Ok(c >= min && max.is_none_or(|m| c <= m))
}
impl Target {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::from_value(canonical::require_jcs(bytes)?)
    }
    pub fn from_value(v: Value) -> Result<Self> {
        fields(
            &v,
            &[
                "ptr_widths_supported",
                "classes",
                "gates",
                "schemes",
                "qram",
                "capabilities",
                "objective",
                "tables",
                "imports",
                "resources",
            ],
            &["topology"],
        )?;
        let m = v["classes"]
            .as_object()
            .ok_or_else(|| Error::target("classes must be object"))?;
        if m.is_empty() {
            return Err(Error::target("target declares no classes"));
        }
        let mut classes = BTreeMap::new();
        for (name, c) in m {
            if name.is_empty() || name == "any" {
                return Err(Error::target("invalid target class name"));
            }
            fields(c, &["capacity", "qubit_second", "dirty_rate"], &[])?;
            classes.insert(
                name.clone(),
                Class {
                    capacity: bound(&c["capacity"])?,
                    qubit_second: d::nonnegative(&c["qubit_second"])?,
                    dirty_rate: d::nonnegative(&c["dirty_rate"])?,
                },
            );
        }
        for n in array(&v["ptr_widths_supported"])? {
            if uint(n)? == 0 {
                return Err(Error::target("pointer width must be positive"));
            }
        }
        let tables = v["tables"]
            .as_object()
            .ok_or_else(|| Error::target("tables must be object"))?;
        for a in tables.values() {
            for n in array(a)? {
                if !n.is_number() {
                    return Err(Error::target("table values must be numbers"));
                }
                d::nonnegative(n)?;
            }
        }
        let resources = v["resources"]
            .as_object()
            .ok_or_else(|| Error::target("resources must be object"))?;
        for (name, r) in resources {
            fields(r, &["kind", "unit"], &["rate"])?;
            text(&r["unit"])?;
            if !["count", "time", "occupancy", "peak"].contains(&text(&r["kind"])?) {
                return Err(Error::target("invalid resource kind"));
            }
            if let Some(rate) = r.get("rate") {
                for (c, n) in rate
                    .as_object()
                    .ok_or_else(|| Error::target("rate must be object"))?
                {
                    if !classes.contains_key(c) {
                        return Err(Error::target("unknown rate class"));
                    }
                    d::nonnegative(n)?;
                }
            }
            if name.is_empty() {
                return Err(Error::target("empty resource name"));
            }
        }
        for (name, kind) in [
            ("time", "time"),
            ("qubit_second", "occupancy"),
            ("peak_qubits", "peak"),
        ] {
            if v["resources"][name]["kind"] != kind {
                return Err(Error::target(format!(
                    "mandatory resource {name} must be {kind}"
                )));
            }
        }
        for g in array(&v["gates"])? {
            fields(
                g,
                &["op", "classes", "controls", "epsilon", "cost"],
                &["params"],
            )?;
            if !GATES.contains(&text(&g["op"])?) {
                return Err(Error::target("unknown native gate"));
            }
            let pattern = array(&g["classes"])?;
            if pattern.is_empty() {
                return Err(Error::target("empty class pattern"));
            }
            for c in pattern {
                if c != "*" && !classes.contains_key(text(c)?) {
                    return Err(Error::target("unknown native class"));
                }
            }
            control_range(&g["controls"], 0)?;
            expression(&g["epsilon"], &["k", "c", "eps"], &v["tables"])?;
            let exact = g["epsilon"].is_number() && d::value(&g["epsilon"])?.is_zero();
            if exact && (refs_eps(&g["epsilon"]) || refs_eps(&g["cost"])) {
                return Err(Error::target("§8.1: exact entry references eps"));
            }
            let costs = g["cost"]
                .as_object()
                .ok_or_else(|| Error::target("cost must be object"))?;
            if !costs.contains_key("time") {
                return Err(Error::target("gate cost omits time"));
            }
            for (r, e) in costs {
                if !resources.contains_key(r) {
                    return Err(Error::target("cost names unknown resource"));
                }
                expression(
                    e,
                    if exact {
                        &["k", "c"]
                    } else {
                        &["k", "c", "eps"]
                    },
                    &v["tables"],
                )?;
            }
            if let Some(params) = g.get("params") {
                if params != &json!(["theta"]) {
                    return Err(Error::target("invalid native gate params"));
                }
            }
        }
        let mut names = BTreeSet::new();
        for s in array(&v["schemes"])? {
            fields(s, &["name", "realises", "ancillae", "template"], &[])?;
            if !names.insert(text(&s["name"])?)
                || text(&s["name"])?.is_empty()
                || s["name"] == "native"
            {
                return Err(Error::target("invalid/duplicate scheme name"));
            }
            fields(&s["realises"], &["op", "controls"], &["params"])?;
            if !GATES.contains(&text(&s["realises"]["op"])?) {
                return Err(Error::target("unknown realised gate"));
            }
            control_range(&s["realises"]["controls"], 0)?;
            fields(&s["ancillae"], &["count", "state", "class"], &[])?;
            if !["clean", "dirty"].contains(&text(&s["ancillae"]["state"])?)
                || !classes.contains_key(text(&s["ancillae"]["class"])?)
            {
                return Err(Error::target("invalid scheme ancilla"));
            }
            expression(&s["ancillae"]["count"], &["k", "c", "theta"], &v["tables"])?;
            for leaf in array(&s["template"])? {
                fields(leaf, &["gate", "wires"], &["params", "repeat"])?;
                if !GATES.contains(&text(&leaf["gate"])?) {
                    return Err(Error::target("unknown scheme template gate"));
                }
                for w in array(&leaf["wires"])? {
                    text(w)?;
                }
                if let Some(p) = leaf.get("params") {
                    for e in array(p)? {
                        expression(e, &["theta", "i"], &v["tables"])?;
                    }
                }
                if let Some(r) = leaf.get("repeat") {
                    fields(r, &["var", "from", "to"], &[])?;
                    if r["var"] != "i" {
                        return Err(Error::target("scheme repeat variable must be i"));
                    }
                    for key in ["from", "to"] {
                        expression(&r[key], &["k", "c", "theta"], &v["tables"])?;
                    }
                }
            }
        }
        let cap = &v["capabilities"];
        fields(
            cap,
            &[
                "measurement",
                "mbu",
                "max_parallel",
                "max_predicate_depth",
                "classical_step_time",
            ],
            &[],
        )?;
        if cap["mbu"] != false {
            return Err(Error::target("§6.4: mbu must be false"));
        }
        fields(
            &cap["measurement"],
            &["mid_circuit", "feedforward_latency"],
            &[],
        )?;
        if !cap["measurement"]["mid_circuit"].is_boolean() {
            return Err(Error::target("measurement capability must be boolean"));
        }
        d::nonnegative(&cap["measurement"]["feedforward_latency"])?;
        d::nonnegative(&cap["classical_step_time"])?;
        bound(&cap["max_parallel"])?;
        bound(&cap["max_predicate_depth"])?;
        if let Some(t) = v.get("topology") {
            fields(t, &["vertices", "edges"], &[])?;
            let vertices = array(&t["vertices"])?;
            for vertex in vertices {
                fields(vertex, &["class"], &[])?;
                if !classes.contains_key(text(&vertex["class"])?) {
                    return Err(Error::target("unknown topology class"));
                }
            }
            let mut prev = None;
            for edge in array(&t["edges"])? {
                let e = array(edge)?;
                if e.len() != 2 {
                    return Err(Error::target("topology edge requires two vertices"));
                }
                let pair = (uint(&e[0])?, uint(&e[1])?);
                if pair.0 >= pair.1
                    || pair.1 >= vertices.len() as u64
                    || prev.is_some_and(|p| p >= pair)
                {
                    return Err(Error::target(
                        "§8.3: unsorted, duplicate or invalid topology edge",
                    ));
                }
                prev = Some(pair);
            }
        }
        let obj = &v["objective"];
        if obj.get("weighted").is_some() {
            fields(obj, &["weighted"], &[])?;
            for (r, n) in obj["weighted"]
                .as_object()
                .ok_or_else(|| Error::target("weighted objective must be object"))?
            {
                if !resources.contains_key(r) {
                    return Err(Error::target("objective names unknown resource"));
                }
                d::nonnegative(n)?;
            }
        } else {
            fields(obj, &["lexicographic"], &[])?;
            for r in array(&obj["lexicographic"])? {
                if !resources.contains_key(text(r)?) {
                    return Err(Error::target("objective names unknown resource"));
                }
            }
        }
        if !v["imports"].is_object() {
            return Err(Error::target("imports must be object"));
        }
        if !v["qram"].is_null() {
            fields(&v["qram"], &["load", "store", "exch"], &[])?;
            for op in ["load", "store", "exch"] {
                fields(&v["qram"][op], &["cost"], &[])?;
                for (r, e) in v["qram"][op]["cost"]
                    .as_object()
                    .ok_or_else(|| Error::target("qram cost must be object"))?
                {
                    if !resources.contains_key(r) {
                        return Err(Error::target("qram resource unknown"));
                    }
                    expression(e, &["a", "w", "c"], &v["tables"])?;
                }
            }
        }
        let hash = canonical::hash(&canonical::to_vec(&v)?);
        Ok(Self {
            value: v,
            classes,
            hash,
        })
    }
    pub fn native(
        &self,
        g: &Gate,
        classes: &[String],
        eps: &Rational,
    ) -> Result<Option<NativeCost>> {
        g.validate()?;
        let k = g.own_controls() as u64;
        let c = g.control_wires().len() as u64;
        let vars = Variables::from([
            ("k".into(), d::integer(k)),
            ("c".into(), d::integer(c)),
            ("eps".into(), eps.clone()),
        ]);
        for (i, e) in array(&self.value["gates"])?.iter().enumerate() {
            if e["op"] != g.op || !control_range(&e["controls"], c)? {
                continue;
            }
            if e.get("params").is_some() != g.theta.is_some() {
                continue;
            }
            let pattern = array(&e["classes"])?;
            let matches = if pattern.len() == 1 {
                classes
                    .iter()
                    .all(|c| pattern[0] == "*" || pattern[0] == *c)
            } else {
                pattern.len() == classes.len()
                    && pattern.iter().zip(classes).all(|(p, c)| p == "*" || p == c)
            };
            if !matches {
                continue;
            }
            let epsilon = eval(&e["epsilon"], &vars, &self.value["tables"])?;
            if epsilon.is_negative() {
                return Err(Error::target("negative achieved epsilon"));
            }
            if epsilon.is_zero() && (refs_eps(&e["epsilon"]) || refs_eps(&e["cost"])) {
                return Err(Error::target("exact entry references eps"));
            }
            if epsilon > *eps {
                continue;
            }
            let mut costs = BTreeMap::new();
            for (name, expr) in e["cost"].as_object().expect("validated costs") {
                let q = eval(expr, &vars, &self.value["tables"])?;
                if q.is_negative() {
                    return Err(Error::target("negative gate cost"));
                }
                costs.insert(name.clone(), q);
            }
            return Ok(Some(NativeCost {
                duration: costs["time"].clone(),
                epsilon,
                costs,
                entry: i,
            }));
        }
        Ok(None)
    }
    pub fn adjacent(&self, vertices: &[u64]) -> bool {
        let Some(t) = self.value.get("topology") else {
            return true;
        };
        for (i, &u) in vertices.iter().enumerate() {
            for &v in &vertices[i + 1..] {
                let edge = json!([u.min(v), u.max(v)]);
                if !t["edges"]
                    .as_array()
                    .expect("validated edges")
                    .contains(&edge)
                {
                    return false;
                }
            }
        }
        true
    }
    pub fn scheme(&self, g: &Gate, prefix: &str) -> Result<Option<SchemePlan>> {
        g.validate()?;
        let k = g.own_controls() as u64;
        let controls = g.control_wires();
        let c = controls.len() as u64;
        let mut vars = Variables::from([("k".into(), d::integer(k)), ("c".into(), d::integer(c))]);
        if let Some(theta) = &g.theta {
            let n = u64::from_str_radix(&theta[2..], 16)
                .map_err(|_| Error::target("invalid scheme angle"))?;
            vars.insert(
                "theta".into(),
                Rational::from_float(f64::from_bits(n))
                    .ok_or_else(|| Error::target("nonfinite scheme angle"))?,
            );
        }
        for scheme in array(&self.value["schemes"])? {
            let realises = &scheme["realises"];
            if realises["op"] != g.op
                || !control_range(&realises["controls"], c)?
                || realises.get("params").is_some() != g.theta.is_some()
            {
                continue;
            }
            let count = eval(&scheme["ancillae"]["count"], &vars, &self.value["tables"])?;
            if !count.is_integer() || count.is_negative() {
                return Err(Error::target(
                    "scheme ancilla count must be nonnegative integer",
                ));
            }
            let n = count
                .to_integer()
                .to_usize()
                .ok_or_else(|| Error::target("scheme ancilla count exceeds host usize"))?;
            let ancillae = (0..n)
                .map(|i| format!("{prefix}/ancilla[{i}]"))
                .collect::<Vec<_>>();
            let targets = &g.wires[g.own_controls()..];
            let mut leaves = Vec::new();
            for template in array(&scheme["template"])? {
                let (from, to) = if let Some(repeat) = template.get("repeat") {
                    (
                        eval(&repeat["from"], &vars, &self.value["tables"])?,
                        eval(&repeat["to"], &vars, &self.value["tables"])?,
                    )
                } else {
                    (d::integer(0), d::integer(1))
                };
                if !from.is_integer() || !to.is_integer() || from.is_negative() || to < from {
                    return Err(Error::target("invalid scheme repeat bounds"));
                }
                let mut i = from;
                while i < to {
                    let mut local = vars.clone();
                    local.insert("i".into(), i.clone());
                    let mut wires = Vec::new();
                    for w in array(&template["wires"])? {
                        let w = text(w)?;
                        if w == "t" || w == "t2" {
                            let index = usize::from(w == "t2");
                            wires.push(
                                targets
                                    .get(index)
                                    .ok_or_else(|| {
                                        Error::target("scheme target index outside gate")
                                    })?
                                    .clone(),
                            );
                        } else {
                            let (bundle, index) = w
                                .split_once('[')
                                .ok_or_else(|| Error::target("invalid template wire"))?;
                            let index = index
                                .strip_suffix(']')
                                .ok_or_else(|| Error::target("invalid template subscript"))?;
                            let e = if local.contains_key(index) {
                                json!({"var":index})
                            } else {
                                canonical::parse(index.as_bytes())?
                            };
                            let q = eval(&e, &local, &self.value["tables"])?;
                            if !q.is_integer() || q.is_negative() {
                                return Err(Error::target(
                                    "scheme wire subscript must be nonnegative integer",
                                ));
                            }
                            let index = q
                                .to_integer()
                                .to_usize()
                                .ok_or_else(|| Error::target("scheme index exceeds host usize"))?;
                            let wire = match bundle {
                                "c" => controls.get(index),
                                "a" => ancillae.get(index),
                                _ => None,
                            }
                            .ok_or_else(|| {
                                Error::target("scheme wire outside controls/ancillae")
                            })?;
                            wires.push(wire.clone());
                        }
                    }
                    let theta = if let Some(params) = template.get("params") {
                        let p = array(params)?;
                        if p.len() != 1 {
                            return Err(Error::target("scheme leaf angle arity"));
                        }
                        let q = eval(&p[0], &local, &self.value["tables"])?;
                        let f = q
                            .to_f64()
                            .filter(|f| f.is_finite())
                            .ok_or_else(|| Error::target("scheme angle exceeds finite binary64"))?;
                        Some(format!("0x{:016x}", f.to_bits()))
                    } else {
                        None
                    };
                    let leaf = Gate {
                        op: text(&template["gate"])?.into(),
                        wires,
                        controls: vec![],
                        theta,
                    };
                    leaf.validate()?;
                    leaves.push(leaf);
                    i += d::integer(1);
                }
            }
            return Ok(Some(SchemePlan {
                name: text(&scheme["name"])?.into(),
                class: text(&scheme["ancillae"]["class"])?.into(),
                state: text(&scheme["ancillae"]["state"])?.into(),
                ancillae,
                leaves,
            }));
        }
        Ok(None)
    }
}
