//! Canonical object ingestion and independently implemented §9.2 envelopes/SCC identity.
use crate::{
    Error, Result, Status, canonical,
    expansion::validate_value,
    schedule::{array, fields, text, uint},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub ty: String,
    pub role: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Node {
    pub point: String,
    pub line: String,
    pub lhs: Option<String>,
    pub op: String,
    pub operands: Vec<String>,
    pub bodies: Vec<Vec<Node>>,
}
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub kind: String,
    pub signature: Value,
    pub header: String,
    pub canonical: String,
    pub groups: Vec<Vec<Parameter>>,
    pub regs: Vec<Parameter>,
    pub body: Vec<Node>,
}
#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub signature: Value,
    pub interface_hash: String,
}
#[derive(Clone, Debug)]
pub struct Object {
    pub ptr_width: u64,
    pub versions: Value,
    pub symbols: BTreeMap<String, Symbol>,
    pub functions: BTreeMap<String, Function>,
    pub metadata: BTreeMap<String, Value>,
    pub data: BTreeMap<String, String>,
    pub object_hash: String,
    interfaces: BTreeMap<String, String>,
    closures: BTreeMap<String, Result<String>>,
}
pub fn name(s: &str) -> bool {
    let mut b = s.bytes();
    b.next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && b.all(|c| c.is_ascii_alphanumeric() || b"_.".contains(&c))
}
pub fn words(s: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut begin = 0;
    let mut nesting = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => nesting += 1,
            ')' => {
                nesting -= 1;
                if nesting < 0 {
                    return Err(Error::arg("unmatched operand parenthesis"));
                }
            }
            ' ' if nesting == 0 => {
                if begin < i {
                    out.push(s[begin..i].into());
                }
                begin = i + 1;
            }
            _ => {}
        }
    }
    if nesting != 0 {
        return Err(Error::arg("unclosed operand list"));
    }
    if begin < s.len() {
        out.push(s[begin..].into());
    }
    Ok(out)
}
pub fn list(s: &str) -> Result<Vec<String>> {
    if !s.starts_with('(') || !s.ends_with(')') {
        return Err(Error::arg("expected parenthesized operand list"));
    }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        Ok(vec![])
    } else {
        Ok(inner.split(", ").map(str::to_owned).collect())
    }
}
fn parameters(s: &str) -> Result<Vec<Parameter>> {
    list(s)?
        .iter()
        .map(|s| {
            let (n, t) = s
                .split_once(':')
                .ok_or_else(|| Error::arg("parameter requires name:type"))?;
            if !name(n) {
                return Err(Error::arg("invalid parameter name"));
            }
            let mut words = t.split(' ');
            let ty = words.next().unwrap().into();
            let role = words.next().map(str::to_owned);
            if words.next().is_some() {
                return Err(Error::arg("invalid parameter role"));
            }
            Ok(Parameter {
                name: n.into(),
                ty,
                role,
            })
        })
        .collect()
}
fn metadata_numbers(v: &Value) -> Result<()> {
    match v {
        Value::Number(n) => {
            if n.as_f64()
                .is_none_or(|f| !f.is_finite() || f >= 9007199254740992.0)
            {
                return Err(Error::arg(
                    "§9.1: metadata number >= 2^53 must be a decimal string",
                ));
            }
        }
        Value::Array(a) => {
            for v in a {
                metadata_numbers(v)?;
            }
        }
        Value::Object(o) => {
            for v in o.values() {
                metadata_numbers(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}
impl Object {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let source =
            std::str::from_utf8(bytes).map_err(|_| Error::arg("§16.1: object is not UTF-8"))?;
        if !source.ends_with('\n') || source.contains(['\r', '\t']) {
            return Err(Error::arg("§16.1: object requires LF lines and final LF"));
        }
        let (header, rest) = source
            .split_once('\n')
            .ok_or_else(|| Error::arg("missing object header"))?;
        let h: Vec<_> = header.split(' ').collect();
        let version_keys = [
            "isa", "object", "launch", "sched", "cert", "coverage", "target",
        ];
        if h.len() != 9 || h[0] != "bennett-object" {
            return Err(Error::arg("invalid object header"));
        }
        let mut versions = json!({});
        for (i, key) in version_keys.iter().enumerate() {
            if h[i + 1] != format!("{key}=0.9") {
                return Err(Error::new(Status::Version, format!("expected {key}=0.9")));
            }
            versions[*key] = json!("0.9");
        }
        let width = h[8]
            .strip_prefix("ptr_width=")
            .ok_or_else(|| Error::arg("missing ptr_width"))?;
        let ptr_width = uint(&json!(width))?;
        if ptr_width == 0 {
            return Err(Error::new(Status::PtrWidth, "ptr_width must be positive"));
        }
        let order = [
            "symbols",
            "imports",
            "functions",
            "data",
            "devices",
            "layout",
            "schedule",
            "certificate",
            "coverage",
        ];
        let mut sections = Vec::new();
        let mut last = None;
        if !rest.is_empty() {
            if !rest.starts_with("\nsection ") {
                return Err(Error::arg(
                    "§16.1: exactly one blank line required before sections",
                ));
            }
            for raw in rest[1..].split("\n\n") {
                let (section, body) = raw
                    .split_once('\n')
                    .ok_or_else(|| Error::arg("section missing body"))?;
                let key = section
                    .strip_prefix("section ")
                    .ok_or_else(|| Error::arg("unexpected blank line outside section boundary"))?;
                let index = order
                    .iter()
                    .position(|s| *s == key)
                    .ok_or_else(|| Error::arg("§9.1: unknown section"))?;
                if last.is_some_and(|n| n >= index) {
                    return Err(Error::arg("§9.1: duplicate or out-of-order section"));
                }
                last = Some(index);
                let body = body.strip_suffix('\n').unwrap_or(body);
                if body.lines().any(|l| {
                    l.is_empty()
                        || l.ends_with(' ')
                            && !(key == "data"
                                && l.split_once(' ').is_some_and(|(_, h)| h.is_empty()))
                }) {
                    return Err(Error::arg("§16.1: blank/trailing-space line"));
                }
                sections.push((key.to_owned(), body.to_owned()));
            }
        }
        let mut metadata = BTreeMap::new();
        let mut data = BTreeMap::new();
        let mut function_text = None;
        for (key, body) in &sections {
            match key.as_str() {
                "functions" => function_text = Some(body),
                "data" => {
                    for l in body.lines() {
                        let (n, hex) = l
                            .split_once(' ')
                            .ok_or_else(|| Error::arg("data requires NAME SP HEX"))?;
                        if !name(n)
                            || hex.len() % 2 != 0
                            || !hex
                                .bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                            || data.insert(n.to_owned(), hex.to_owned()).is_some()
                        {
                            return Err(Error::arg("invalid or duplicate data symbol"));
                        }
                    }
                }
                _ => {
                    if body.contains('\n') {
                        return Err(Error::arg("metadata must be one JCS line"));
                    }
                    let v = canonical::require_jcs(body.as_bytes())?;
                    if !v.is_object() {
                        return Err(Error::arg("metadata must be object"));
                    }
                    metadata_numbers(&v)?;
                    metadata.insert(key.clone(), v);
                }
            }
        }
        let mut symbols = BTreeMap::new();
        let symbol_table = metadata
            .get("symbols")
            .and_then(Value::as_object)
            .ok_or_else(|| Error::arg("object missing symbols"))?;
        for (index, v) in symbol_table {
            uint(&json!(index))?;
            fields(v, &["name", "kind", "signature", "interface_hash"], &[])?;
            let n = text(&v["name"])?;
            let kind = text(&v["kind"])?;
            if !name(n)
                || !canonical::is_hash(text(&v["interface_hash"])?)
                || v["signature"]["kind"] != kind
                || v["signature"]["abi"] != "0.9"
            {
                return Err(Error::arg("invalid symbol record"));
            }
            let sym = Symbol {
                name: n.into(),
                kind: kind.into(),
                signature: v["signature"].clone(),
                interface_hash: text(&v["interface_hash"])?.into(),
            };
            validate_signature(&sym)?;
            if symbols.insert(n.into(), sym).is_some() {
                return Err(Error::arg("duplicate symbol name"));
            }
        }
        let mut functions = BTreeMap::new();
        if let Some(body) = function_text {
            let lines: Vec<_> = body.lines().collect();
            let mut i = 0;
            while i < lines.len() {
                let start = i;
                let w = words(lines[i])?;
                let (kind, sym_index) = if w.first().is_some_and(|s| s == "fn") {
                    (
                        w.get(1)
                            .ok_or_else(|| Error::arg("missing fn tier"))?
                            .clone(),
                        2,
                    )
                } else {
                    (
                        w.first()
                            .ok_or_else(|| Error::arg("missing function kind"))?
                            .clone(),
                        1,
                    )
                };
                let n = w
                    .get(sym_index)
                    .and_then(|s| s.strip_prefix('@'))
                    .ok_or_else(|| Error::arg("missing function definition"))?;
                let sym = symbols
                    .get(n)
                    .ok_or_else(|| Error::arg("function not in symbol table"))?;
                if sym.kind != kind || w.len() != sym_index + 6 {
                    return Err(Error::arg("function header kind/group mismatch"));
                }
                let groups = w[sym_index + 1..]
                    .iter()
                    .map(|s| parameters(s))
                    .collect::<Result<Vec<_>>>()?;
                i += 1;
                while i < lines.len() && !is_header(lines[i]) {
                    i += 1;
                }
                let mut pos = 0;
                let mut block = String::new();
                let mut ordinal = 0;
                let mut regs = Vec::new();
                let code = &lines[start + 1..i];
                if code.first().is_some_and(|s| s.starts_with("regs ")) {
                    let line = code[0];
                    let end = line
                        .find(" }")
                        .ok_or_else(|| Error::arg("invalid regs declaration"))?;
                    regs = parameters(&format!("({})", &line[7..end]))?;
                    pos = 1;
                }
                let body = parse_nodes(code, &mut pos, &mut block, &mut ordinal, false)?;
                let f = Function {
                    name: n.into(),
                    kind,
                    signature: sym.signature.clone(),
                    header: lines[start].into(),
                    canonical: lines[start..i].join("\n"),
                    groups,
                    regs,
                    body,
                };
                validate_interface(&f)?;
                if functions.insert(n.into(), f).is_some() {
                    return Err(Error::arg("duplicate function"));
                }
            }
        }
        for s in symbols.values() {
            if s.kind == "data" {
                if data.get(&s.name).map(String::len).map(|n| n / 2)
                    != Some(uint(&s.signature["size"])? as usize)
                {
                    return Err(Error::arg("data symbol size mismatch"));
                }
            } else if !functions.contains_key(&s.name) {
                return Err(Error::arg("symbol has no function definition"));
            }
        }
        let mut stripped = format!("{header}\n");
        for (key, body) in &sections {
            if !["schedule", "certificate", "coverage"].contains(&key.as_str()) {
                stripped.push_str(&format!("\nsection {key}\n{body}\n"));
            }
        }
        let mut object = Self {
            ptr_width,
            versions,
            symbols,
            functions,
            metadata,
            data,
            object_hash: canonical::hash(stripped.as_bytes()),
            interfaces: BTreeMap::new(),
            closures: BTreeMap::new(),
        };
        if !object.metadata.contains_key("layout"){
            for function in object.functions.values(){
                if function.kind=="lproc"||function.signature.get("mem").and_then(Value::as_array).is_some_and(|m|!m.is_empty()){
                    return Err(Error::arg("§7.2: memoryless objects cannot contain lprocs or interface regions"));
                }
                for node in flattened(&function.body){let op=node.op.as_str();if ["lift.addr","lift.boundscheck","lift.alloc","lift.free","lift.call","lift.ret","alloc","free","load","store"].contains(&op)||["lift.load.i","lift.store.i","load.i","store.i"].iter().any(|prefix|op.starts_with(prefix)){
                    return Err(Error::arg("§7.2: memory operation in an object without layout"));
                }}
            }
        }
        let interfaces = object.hashes(false)?;
        for (n, s) in &object.symbols {
            let hash = interfaces
                .get(n)
                .ok_or_else(|| Error::arg("missing computed interface hash"))?
                .clone()?;
            if hash != s.interface_hash {
                return Err(Error::arg(format!(
                    "§9.2: interface hash mismatch for {n}: expected {hash}"
                )));
            }
            object.interfaces.insert(n.clone(), hash);
        }
        object.closures = object.hashes(true)?;
        Ok(object)
    }
    pub fn closure_hash(&self, name: &str) -> Result<&str> {
        self.closures
            .get(name)
            .ok_or_else(|| Error::arg("unknown region"))?
            .as_ref()
            .map(String::as_str)
            .map_err(Clone::clone)
    }
    pub fn region(&self, hash: &str) -> Result<&Function> {
        self.functions
            .values()
            .find(|f| self.closure_hash(&f.name).ok() == Some(hash))
            .ok_or_else(|| Error::arg("closure hash does not name a linked function"))
    }
    pub fn validate_params(&self, name: &str, params: &Value) -> Result<()> {
        let sig = &self
            .symbols
            .get(name)
            .ok_or_else(|| Error::arg("unknown region"))?
            .signature;
        let types = array(&sig["args"])?;
        let args = array(params)?;
        if args.len() != types.len() {
            return Err(Error::arg("§12.3: classical parameter count mismatch"));
        }
        for (t, v) in types.iter().zip(args) {
            validate_value(
                &if t == "ptr" {
                    json!({"ptr":self.ptr_width})
                } else {
                    t.clone()
                },
                v,
            )?;
        }
        Ok(())
    }
    fn refs(&self, n: &str) -> Vec<String> {
        self.functions
            .get(n)
            .map(|f| {
                references(&f.canonical)
                    .into_iter()
                    .filter(|r| {
                        !f.groups
                            .iter()
                            .flatten()
                            .any(|p| p.name == *r && p.ty.starts_with("region["))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn layout_leaves(&self, n: &str) -> Result<Vec<(Vec<u8>, Value)>> {
        let Some(layout) = self.metadata.get("layout") else {
            return Ok(vec![]);
        };
        let kind = &self.symbols[n].kind;
        if ["action", "perm_action", "data"].contains(&kind.as_str()) {
            return Ok(vec![]);
        }
        let refs = self.refs(n);
        let mut leaves = Vec::new();
        for (k, v) in layout
            .as_object()
            .ok_or_else(|| Error::arg("layout must be object"))?
        {
            if k == "globals" {
                for (name, v) in v
                    .as_object()
                    .ok_or_else(|| Error::arg("globals must be object"))?
                {
                    if kind != "lproc" || refs.contains(name) {
                        leaves.push((canonical::to_vec(&json!([k, name]))?, v.clone()));
                    }
                }
            } else if k == "interface_regions" {
                if kind != "lproc" {
                    for (owner, v) in v
                        .as_object()
                        .ok_or_else(|| Error::arg("interface_regions must be object"))?
                    {
                        if kind == "oracle" && owner != n {
                            continue;
                        }
                        for (name, v) in v
                            .as_object()
                            .ok_or_else(|| Error::arg("interface region map must be object"))?
                        {
                            leaves.push((canonical::to_vec(&json!([k, owner, name]))?, v.clone()));
                        }
                    }
                }
            } else if kind != "lproc" {
                leaves.push((canonical::to_vec(&json!([k]))?, v.clone()));
            }
        }
        leaves.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(leaves)
    }
    fn envelope(
        &self,
        n: &str,
        replacements: &BTreeMap<String, String>,
        external: &[String],
    ) -> Result<Vec<u8>> {
        let sym = &self.symbols[n];
        let mut out = format!(
            "isa=0.9\nptr_width={}\n{}\n",
            self.ptr_width,
            String::from_utf8(canonical::to_vec(&sym.signature)?).unwrap()
        );
        if sym.kind == "data" {
            out.push_str(&format!("{} {}\n", n, self.data[n]));
            return Ok(out.into_bytes());
        }
        let f = &self.functions[n];
        let mut lines = f.canonical.lines();
        out.push_str(lines.next().unwrap());
        out.push('\n');
        for line in lines {
            out.push_str(&replace_references(line, replacements));
            out.push('\n');
        }
        let leaves = self.layout_leaves(n)?;
        let mut images = BTreeSet::new();
        for (path, v) in &leaves {
            let keys: Value = canonical::parse(path)?;
            if keys[0] == "globals" {
                images.insert(text(&v["image"])?.to_owned());
            } else if keys[0] == "block_image" {
                images.insert(text(v)?.to_owned());
            }
        }
        for image in images {
            out.push_str(&format!(
                "data {} {}\n",
                image,
                self.data
                    .get(&image)
                    .ok_or_else(|| Error::arg("missing layout data image"))?
            ));
        }
        for (path, v) in leaves {
            out.push_str("layout ");
            out.push_str(std::str::from_utf8(&path).unwrap());
            out.push(' ');
            out.push_str(&String::from_utf8(canonical::to_vec(&v)?).unwrap());
            out.push('\n');
        }
        if ["cc", "r"].contains(&sym.kind.as_str()) {
            if let Some(devices) = self.metadata.get("devices") {
                let mut used = BTreeSet::new();
                for node in flattened(&f.body) {
                    let op = node.op.as_str();
                    if op == "effect" {
                        if let Some(d) = node.operands.first() {
                            let name = d.split('.').next().unwrap();
                            if let Some(v) = devices["models"].get(name) {
                                used.insert((name.to_owned(), canonical::to_vec(v)?));
                            }
                        }
                    } else if op == "hostcall" {
                        if let Some(name) = node.operands.first() {
                            if let Some(v) = devices["services"].get(name) {
                                used.insert((name.clone(), canonical::to_vec(v)?));
                            }
                        }
                    }
                }
                for (name, v) in used {
                    out.push_str(&format!(
                        "device {} {}\n",
                        name,
                        String::from_utf8(v).unwrap()
                    ));
                }
            }
        }
        for hash in external {
            out.push_str(hash);
            out.push('\n');
        }
        Ok(out.into_bytes())
    }
    pub(crate) fn dispatch_component_key(&self,members:&[String])->Result<String>{
        if members.len()==1{return Ok(self.closure_hash(&members[0])?.into());}
        let mut bytes=Vec::new();for name in members{let mut replacements=members.iter().enumerate().map(|(i,n)|(n.clone(),format!("${i}"))).collect::<BTreeMap<_,_>>();let mut external=Vec::new();let mut seen=BTreeSet::new();for reference in self.refs(name){if members.contains(&reference)||!seen.insert(reference.clone()){continue;}let hash=self.closure_hash(&reference)?.to_owned();external.push(hash.clone());replacements.insert(reference,hash);}bytes.extend(self.envelope(name,&replacements,&external)?);}Ok(canonical::hash(&bytes))
    }
    fn hashes(&self, closure: bool) -> Result<BTreeMap<String, Result<String>>> {
        let names: Vec<_> = self.symbols.keys().cloned().collect();
        fn reach(n: &str, o: &Object, seen: &mut BTreeSet<String>) {
            if !seen.insert(n.into()) {
                return;
            }
            for r in o.refs(n) {
                if o.symbols.contains_key(&r) {
                    reach(&r, o, seen);
                }
            }
        }
        let reachability: BTreeMap<_, _> = names
            .iter()
            .map(|n| {
                let mut r = BTreeSet::new();
                reach(n, self, &mut r);
                (n.clone(), r)
            })
            .collect();
        let mut components: Vec<Vec<String>> = Vec::new();
        let mut indices = BTreeMap::new();
        for n in &names {
            if indices.contains_key(n) {
                continue;
            }
            let group: Vec<_> = names
                .iter()
                .filter(|m| reachability[n].contains(*m) && reachability[*m].contains(n))
                .cloned()
                .collect();
            let index = components.len();
            for m in &group {
                indices.insert(m.clone(), index);
            }
            components.push(group);
        }
        fn compute(
            i: usize,
            o: &Object,
            closure: bool,
            components: &[Vec<String>],
            indices: &BTreeMap<String, usize>,
            cache: &mut BTreeMap<String, Result<String>>,
        ) -> Result<()> {
            let group = &components[i];
            if cache.contains_key(&group[0]) {
                return Ok(());
            }
            let cyclic = group.len() > 1 || o.refs(&group[0]).contains(&group[0]);
            let mut all = Vec::new();
            let result = (|| -> Result<Vec<(String, String)>> {
                for n in group {
                    let mut replacements = BTreeMap::new();
                    for (j, m) in group.iter().enumerate() {
                        if cyclic {
                            replacements.insert(m.clone(), format!("${j}"));
                        }
                    }
                    let mut external = Vec::new();
                    let mut seen = BTreeSet::new();
                    for r in o.refs(n) {
                        if group.contains(&r) {
                            if !cyclic {
                                return Err(Error::arg("unexpected self reference"));
                            }
                            continue;
                        }
                        if !seen.insert(r.clone()) {
                            continue;
                        }
                        let hash = if let Some(&j) = indices.get(&r) {
                            compute(j, o, closure, components, indices, cache)?;
                            cache[&r].clone()?
                        } else {
                            let import = o
                                .metadata
                                .get("imports")
                                .and_then(|m| m.get(&r))
                                .ok_or_else(|| Error::arg(format!("unresolved symbol {r}")))?;
                            if closure {
                                return Err(Error::infeasible(format!(
                                    "§9.3: unresolved imported closure {r}"
                                )));
                            }
                            canonical::hash(
                                &[
                                    r.as_bytes(),
                                    b"\0",
                                    &canonical::to_vec(&import["signature"])?,
                                ]
                                .concat(),
                            )
                        };
                        replacements.insert(r, hash.clone());
                        external.push(hash);
                    }
                    let bytes = o.envelope(n, &replacements, &external)?;
                    if cyclic {
                        all.extend(bytes);
                    } else {
                        return Ok(vec![(n.clone(), canonical::hash(&bytes))]);
                    }
                }
                let scc = canonical::hash(&all);
                Ok(group
                    .iter()
                    .enumerate()
                    .map(|(j, n)| (n.clone(), canonical::hash(format!("{scc}:{j}").as_bytes())))
                    .collect())
            })();
            match result {
                Ok(v) => {
                    for (n, h) in v {
                        cache.insert(n, Ok(h));
                    }
                }
                Err(e) => {
                    for n in group {
                        cache.insert(n.clone(), Err(e.clone()));
                    }
                }
            }
            Ok(())
        }
        let mut cache = BTreeMap::new();
        for i in 0..components.len() {
            compute(i, self, closure, &components, &indices, &mut cache)?;
        }
        Ok(cache)
    }
}
fn is_header(line: &str) -> bool {
    [
        "fn cc @",
        "fn r @",
        "oracle @",
        "action @",
        "perm_action @",
        "lproc @",
    ]
    .iter()
    .any(|p| line.starts_with(p))
}
fn parse_nodes(
    lines: &[&str],
    pos: &mut usize,
    block: &mut String,
    ordinal: &mut u64,
    nested: bool,
) -> Result<Vec<Node>> {
    let mut out = Vec::new();
    while *pos < lines.len() {
        let line = lines[*pos];
        if line == "}" || line == "} {" {
            if nested {
                return Ok(out);
            }
            return Err(Error::arg("unmatched source closing brace"));
        }
        if let Some(label) = line.strip_prefix('^') {
            if nested {
                return Err(Error::arg("block label inside structured body"));
            }
            let (n, _) = label
                .split_once('(')
                .ok_or_else(|| Error::arg("invalid block label"))?;
            if !name(n) || !line.ends_with("):") {
                return Err(Error::arg("invalid block label"));
            }
            *block = n.into();
            *ordinal = 0;
            *pos += 1;
            continue;
        }
        if block.is_empty() {
            return Err(Error::arg("instruction outside block"));
        }
        let point = format!("{block}:{}", *ordinal);
        *ordinal += 1;
        *pos += 1;
        let (lhs, rest) = if let Some((l, r)) = line.split_once(" = ") {
            (Some(l.to_owned()), r)
        } else {
            (None, line)
        };
        let structured = rest.ends_with(" {");
        let rest = rest.strip_suffix(" {").unwrap_or(rest);
        let mut w = words(rest)?;
        if w.is_empty() {
            return Err(Error::arg("empty instruction"));
        }
        let op = w.remove(0);
        let mut bodies = Vec::new();
        if structured {
            loop {
                bodies.push(parse_nodes(lines, pos, block, ordinal, true)?);
                let close = *lines
                    .get(*pos)
                    .ok_or_else(|| Error::arg("unclosed structured body"))?;
                *pos += 1;
                if close == "}" {
                    break;
                }
            }
        }
        out.push(Node {
            point,
            line: line.into(),
            lhs,
            op,
            operands: w,
            bodies,
        });
    }
    if nested {
        return Err(Error::arg("unclosed structured body"));
    }
    Ok(out)
}
pub fn flattened(nodes: &[Node]) -> Vec<&Node> {
    let mut out = Vec::new();
    for n in nodes {
        out.push(n);
        for b in &n.bodies {
            out.extend(flattened(b));
        }
    }
    out
}
fn references(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let first_end = s.find('\n').unwrap_or(s.len());
    let body = &s[first_end..];
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'@' {
            let start = i + 1;
            i = start;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b"_.".contains(&b[i])) {
                i += 1;
            }
            if i > start {
                let n = body[start..i].to_owned();
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        } else {
            i += 1;
        }
    }
    out
}
fn replace_references(s: &str, map: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut last = 0;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'@' {
            let start = i + 1;
            i = start;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b"_.".contains(&b[i])) {
                i += 1;
            }
            if let Some(hash) = map.get(&s[start..i]) {
                out.push_str(&s[last..start]);
                out.push_str(hash);
                last = i;
            }
        } else {
            i += 1;
        }
    }
    out.push_str(&s[last..]);
    out
}
fn validate_signature(s: &Symbol) -> Result<()> {
    let v = &s.signature;
    let required: &[&str] = match s.kind.as_str() {
        "oracle" => &["tier", "kind", "abi", "in", "tgt", "fault", "mem", "args"],
        "action" | "perm_action" => &["tier", "kind", "abi", "mut", "in", "mem", "args", "bounds"],
        "lproc" => &[
            "tier", "kind", "abi", "params", "result", "tokens", "args", "mem",
        ],
        "cc" | "r" => &["tier", "kind", "abi", "params", "results"],
        "data" => &["tier", "kind", "abi", "size"],
        _ => return Err(Error::arg("unknown symbol kind")),
    };
    fields(v, required, &[])?;
    Ok(())
}
pub fn wire_width(ty: &str) -> Result<u64> {
    if ty == "wire" {
        return Ok(1);
    }
    let n = ty
        .strip_prefix("wire[")
        .or_else(|| ty.strip_prefix("reg["))
        .and_then(|t| t.strip_suffix(']'))
        .ok_or_else(|| Error::arg("expected wire/reg type"))?;
    let w = uint(&json!(n))?;
    if w == 0 {
        return Err(Error::arg("wire width must be positive"));
    }
    Ok(w)
}
pub fn region_shape(ty: &str) -> Result<(u64, u64)> {
    let inner = ty
        .strip_prefix("region[")
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| Error::arg("expected region[A,W] type"))?;
    let (a, w) = inner
        .split_once(',')
        .ok_or_else(|| Error::arg("region type requires address and cell widths"))?;
    let a = uint(&json!(a))?;
    let w = uint(&json!(w))?;
    if w == 0 {
        return Err(Error::arg("region cell width must be positive"));
    }
    Ok((a, w))
}
fn validate_interface(f: &Function) -> Result<()> {
    let sig = &f.signature;
    let widths: Vec<Vec<Value>> = match f.kind.as_str() {
        "oracle" => vec![
            array(&sig["in"])?.clone(),
            vec![sig["tgt"].clone()],
            vec![sig["fault"].clone()],
        ],
        "action" | "perm_action" => vec![array(&sig["mut"])?.clone(), array(&sig["in"])?.clone()],
        _ => return Ok(()),
    };
    for (i, ws) in widths.iter().enumerate() {
        if f.groups[i].len() != ws.len() {
            return Err(Error::arg("header/signature slot count mismatch"));
        }
        for (p, w) in f.groups[i].iter().zip(ws) {
            if wire_width(&p.ty)? != uint(w)? {
                return Err(Error::arg("header/signature width mismatch"));
            }
            let role = if f.kind == "oracle" {
                if i == 0 { Some("in") } else { None }
            } else if i == 0 {
                Some("mut")
            } else {
                Some("in")
            };
            if p.role.as_deref().is_some_and(|r| Some(r) != role) {
                return Err(Error::arg(
                    "§5.1: interface wire role does not match its header group",
                ));
            }
        }
    }
    let region_group = if f.kind == "oracle" { 3 } else { 2 };
    let regions = array(&sig["mem"])?;
    if regions.len() != f.groups[region_group].len() {
        return Err(Error::arg("header/signature region count mismatch"));
    }
    for (p, r) in f.groups[region_group].iter().zip(regions) {
        fields(r, &["A", "W", "role"], &[])?;
        let (a, w) = region_shape(&p.ty)?;
        let role = text(&r["role"])?;
        if (a, w) != (uint(&r["A"])?, uint(&r["W"])?)
            || p.role.as_deref() != Some(role)
            || !["in", "mut"].contains(&role)
            || (f.kind == "oracle" && role != "in")
        {
            return Err(Error::arg(
                "§3.3/§5.1: header/signature region shape or role mismatch",
            ));
        }
    }
    let args_index = if f.kind == "oracle" { 4 } else { 3 };
    let types = array(&sig["args"])?;
    if f.groups[args_index]
        .iter()
        .map(|p| json!(p.ty))
        .collect::<Vec<_>>()
        != *types
    {
        return Err(Error::arg("header/signature argument types mismatch"));
    }
    Ok(())
}
