//! Typed, capture-free §11.1 constructors. No caller-environment fallback for invoke.
use crate::{
    Error, Result, canonical,
    schedule::{EXPANSION_HASH, array, fields, text, uint},
    target::Gate,
};
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub type Env = BTreeMap<String, Binding>;
#[derive(Clone, Debug)]
pub enum Binding {
    Integer(BigInt),
    Wires(Vec<String>),
    Angle(String),
    Environment(Vec<String>),
    Body(Capture),
    Templates(Vec<Template>),
    Symbol(String),
    Arguments {
        symbol: String,
        values: Vec<Binding>,
    },
    Event(Value),
    Image(String),
    Class(String),
    Allocation {
        identity: String,
        wires: Vec<String>,
        class: String,
        state: String,
    },
    Region {
        address: u64,
        cell: u64,
        role: String,
        origin: String,
        wires: Vec<String>,
    },
    Records {
        schema: Value,
        records: Vec<Env>,
    },
    Value {
        isa_type: Value,
        value: Value,
    },
}
#[derive(Clone, Debug)]
pub struct Capture {
    pub body: Vec<Value>,
    pub env: Env,
    pub controls: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Template {
    pub parameters: Vec<Value>,
    pub capture: Capture,
}
#[derive(Clone, Debug)]
pub struct Symbol {
    pub parameters: Vec<Value>,
    pub body: Vec<Value>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Gate(Gate),
    Alloc {
        identity: String,
        wires: Vec<String>,
        class: String,
        state: String,
    },
    Release {
        identity: String,
        wires: Vec<String>,
        class: String,
        state: String,
    },
    Classical {
        opcode: String,
        event: Value,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallEdge {
    pub symbol: String,
    pub call_index: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leaf {
    pub ordinal: u64,
    pub calls: Vec<CallEdge>,
    pub op: Operation,
}
impl Leaf {
    pub fn wires(&self) -> Vec<String> {
        match &self.op {
            Operation::Gate(g) => g.all_wires(),
            Operation::Alloc { wires, .. } | Operation::Release { wires, .. } => wires.clone(),
            Operation::Classical { .. } => vec![],
        }
    }
}
pub fn inverse(leaves: &[Leaf]) -> Vec<Leaf> {
    leaves
        .iter()
        .rev()
        .map(|l| {
            let mut l = l.clone();
            l.op = match l.op {
                Operation::Gate(g) => Operation::Gate(g.adjoint()),
                Operation::Alloc {
                    identity,
                    wires,
                    class,
                    state,
                } => Operation::Release {
                    identity,
                    wires,
                    class,
                    state,
                },
                Operation::Release {
                    identity,
                    wires,
                    class,
                    state,
                } => Operation::Alloc {
                    identity,
                    wires,
                    class,
                    state,
                },
                Operation::Classical { opcode, event } => Operation::Classical { opcode, event },
            };
            l
        })
        .collect()
}
#[derive(Clone, Debug)]
pub struct Library {
    pub recipes: BTreeMap<String, Value>,
    pub symbols: BTreeMap<String, Symbol>,
}
impl Library {
    pub fn published() -> Result<Self> {
        let bytes = include_bytes!("../data/ref-0.9.json");
        let hash = canonical::hash(
            &[
                b"isa=0.9\nexpansion=ref\nversion=0.9\n".as_slice(),
                bytes,
                b"\n",
            ]
            .concat(),
        );
        if hash != EXPANSION_HASH {
            return Err(Error::infeasible("§11.1: ref library envelope mismatch"));
        }
        Self::from_value(canonical::require_jcs(bytes)?)
    }
    pub fn from_value(v: Value) -> Result<Self> {
        fields(&v, &["isa", "name", "version", "recipes"], &[])?;
        if v["isa"] != "0.9" || v["name"] != "ref" || v["version"] != "0.9" {
            return Err(Error::infeasible("unsupported reference library identity"));
        }
        let mut recipes = BTreeMap::new();
        let mut previous = String::new();
        for r in array(&v["recipes"])? {
            fields(
                r,
                &[
                    "kind",
                    "parameters",
                    "body",
                    "scratch",
                    "removable",
                    "source",
                ],
                &[],
            )?;
            let name = text(&r["kind"])?;
            if name <= previous.as_str() || recipes.contains_key(name) {
                return Err(Error::arg("§11.1: recipe kinds must be unique and sorted"));
            }
            previous = name.into();
            if !array(&r["removable"])?.is_empty() {
                return Err(Error::arg("§11.1: ref 0.9 removable must be empty"));
            }
            validate_formals(array(&r["parameters"])?)?;
            validate_body(array(&r["body"])?)?;
            for s in array(&r["scratch"])? {
                text(s)?;
            }
            text(&r["source"])?;
            recipes.insert(name.into(), r.clone());
        }
        let lib = Self {
            recipes,
            symbols: BTreeMap::new(),
        };
        lib.acyclic()?;
        Ok(lib)
    }
    fn acyclic(&self) -> Result<()> {
        fn dependencies(v: &Value, out: &mut BTreeSet<String>) {
            match v {
                Value::Object(o) => {
                    if let Some(s) = o.get("invoke").and_then(Value::as_str) {
                        out.insert(s.into());
                    }
                    for v in o.values() {
                        dependencies(v, out);
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        dependencies(v, out);
                    }
                }
                _ => {}
            }
        }
        fn visit(
            name: &str,
            lib: &Library,
            active: &mut BTreeSet<String>,
            done: &mut BTreeSet<String>,
        ) -> Result<()> {
            if done.contains(name) {
                return Ok(());
            }
            if !active.insert(name.into()) {
                return Err(Error::arg("§11.1: cyclic recipe dependencies"));
            }
            let r = lib
                .recipes
                .get(name)
                .ok_or_else(|| Error::arg(format!("unbound recipe {name}")))?;
            let mut deps = BTreeSet::new();
            dependencies(&r["body"], &mut deps);
            for dep in deps {
                visit(&dep, lib, active, done)?;
            }
            active.remove(name);
            done.insert(name.into());
            Ok(())
        }
        let mut done = BTreeSet::new();
        for name in self.recipes.keys() {
            visit(name, self, &mut BTreeSet::new(), &mut done)?;
        }
        Ok(())
    }
    pub fn expand(&self, kind: &str, args: &Value, caller: &Env) -> Result<Vec<Leaf>> {
        let r = self
            .recipes
            .get(kind)
            .ok_or_else(|| Error::arg(format!("unbound recipe {kind}")))?;
        let mut c = Compiler {
            library: self,
            next: 0,
            depth: 0,
        };
        let mut frame = Frame::default();
        let bound = c.bind(array(&r["parameters"])?, array(args)?, caller, &[], true)?;
        let mut env=crate::ref_contract::named_scope(r,bound)?;
        let controls = environment_controls(&env, &[]);
        let mut leaves = c.compile(array(&r["body"])?, &mut env, &controls, &mut frame, false)?;
        number(&mut leaves);
        fresh_acquisitions(&mut leaves);
        validate_lifetimes(&leaves)?;
        Ok(leaves)
    }
    pub fn body(&self, body: &[Value], env: &Env, controls: &[String]) -> Result<Vec<Leaf>> {
        validate_body(body)?;
        let mut c = Compiler {
            library: self,
            next: 0,
            depth: 0,
        };
        let mut frame = Frame::default();
        let mut leaves = c.compile(body, &mut env.clone(), controls, &mut frame, false)?;
        number(&mut leaves);
        fresh_acquisitions(&mut leaves);
        validate_lifetimes(&leaves)?;
        Ok(leaves)
    }
}
fn number(leaves: &mut [Leaf]) {
    let mut n = 0;
    for l in leaves {
        if l.calls.is_empty() {
            l.ordinal = n;
            n += 1;
        }
    }
}
fn environment_controls(env: &Env, outer: &[String]) -> Vec<String> {
    let mut c = outer.to_vec();
    for b in env.values() {
        if let Binding::Environment(w) = b {
            union(&mut c, w);
        }
    }
    c
}
fn union(a: &mut Vec<String>, b: &[String]) {
    for w in b {
        if !a.contains(w) {
            a.push(w.clone());
        }
    }
}
pub fn scalar(v: &Value, env: &Env) -> Result<BigInt> {
    if v.is_number() {
        let s = v.to_string();
        return s
            .parse()
            .map_err(|_| Error::arg("§11.1: integer expression literal must be integral"));
    }
    if let Some(s) = v.as_str() {
        return match env.get(s) {
            Some(Binding::Integer(n)) => Ok(n.clone()),
            _ => Err(Error::arg(format!("§11.1: unbound integer {s}"))),
        };
    }
    let a = array(v)?;
    if a.len() != 3 {
        return Err(Error::arg("§11.1: scalar operator arity"));
    }
    let x = scalar(&a[1], env)?;
    let y = scalar(&a[2], env)?;
    Ok(match text(&a[0])? {
        "+" => x + y,
        "-" => x - y,
        "*" => x * y,
        "div" | "mod" => {
            if y <= BigInt::zero() {
                return Err(Error::arg("§11.1: scalar divisor must be positive"));
            }
            let mut q = &x / &y;
            let mut r = &x % &y;
            if r.is_negative() {
                q -= 1;
                r += &y;
            }
            if a[0] == "div" { q } else { r }
        }
        "bit" => {
            if x.is_negative() || y.is_negative() {
                return Err(Error::arg("§11.1: bit needs nonnegative operands"));
            }
            let i = y
                .to_u64()
                .ok_or_else(|| Error::arg("bit index exceeds host u64"))?;
            BigInt::from(u8::from(x.bit(i)))
        }
        _ => return Err(Error::arg("§11.1: unknown scalar operator")),
    })
}
fn size(v: &Value, env: &Env) -> Result<usize> {
    scalar(v, env)?
        .to_usize()
        .ok_or_else(|| Error::arg("§11.1: nonnegative size exceeds host usize"))
}
fn wire(v: &Value, env: &Env, host: bool) -> Result<Vec<String>> {
    if let Some(s) = v.as_str() {
        if let Some((base, index)) = s.split_once('[') {
            if !index.ends_with(']') {
                return Err(Error::arg("malformed wire subscript"));
            }
            let index = canonical::parse(index[..index.len() - 1].as_bytes())?;
            let wires = wire(&json!(base), env, host)?;
            let i = size(&index, env)?;
            return wires
                .get(i)
                .cloned()
                .map(|w| vec![w])
                .ok_or_else(|| Error::arg("wire subscript outside bundle"));
        }
        return match env.get(s) {
            Some(
                Binding::Wires(w)
                | Binding::Environment(w)
                | Binding::Region { wires: w, .. }
                | Binding::Allocation { wires: w, .. },
            ) => Ok(w.clone()),
            _ if host => Ok(vec![s.into()]),
            _ => Err(Error::arg(format!("§11.1: unbound wire {s}"))),
        };
    }
    if let Some(a) = v.as_array() {
        let mut w = Vec::new();
        for v in a {
            w.extend(wire(v, env, host)?);
        }
        return Ok(w);
    }
    fields(v, &["range", "from", "to"], &[])?;
    let w = wire(&v["range"], env, host)?;
    let from = size(&v["from"], env)?;
    let to = size(&v["to"], env)?;
    if from > to || to > w.len() {
        return Err(Error::arg("§11.1: invalid wire range"));
    }
    Ok(w[from..to].to_vec())
}
fn validate_formals(params: &[Value]) -> Result<()> {
    let mut names = BTreeSet::new();
    for p in params {
        let name = if p.is_string() {
            text(p)?
        } else {
            text(&p["name"])?
        };
        if name.is_empty() || !names.insert(name) {
            return Err(Error::arg("§11.1: empty or duplicate formal"));
        }
        if p.is_string() {
            continue;
        }
        match text(&p["type"])? {
            "integer" | "angle" | "body" | "symbol" | "environment" | "event" | "image"
            | "class" | "allocation" => fields(p, &["name", "type"], &[])?,
            "wire" => fields(p, &["name", "type", "width"], &[])?,
            "templates" => fields(p, &["name", "type", "length"], &[])?,
            "arguments" => fields(p, &["name", "type", "symbol"], &[])?,
            "value" => fields(p, &["name", "type", "isa_type"], &[])?,
            "records" => {
                fields(p, &["name", "type", "fields", "order"], &[])?;
                validate_formals(array(&p["fields"])?)?;
                array(&p["order"])?;
            }
            "region" => {
                fields(p, &["name", "type", "address", "cell", "role"], &[])?;
                if !["in", "mut"].contains(&text(&p["role"])?) {
                    return Err(Error::arg("invalid region formal role"));
                }
            }
            _ => return Err(Error::arg("§11.1: unknown formal type")),
        }
    }
    Ok(())
}
fn validate_body(body: &[Value]) -> Result<()> {
    for c in body {
        let required: &[&str] = if c.get("gate").is_some() {
            &["gate", "wires"]
        } else if c.get("alloc").is_some() {
            &["alloc", "width", "class", "state"]
        } else if c.get("release").is_some() {
            &["release"]
        } else if c.get("classical").is_some() {
            &["classical", "point"]
        } else if c.get("for").is_some() {
            &["for", "from", "to", "body"]
        } else if c.get("invoke").is_some() {
            &["invoke", "args"]
        } else if c.get("inverse").is_some() {
            if c["inverse"] != true {
                return Err(Error::arg("inverse must be true"));
            }
            &["inverse", "body"]
        } else if c.get("each").is_some() {
            &["each", "in", "body"]
        } else if c.get("splice").is_some() {
            &["splice"]
        } else if c.get("call").is_some() {
            &["call", "args"]
        } else if c.get("under").is_some() {
            &["under", "body"]
        } else if c.get("image").is_some() {
            &["image", "wires"]
        } else if c.get("apply").is_some() {
            &["apply", "index", "args"]
        } else if c.get("visit").is_some() {
            &["visit", "body"]
        } else if c.get("declare").is_some() {
            &["declare", "width", "class", "state", "body"]
        } else if c.get("acquire").is_some() {
            &["acquire"]
        } else if c.get("relinquish").is_some() {
            &["relinquish"]
        } else {
            return Err(Error::arg("§11.1: unknown constructor"));
        };
        fields(
            c,
            required,
            if c.get("gate").is_some() {
                &["params"]
            } else {
                &[]
            },
        )?;
        if let Some(body) = c.get("body") {
            validate_body(array(body)?)?;
        }
    }
    Ok(())
}
#[derive(Default)]
struct Frame {
    calls: u64,
}
struct Compiler<'a> {
    library: &'a Library,
    next: u64,
    depth: usize,
}
impl Compiler<'_> {
    fn bind(
        &mut self,
        params: &[Value],
        args: &[Value],
        caller: &Env,
        controls: &[String],
        host: bool,
    ) -> Result<Env> {
        validate_formals(params)?;
        if params.len() != args.len() {
            return Err(Error::arg("§11.1: recipe argument count mismatch"));
        }
        let mut env = Env::new();
        for (p, a) in params.iter().zip(args) {
            let (name, ty) = if let Some(n) = p.as_str() {
                (n, "integer")
            } else {
                (text(&p["name"])?, text(&p["type"])?)
            };
            let value = self.actual(ty, p, a, caller, &env, controls, host)?;
            env.insert(name.into(), value);
        }
        Ok(env)
    }
    fn actual(
        &mut self,
        ty: &str,
        p: &Value,
        a: &Value,
        caller: &Env,
        earlier: &Env,
        controls: &[String],
        host: bool,
    ) -> Result<Binding> {
        let named = a.as_str().and_then(|s| caller.get(s));
        Ok(match ty {
            "integer" => Binding::Integer(scalar(a, caller)?),
            "wire" => {
                let w = wire(a, caller, host)?;
                if w.len() != size(&p["width"], earlier)? {
                    return Err(Error::arg("§11.1: wire width mismatch"));
                }
                Binding::Wires(w)
            }
            "environment" => Binding::Environment(wire(a, caller, host)?),
            "angle" => {
                let s = if let Some(Binding::Angle(s)) = named {
                    s.clone()
                } else {
                    text(a)?.into()
                };
                Gate {
                    op: "rx".into(),
                    wires: vec!["validation".into()],
                    controls: vec![],
                    theta: Some(s.clone()),
                }
                .validate()?;
                Binding::Angle(s)
            }
            "body" => {
                if let Some(Binding::Body(c)) = named {
                    Binding::Body(c.clone())
                } else {
                    let body = array(a)?.clone();
                    validate_body(&body)?;
                    Binding::Body(Capture {
                        body,
                        env: caller.clone(),
                        controls: controls.to_vec(),
                    })
                }
            }
            "templates" => {
                let t = if let Some(Binding::Templates(t)) = named {
                    t.clone()
                } else {
                    array(a)?
                        .iter()
                        .map(|t| {
                            fields(t, &["parameters", "body"], &[])?;
                            let parameters = array(&t["parameters"])?.clone();
                            validate_formals(&parameters)?;
                            let body = array(&t["body"])?.clone();
                            validate_body(&body)?;
                            Ok(Template {
                                parameters,
                                capture: Capture {
                                    body,
                                    env: caller.clone(),
                                    controls: controls.to_vec(),
                                },
                            })
                        })
                        .collect::<Result<Vec<_>>>()?
                };
                if t.len() != size(&p["length"], earlier)? {
                    return Err(Error::arg("§11.1: template length mismatch"));
                }
                Binding::Templates(t)
            }
            "symbol" => {
                let s = if let Some(Binding::Symbol(s)) = named {
                    s.clone()
                } else {
                    text(a)?.into()
                };
                if !canonical::is_hash(&s) || !self.library.symbols.contains_key(&s) {
                    return Err(Error::arg("§11.1: unbound closure hash"));
                }
                Binding::Symbol(s)
            }
            "arguments" => {
                let Some(Binding::Symbol(symbol)) = earlier.get(text(&p["symbol"])?) else {
                    return Err(Error::arg("arguments formal has no earlier symbol"));
                };
                if let Some(Binding::Arguments { symbol: s, values }) = named {
                    if s != symbol {
                        return Err(Error::arg("arguments vector closure mismatch"));
                    }
                    Binding::Arguments {
                        symbol: s.clone(),
                        values: values.clone(),
                    }
                } else {
                    let sig = &self.library.symbols[symbol].parameters;
                    let env = self.bind(sig, array(a)?, caller, controls, host)?;
                    let values = sig
                        .iter()
                        .map(|p| {
                            env[if p.is_string() {
                                p.as_str().unwrap()
                            } else {
                                p["name"].as_str().unwrap()
                            }]
                            .clone()
                        })
                        .collect();
                    Binding::Arguments {
                        symbol: symbol.clone(),
                        values,
                    }
                }
            }
            "event" => {
                let v = if let Some(Binding::Event(v)) = named {
                    v.clone()
                } else {
                    a.clone()
                };
                fields(&v, &["instruction", "point"], &[])?;
                text(&v["instruction"])?;
                text(&v["point"])?;
                Binding::Event(v)
            }
            "image" => {
                let s = if let Some(Binding::Image(s)) = named {
                    s.clone()
                } else {
                    text(a)?.into()
                };
                if s.len() % 2 != 0
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Error::arg("invalid image hex"));
                }
                Binding::Image(s)
            }
            "class" => {
                let s = if let Some(Binding::Class(s)) = named {
                    s.clone()
                } else {
                    fields(a, &["class"], &[])?;
                    text(&a["class"])?.into()
                };
                if s.is_empty() {
                    return Err(Error::arg("empty source class"));
                }
                Binding::Class(s)
            }
            "allocation" => match named {
                Some(b @ Binding::Allocation { .. }) => b.clone(),
                _ => return Err(Error::arg("unbound allocation declaration")),
            },
            "region" => {
                let b = if let Some(b @ Binding::Region { .. }) = named {
                    b.clone()
                } else {
                    fields(a, &["address", "cell", "role", "origin", "cells"], &[])?;
                    Binding::Region {
                        address: uint(&a["address"])?,
                        cell: uint(&a["cell"])?,
                        role: text(&a["role"])?.into(),
                        origin: text(&a["origin"])?.into(),
                        wires: wire(&a["cells"], caller, host)?,
                    }
                };
                let Binding::Region {
                    address,
                    cell,
                    ref role,
                    ref origin,
                    ref wires,
                } = b
                else {
                    unreachable!()
                };
                let width = (BigInt::one()
                    << usize::try_from(address)
                        .map_err(|_| Error::arg("region address host limit"))?)
                    * cell;
                if width.to_usize() != Some(wires.len())
                    || address != size(&p["address"], earlier)? as u64
                    || cell == 0
                    || cell != size(&p["cell"], earlier)? as u64
                    || role != text(&p["role"])?
                    || !(origin.starts_with("interface:") || origin.starts_with("local:"))
                    || origin.ends_with(':')
                {
                    return Err(Error::arg("§11.1: region shape, role or origin mismatch"));
                }
                b
            }
            "records" => {
                if let Some(Binding::Records { schema, records }) = named {
                    if schema != p {
                        return Err(Error::arg("record forwarding schema mismatch"));
                    }
                    Binding::Records {
                        schema: schema.clone(),
                        records: records.clone(),
                    }
                } else {
                    let ps = array(&p["fields"])?;
                    let order = array(&p["order"])?;
                    let mut last: Option<Vec<BigInt>> = None;
                    let mut records = Vec::new();
                    for r in array(a)? {
                        let names: Vec<_> = ps
                            .iter()
                            .map(|p| {
                                if p.is_string() {
                                    p.as_str().unwrap()
                                } else {
                                    p["name"].as_str().unwrap()
                                }
                            })
                            .collect();
                        fields(r, &names, &[])?;
                        let mut env = earlier.clone();
                        let mut resolved = Env::new();
                        for (fp, name) in ps.iter().zip(&names) {
                            let ft = if fp.is_string() {
                                "integer"
                            } else {
                                text(&fp["type"])?
                            };
                            let b = self.actual(ft, fp, &r[*name], caller, &env, controls, host)?;
                            env.insert((*name).into(), b.clone());
                            resolved.insert((*name).into(), b);
                        }
                        let key = order
                            .iter()
                            .map(|n| match resolved.get(text(n)?) {
                                Some(Binding::Integer(i)) => Ok(i.clone()),
                                _ => Err(Error::arg("record order must name integer field")),
                            })
                            .collect::<Result<Vec<_>>>()?;
                        if !order.is_empty() && last.as_ref().is_some_and(|k| k >= &key) {
                            return Err(Error::arg("§11.1: records not strictly ordered"));
                        }
                        last = Some(key);
                        records.push(resolved);
                    }
                    Binding::Records {
                        schema: p.clone(),
                        records,
                    }
                }
            }
            "value" => {
                let isa_type = p["isa_type"].clone();
                let value = if let Some(Binding::Value {
                    isa_type: t,
                    value: v,
                }) = named
                {
                    if t != &isa_type {
                        return Err(Error::arg("value forwarding type mismatch"));
                    }
                    v.clone()
                } else {
                    a.clone()
                };
                validate_value(&isa_type, &value)?;
                Binding::Value { isa_type, value }
            }
            _ => return Err(Error::arg("unknown formal type")),
        })
    }
    fn compile(
        &mut self,
        body: &[Value],
        env: &mut Env,
        controls: &[String],
        frame: &mut Frame,
        inverse_scope: bool,
    ) -> Result<Vec<Leaf>> {
        self.depth += 1;
        if self.depth > 256 {
            return Err(Error::arg("§11.1: expansion host nesting limit 256"));
        }
        let mut out = Vec::new();
        for c in body {
            let mut emit = |op| {
                out.push(Leaf {
                    ordinal: 0,
                    calls: vec![],
                    op,
                })
            };
            if let Some(op) = c.get("gate") {
                let wires = wire(&c["wires"], env, false)?;
                let params = c.get("params").map(array).transpose()?;
                let theta = if let Some(p) = params {
                    if p.len() != 1 {
                        return Err(Error::arg("gate angle arity"));
                    }
                    Some(match p[0].as_str().and_then(|s| env.get(s)) {
                        Some(Binding::Angle(s)) => s.clone(),
                        _ => text(&p[0])?.into(),
                    })
                } else {
                    None
                };
                let g = Gate {
                    op: text(op)?.into(),
                    wires,
                    controls: controls.to_vec(),
                    theta,
                };
                g.validate()?;
                emit(Operation::Gate(g));
            } else if let Some(name) = c.get("alloc") {
                let name = text(name)?;
                let width = size(&c["width"], env)?;
                let class = text(&c["class"])?.to_owned();
                let state = text(&c["state"])?.to_owned();
                if state != "clean" {
                    return Err(Error::arg("library scratch must be clean"));
                }
                let binding = if inverse_scope {
                    env.get(name)
                        .filter(|b| matches!(b, Binding::Allocation { .. }))
                        .cloned()
                } else {
                    None
                };
                let b = binding.unwrap_or_else(|| {
                    let identity = format!("scratch/{name}/{}", self.next);
                    self.next += 1;
                    let wires = (0..width).map(|i| format!("{identity}[{i}]")).collect();
                    Binding::Allocation {
                        identity,
                        wires,
                        class: class.clone(),
                        state: state.clone(),
                    }
                });
                let Binding::Allocation {
                    ref identity,
                    ref wires,
                    ..
                } = b
                else {
                    unreachable!()
                };
                if wires.len() != width {
                    return Err(Error::arg("inverse scratch width mismatch"));
                }
                emit(Operation::Alloc {
                    identity: identity.clone(),
                    wires: wires.clone(),
                    class,
                    state,
                });
                env.insert(name.into(), b);
            } else if let Some(name) = c.get("release") {
                let Some(Binding::Allocation {
                    identity,
                    wires,
                    class,
                    state,
                }) = env.get(text(name)?)
                else {
                    return Err(Error::arg("§11.1: unbound allocation release"));
                };
                emit(Operation::Release {
                    identity: identity.clone(),
                    wires: wires.clone(),
                    class: class.clone(),
                    state: state.clone(),
                });
            } else if c.get("inverse").is_some() {
                let leaves =
                    self.compile(array(&c["body"])?, &mut env.clone(), controls, frame, true)?;
                out.extend(inverse(&leaves));
            } else if let Some(name) = c.get("for") {
                let from = scalar(&c["from"], env)?;
                let to = scalar(&c["to"], env)?;
                if from.is_negative() || to < from {
                    return Err(Error::arg("§11.1: invalid counted-loop range"));
                }
                let mut i = from;
                while i < to {
                    let mut local = env.clone();
                    local.insert(text(name)?.into(), Binding::Integer(i.clone()));
                    out.extend(self.compile(
                        array(&c["body"])?,
                        &mut local,
                        controls,
                        frame,
                        inverse_scope,
                    )?);
                    i += 1;
                }
            } else if let Some(name) = c.get("each") {
                for w in wire(&c["in"], env, false)? {
                    let mut local = env.clone();
                    local.insert(text(name)?.into(), Binding::Wires(vec![w]));
                    out.extend(self.compile(
                        array(&c["body"])?,
                        &mut local,
                        controls,
                        frame,
                        inverse_scope,
                    )?);
                }
            } else if let Some(name) = c.get("invoke") {
                let r = self
                    .library
                    .recipes
                    .get(text(name)?)
                    .ok_or_else(|| Error::arg("unbound invoked recipe"))?;
                let bound = self.bind(
                    array(&r["parameters"])?,
                    array(&c["args"])?,
                    env,
                    controls,
                    false,
                )?;
                let mut local=crate::ref_contract::named_scope(r,bound)?;
                let effective = environment_controls(&local, controls);
                out.extend(self.compile(
                    array(&r["body"])?,
                    &mut local,
                    &effective,
                    frame,
                    inverse_scope,
                )?);
            } else if let Some(name) = c.get("splice") {
                let Some(Binding::Body(capture)) = env.get(text(name)?).cloned() else {
                    return Err(Error::arg("unbound body splice"));
                };
                let mut eff = capture.controls.clone();
                union(&mut eff, controls);
                out.extend(self.compile(
                    &capture.body,
                    &mut capture.env.clone(),
                    &eff,
                    frame,
                    inverse_scope,
                )?);
            } else if c.get("under").is_some() {
                let mut effective = controls.to_vec();
                union(&mut effective, &wire(&c["under"], env, false)?);
                out.extend(self.compile(
                    array(&c["body"])?,
                    &mut env.clone(),
                    &effective,
                    frame,
                    inverse_scope,
                )?);
            } else if let Some(name) = c.get("image") {
                let Some(Binding::Image(s)) = env.get(text(name)?) else {
                    return Err(Error::arg("unbound image"));
                };
                let w = wire(&c["wires"], env, false)?;
                if !s.is_empty() && s.len() * 4 != w.len() {
                    return Err(Error::arg("image byte/wire width mismatch"));
                }
                for (j, chunk) in s.as_bytes().chunks(2).enumerate() {
                    let n = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16)
                        .map_err(|_| Error::arg("image hex"))?;
                    for i in 0..8 {
                        if n & (1 << i) != 0 {
                            emit(Operation::Gate(Gate {
                                op: "x".into(),
                                wires: vec![w[j * 8 + i].clone()],
                                controls: vec![],
                                theta: None,
                            }));
                        }
                    }
                }
            } else if let Some(name) = c.get("apply") {
                let Some(Binding::Templates(ts)) = env.get(text(name)?).cloned() else {
                    return Err(Error::arg("unbound templates"));
                };
                let index = size(&c["index"], env)?;
                let t = ts
                    .get(index)
                    .ok_or_else(|| Error::arg("template index out of bounds"))?;
                let bound = self.bind(&t.parameters, array(&c["args"])?, env, controls, false)?;
                let mut local = t.capture.env.clone();
                local.extend(bound);
                let mut eff = t.capture.controls.clone();
                union(&mut eff, controls);
                eff = environment_controls(&local, &eff);
                out.extend(self.compile(
                    &t.capture.body,
                    &mut local,
                    &eff,
                    frame,
                    inverse_scope,
                )?);
            } else if let Some(name) = c.get("visit") {
                let Some(Binding::Records { records, .. }) = env.get(text(name)?).cloned() else {
                    return Err(Error::arg("unbound descriptor records"));
                };
                for record in records {
                    let mut local = env.clone();
                    local.extend(record);
                    out.extend(self.compile(
                        array(&c["body"])?,
                        &mut local,
                        controls,
                        frame,
                        inverse_scope,
                    )?);
                }
            } else if let Some(name) = c.get("declare") {
                let name = text(name)?;
                if env.contains_key(name) {
                    return Err(Error::arg("allocation declaration shadows a binding"));
                }
                let width = size(&c["width"], env)?;
                let class = match self.actual(
                    "class",
                    &Value::Null,
                    &c["class"],
                    env,
                    &Env::new(),
                    controls,
                    false,
                )? {
                    Binding::Class(s) => s,
                    _ => unreachable!(),
                };
                let state = text(&c["state"])?.to_owned();
                if !["clean", "borrowed"].contains(&state.as_str()) {
                    return Err(Error::arg("invalid declared allocation state"));
                }
                let identity = format!("declaration/{name}/{}", self.next);
                self.next += 1;
                let wires = (0..width).map(|i| format!("{identity}[{i}]")).collect();
                let mut local = env.clone();
                local.insert(
                    name.into(),
                    Binding::Allocation {
                        identity: identity.clone(),
                        wires,
                        class,
                        state,
                    },
                );
                let leaves = self.compile(
                    array(&c["body"])?,
                    &mut local,
                    controls,
                    frame,
                    inverse_scope,
                )?;
                let mut live = false;
                for l in &leaves {
                    match &l.op {
                        Operation::Alloc { identity: id, .. } if id == &identity => {
                            if live {
                                return Err(Error::arg("double declaration acquisition"));
                            }
                            live = true;
                        }
                        Operation::Release { identity: id, .. } if id == &identity => {
                            if !live {
                                return Err(Error::arg("inactive declaration release"));
                            }
                            live = false;
                        }
                        _ => {}
                    }
                }
                if live {
                    return Err(Error::arg(
                        "§11.1: declaration exits with a live allocation",
                    ));
                }
                out.extend(leaves);
            } else if c.get("acquire").is_some() || c.get("relinquish").is_some() {
                let acquire = c.get("acquire").is_some();
                let name = &c[if acquire { "acquire" } else { "relinquish" }];
                let Some(Binding::Allocation {
                    identity,
                    wires,
                    class,
                    state,
                }) = env.get(text(name)?)
                else {
                    return Err(Error::arg("unbound allocation declaration"));
                };
                let (identity, wires, class, state) = (
                    identity.clone(),
                    wires.clone(),
                    class.clone(),
                    state.clone(),
                );
                emit(if acquire {
                    Operation::Alloc {
                        identity,
                        wires,
                        class,
                        state,
                    }
                } else {
                    Operation::Release {
                        identity,
                        wires,
                        class,
                        state,
                    }
                });
            } else if let Some(op) = c.get("classical") {
                let point = text(&c["point"])?;
                let event = match env.get(point) {
                    Some(Binding::Event(e)) => {
                        let instruction = text(&e["instruction"])?;
                        let opcode = instruction
                            .split(" = ")
                            .last()
                            .unwrap()
                            .split_whitespace()
                            .next()
                            .unwrap_or("");
                        if opcode != text(op)? && !opcode.starts_with(&format!("{}.", text(op)?)) {
                            return Err(Error::arg("classical event opcode mismatch"));
                        }
                        e.clone()
                    }
                    _ => json!({"point":point}),
                };
                emit(Operation::Classical {
                    opcode: text(op)?.into(),
                    event,
                });
            } else if let Some(name) = c.get("call") {
                let Some(Binding::Symbol(symbol)) = env.get(text(name)?).cloned() else {
                    return Err(Error::arg("unbound semantic-call symbol"));
                };
                let s = &self.library.symbols[&symbol];
                let mut local = if let Some(Binding::Arguments {
                    symbol: hash,
                    values,
                }) = c["args"].as_str().and_then(|s| env.get(s))
                {
                    if hash != &symbol {
                        return Err(Error::arg("call arguments closure mismatch"));
                    }
                    let names = s.parameters.iter().map(|p| {
                        if p.is_string() {
                            p.as_str().unwrap()
                        } else {
                            p["name"].as_str().unwrap()
                        }
                    });
                    names
                        .zip(values.iter().cloned())
                        .map(|(n, b)| (n.into(), b))
                        .collect()
                } else {
                    self.bind(&s.parameters, array(&c["args"])?, env, controls, false)?
                };
                let index = frame.calls;
                frame.calls += 1;
                let effective = environment_controls(&local, controls);
                let mut child = Frame::default();
                let mut leaves =
                    self.compile(&s.body, &mut local, &effective, &mut child, inverse_scope)?;
                number(&mut leaves);
                for leaf in &mut leaves {
                    leaf.calls.insert(
                        0,
                        CallEdge {
                            symbol: symbol.clone(),
                            call_index: index,
                        },
                    );
                }
                out.extend(leaves);
            } else {
                return Err(Error::arg("unknown constructor"));
            }
        }
        self.depth -= 1;
        Ok(out)
    }
}
pub fn validate_value(ty: &Value, v: &Value) -> Result<()> {
    if ty == "token" {
        let a = array(v)?;
        if a.len() != 4 {
            return Err(Error::arg("token tuple requires four components"));
        }
        for n in a {
            canonical_unsigned(n)?;
        }
        return Ok(());
    }
    if ty == "f64" {
        let s = text(v)?;
        if s.len() != 18
            || !s.starts_with("0x")
            || !s[2..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::arg("f64 value must preserve binary64 bits"));
        }
        return Ok(());
    }
    let width = if let Some(s) = ty.as_str().and_then(|s| s.strip_prefix('i')) {
        s.parse::<u64>()
            .map_err(|_| Error::arg("invalid integer type"))?
    } else {
        fields(ty, &["ptr"], &[])?;
        uint(&ty["ptr"])?
    };
    if width == 0 {
        return Err(Error::arg("integer width must be positive"));
    }
    let n = canonical_unsigned(v)?;
    if n.bits() > width {
        return Err(Error::arg("integer argument out of range"));
    }
    Ok(())
}
fn canonical_unsigned(v: &Value) -> Result<BigInt> {
    let s = text(v)?;
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) || !s.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(Error::arg(
            "value requires canonical unsigned decimal string",
        ));
    }
    s.parse().map_err(|_| Error::arg("invalid unsigned value"))
}
fn validate_lifetimes(leaves: &[Leaf]) -> Result<()> {
    let allocated: BTreeSet<_> = leaves
        .iter()
        .filter_map(|l| match &l.op {
            Operation::Alloc { identity, .. } => Some(identity.clone()),
            _ => None,
        })
        .collect();
    let owned_wires: BTreeSet<_> = leaves
        .iter()
        .filter_map(|l| match &l.op {
            Operation::Alloc { wires, .. } => Some(wires),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect();
    let mut live = BTreeSet::new();
    let mut live_wires = BTreeSet::new();
    let mut scratch = Vec::new();
    for l in leaves {
        match &l.op {
            Operation::Alloc {
                identity, wires, ..
            } => {
                if !live.insert(identity.clone()) {
                    return Err(Error::arg("§11.1: double allocation/acquisition"));
                }
                if identity.starts_with("scratch/") {
                    scratch.push(identity.clone());
                }
                for w in wires {
                    if !live_wires.insert(w.clone()) {
                        return Err(Error::arg("coincident live allocation wires"));
                    }
                }
            }
            Operation::Release {
                identity, wires, ..
            } => {
                if allocated.contains(identity) && !live.remove(identity) {
                    return Err(Error::arg("§11.1: unmatched allocation release"));
                }
                if identity.starts_with("scratch/") && scratch.pop().as_ref() != Some(identity) {
                    return Err(Error::arg(
                        "§11.1: library scratch release is not in reverse allocation order",
                    ));
                }
                for w in wires {
                    live_wires.remove(w);
                }
            }
            Operation::Gate(g) => {
                if g.all_wires()
                    .iter()
                    .any(|w| owned_wires.contains(w) && !live_wires.contains(w))
                {
                    return Err(Error::arg("§11.1: wire use outside allocation lifetime"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn fresh_acquisitions(leaves: &mut [Leaf]) {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut renames: BTreeMap<String, String> = BTreeMap::new();
    let mut active: BTreeMap<String, String> = BTreeMap::new();
    for l in leaves {
        match &mut l.op {
            Operation::Alloc {
                identity, wires, ..
            } if identity.starts_with("declaration/") => {
                let original = identity.clone();
                let count = counts.entry(original.clone()).or_default();
                *count += 1;
                let current = if *count == 1 {
                    original.clone()
                } else {
                    format!("{original}/acquisition/{}", *count)
                };
                for (i, w) in wires.iter_mut().enumerate() {
                    let old = w.clone();
                    *w = format!("{current}[{i}]");
                    renames.insert(old, w.clone());
                }
                active.insert(original, current.clone());
                *identity = current;
            }
            Operation::Gate(g) => {
                for w in g.wires.iter_mut().chain(&mut g.controls) {
                    if let Some(current) = renames.get(w) {
                        *w = current.clone();
                    }
                }
            }
            Operation::Release {
                identity, wires, ..
            } => {
                if let Some(current) = active.remove(identity) {
                    *identity = current;
                }
                for w in wires {
                    if let Some(current) = renames.remove(w) {
                        *w = current;
                    }
                }
            }
            _ => {}
        }
    }
}
/// Independent Boolean simulation for exact permutation witnesses; never basis-prunes expansion.
pub fn simulate_basis(leaves: &[Leaf], bits: &mut BTreeMap<String, bool>) -> Result<()> {
    for leaf in leaves {
        match &leaf.op {
            Operation::Alloc { wires, state, .. } => {
                if state != "clean" {
                    return Err(Error::arg("borrowed basis input must be supplied"));
                }
                for w in wires {
                    if bits.insert(w.clone(), false).is_some() {
                        return Err(Error::arg("duplicate basis wire"));
                    }
                }
            }
            Operation::Release { wires, state, .. } => {
                for w in wires {
                    let bit = bits
                        .remove(w)
                        .ok_or_else(|| Error::arg("missing released basis wire"))?;
                    if state == "clean" && bit {
                        return Err(Error::arg("released scratch is not clean"));
                    }
                }
            }
            Operation::Gate(g) => {
                if !g
                    .control_wires()
                    .iter()
                    .map(|w| {
                        bits.get(w)
                            .copied()
                            .ok_or_else(|| Error::arg("missing basis control"))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .iter()
                    .all(|b| *b)
                {
                    continue;
                }
                match g.op.as_str() {
                    "x" | "cx" | "ccx" | "mcx" => {
                        let t = g.wires.last().unwrap();
                        *bits
                            .get_mut(t)
                            .ok_or_else(|| Error::arg("missing basis target"))? ^= true;
                    }
                    "swap" => {
                        let a = *bits
                            .get(&g.wires[0])
                            .ok_or_else(|| Error::arg("missing swap wire"))?;
                        let b = *bits
                            .get(&g.wires[1])
                            .ok_or_else(|| Error::arg("missing swap wire"))?;
                        bits.insert(g.wires[0].clone(), b);
                        bits.insert(g.wires[1].clone(), a);
                    }
                    _ => {
                        return Err(Error::arg(
                            "gate requires quantum rather than basis simulation",
                        ));
                    }
                }
            }
            Operation::Classical { .. } => {
                return Err(Error::arg("classical leaf requires retained history"));
            }
        }
    }
    Ok(())
}
