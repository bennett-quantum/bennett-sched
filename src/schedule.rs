//! §11 schema and operation-local maximal selector resolution.
use crate::{Error, Result, canonical, decimal};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const EXPANSION_HASH: &str = "5de4e4995350313e50217880ba1b95de39cd043283b726337d4894efb005d197";
pub fn expansion_binding() -> Value {
    json!({"name":"ref","version":"0.9","hash":EXPANSION_HASH})
}

pub fn fields(v: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let m = v
        .as_object()
        .ok_or_else(|| Error::arg("expected JSON object"))?;
    for r in required {
        if !m.contains_key(*r) {
            return Err(Error::arg(format!("missing field {r}")));
        }
    }
    for k in m.keys() {
        if !required.contains(&k.as_str()) && !optional.contains(&k.as_str()) {
            return Err(Error::arg(format!("unknown field {k}")));
        }
    }
    Ok(())
}
pub fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| Error::arg("expected string"))
}
pub fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| Error::arg("expected array"))
}
pub fn uint(v: &Value) -> Result<u64> {
    if let Some(n) = v.as_u64() {
        return Ok(n);
    }
    if let Some(s) = v.as_str() {
        if !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && (s == "0" || !s.starts_with('0'))
        {
            return s
                .parse()
                .map_err(|_| Error::arg("integer exceeds host u64"));
        }
    }
    Err(Error::arg("expected canonical unsigned integer"))
}
fn uints(v: &Value) -> Result<Vec<u64>> {
    array(v)?.iter().map(uint).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector<T> {
    Any,
    Exact(T),
}
impl<T: PartialEq> Selector<T> {
    fn matches(&self, t: Option<&T>) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(v) => t == Some(v),
        }
    }
    fn concrete(&self) -> usize {
        usize::from(matches!(self, Self::Exact(_)))
    }
}
fn selector<T>(v: &Value, parse: impl Fn(&Value) -> Result<T>) -> Result<Selector<T>> {
    if v == "*" {
        Ok(Selector::Any)
    } else {
        Ok(Selector::Exact(parse(v)?))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Fwd,
    Rev,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coordinate {
    pub step: Option<u64>,
    pub phase: Option<Phase>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathElement {
    pub symbol: String,
    pub point: String,
    pub iteration: Selector<Vec<u64>>,
    pub instance: Selector<Vec<u64>>,
    pub call_index: Selector<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub symbol: String,
    pub point: String,
    pub iteration: Vec<u64>,
    pub instance: Vec<u64>,
    pub call_index: Option<u64>,
    pub entered_oracle: Option<Coordinate>,
}
impl Occurrence {
    pub fn json(&self) -> Value {
        let mut v = json!({"symbol":self.symbol,"point":self.point,"iteration":self.iteration,"instance":self.instance});
        if let Some(i) = self.call_index {
            v["call_index"] = json!(i);
        }
        v
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cleanup,
    Gate,
    Alloc,
    Start,
    Release,
    Classical,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cleanup => "cleanup",
            Self::Gate => "gate",
            Self::Alloc => "alloc",
            Self::Start => "start",
            Self::Release => "release",
            Self::Classical => "classical",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Location {
    pub root: Coordinate,
    pub path: Vec<Occurrence>,
    pub generated: u64,
    pub kind: Kind,
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: Vec<PathElement>,
    pub step: Selector<u64>,
    pub phase: Selector<Phase>,
    pub generated: Selector<u64>,
    pub kind: Kind,
    pub decision: Value,
}
impl Entry {
    pub fn matches(&self, op: &Location) -> bool {
        let kind_ok = if op.kind == Kind::Cleanup {
            self.kind == Kind::Cleanup
        } else {
            self.kind != Kind::Cleanup && (self.kind == Kind::Start || self.kind == op.kind)
        };
        if !kind_ok
            || self.path.len() > op.path.len()
            || !self.generated.matches(Some(&op.generated))
        {
            return false;
        }
        let mut coord = op.root;
        for (i, (sel, point)) in self.path.iter().zip(&op.path).enumerate() {
            if sel.symbol != point.symbol
                || sel.point != point.point
                || !sel.iteration.matches(Some(&point.iteration))
                || !sel.instance.matches(Some(&point.instance))
                || !sel.call_index.matches(point.call_index.as_ref())
            {
                return false;
            }
            if i + 1 < op.path.len() {
                if let Some(c) = point.entered_oracle {
                    coord = c;
                }
            }
        }
        self.step.matches(coord.step.as_ref()) && self.phase.matches(coord.phase.as_ref())
    }
    pub fn specificity(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.path.len(),
            self.path.iter().map(|p| p.iteration.concrete()).sum(),
            self.step.concrete(),
            self.phase.concrete(),
            self.generated.concrete(),
        )
    }
}
pub fn resolve<'a>(
    entries: &'a [Entry],
    op: &Location,
    kind: Kind,
) -> Result<Option<(usize, &'a Entry)>> {
    let mut best: Option<(usize, &Entry)> = None;
    let mut tied = false;
    for (i, e) in entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == kind && e.matches(op))
    {
        match best {
            None => {
                best = Some((i, e));
                tied = false;
            }
            Some((_, b)) if e.specificity() > b.specificity() => {
                best = Some((i, e));
                tied = false;
            }
            Some((_, b)) if e.specificity() == b.specificity() => tied = true,
            _ => {}
        }
    }
    if tied {
        return Err(Error::infeasible(
            "§11: multiple maximal matching schedule entries",
        ));
    }
    Ok(best)
}
fn parse_point(v: &Value) -> Result<String> {
    let p = text(v)?;
    if ["$construct", "$step", "$commit", "$release"].contains(&p) {
        return Ok(p.into());
    }
    let (block, ordinal) = p
        .rsplit_once(':')
        .ok_or_else(|| Error::arg("§11: invalid program point"))?;
    if block.is_empty()
        || block.starts_with('$')
        || ordinal.is_empty()
        || !ordinal.bytes().all(|b| b.is_ascii_digit())
        || (ordinal.len() > 1 && ordinal.starts_with('0'))
    {
        return Err(Error::arg("§11: invalid program point"));
    }
    Ok(p.into())
}
pub fn parse_entry(v: &Value) -> Result<Entry> {
    let kind = match text(&v["kind"])? {
        "cleanup" => Kind::Cleanup,
        "gate" => Kind::Gate,
        "alloc" => Kind::Alloc,
        "start" => Kind::Start,
        _ => return Err(Error::arg("§11: unknown entry kind")),
    };
    fields(
        v,
        &["path", "step", "phase", "generated", "kind", kind.name()],
        &[],
    )?;
    let path = array(&v["path"])?
        .iter()
        .map(|p| {
            fields(
                p,
                &["symbol", "point", "iteration"],
                &["instance", "call_index"],
            )?;
            let symbol = text(&p["symbol"])?;
            if !canonical::is_hash(symbol) {
                return Err(Error::arg("§11: path symbol must be 64 lowercase hex"));
            }
            Ok(PathElement {
                symbol: symbol.into(),
                point: parse_point(&p["point"])?,
                iteration: selector(&p["iteration"], uints)?,
                instance: selector(p.get("instance").unwrap_or(&json!("*")), uints)?,
                call_index: selector(p.get("call_index").unwrap_or(&json!("*")), uint)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let decision = v[kind.name()].clone();
    match kind {
        Kind::Cleanup => {
            fields(&decision, &["realisation"], &[])?;
            if !["reverse", "recompute"].contains(&text(&decision["realisation"])?) {
                return Err(Error::arg("§6.4: invalid cleanup realisation"));
            }
        }
        Kind::Gate => {
            fields(&decision, &["realisation"], &["epsilon_share"])?;
            if text(&decision["realisation"])?.is_empty() {
                return Err(Error::arg("empty gate realisation"));
            }
            if let Some(v) = decision.get("epsilon_share") {
                decimal::nonnegative(v)?;
            }
        }
        Kind::Alloc => {
            fields(&decision, &["class"], &["vertices"])?;
            let class = text(&decision["class"])?;
            if class.is_empty() || class == "any" {
                return Err(Error::arg("§11: allocation class must be concrete"));
            }
            if let Some(v) = decision.get("vertices") {
                let a = uints(v)?;
                if a.iter().collect::<BTreeSet<_>>().len() != a.len() {
                    return Err(Error::arg("duplicate placement vertex"));
                }
            }
        }
        Kind::Start => {
            fields(&decision, &["time"], &[])?;
            decimal::nonnegative(&decision["time"])?;
        }
        _ => unreachable!(),
    }
    Ok(Entry {
        path,
        step: selector(&v["step"], uint)?,
        phase: selector(&v["phase"], |p| match text(p)? {
            "fwd" => Ok(Phase::Fwd),
            "rev" => Ok(Phase::Rev),
            _ => Err(Error::arg("invalid phase")),
        })?,
        generated: selector(&v["generated"], uint)?,
        kind,
        decision,
    })
}
#[derive(Clone, Debug)]
pub struct Document {
    pub value: Value,
    pub entries: Vec<Entry>,
}
impl Document {
    pub fn parse(v: Value) -> Result<Self> {
        fields(&v, &["bindings", "entries"], &["interface_vertices"])?;
        let b = &v["bindings"];
        fields(
            b,
            &[
                "closure_hash",
                "target_hash",
                "region",
                "variant",
                "inherited_controls",
                "budget",
                "params_hash",
                "epsilon",
                "expansion",
                "scheduler",
            ],
            &[],
        )?;
        for key in ["closure_hash", "target_hash", "params_hash"] {
            if !canonical::is_hash(text(&b[key])?) {
                return Err(Error::arg(format!("invalid {key}")));
            }
        }
        for key in ["region", "variant"] {
            if text(&b[key])?.is_empty() {
                return Err(Error::arg(format!("empty {key}")));
            }
        }
        uint(&b["inherited_controls"])?;
        if uints(&b["budget"])?.len() != 4 {
            return Err(Error::arg("budget must have four components"));
        }
        decimal::nonnegative(&b["epsilon"])?;
        fields(&b["scheduler"], &["name", "version"], &[])?;
        for key in ["name", "version"] {
            if text(&b["scheduler"][key])?.is_empty() {
                return Err(Error::arg("empty scheduler identity"));
            }
        }
        fields(&b["expansion"], &["name", "version", "hash"], &[])?;
        if b["expansion"] != expansion_binding() {
            return Err(Error::infeasible(
                "§11: bound expansion library is unavailable",
            ));
        }
        if let Some(placements) = v.get("interface_vertices") {
            for a in array(placements)? {
                uints(a)?;
            }
        }
        let entries = array(&v["entries"])?
            .iter()
            .map(parse_entry)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { value: v, entries })
    }
}
