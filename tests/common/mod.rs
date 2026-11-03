use bennett_sched::canonical;
use serde_json::{Value, json};

/// These fixtures declare distinct root token parameters, including when their
/// tuple values agree. Tests for shared identities supply their own partition.
#[allow(dead_code)]
pub fn trace_distinct_tokens(
    object: &bennett_sched::object::Object,
    name: &str,
    inherited: u32,
    params: &Value,
    budget: [u64; 4],
) -> bennett_sched::Result<bennett_sched::source::Trace> {
    let f = object
        .functions
        .get(name)
        .ok_or_else(|| bennett_sched::Error::arg("fixture region missing"))?;
    let group = if f.kind == "oracle" {
        4
    } else if ["action", "perm_action"].contains(&f.kind.as_str()) {
        3
    } else {
        return bennett_sched::source::Trace::build(object, name, inherited, params, budget);
    };
    let count = f.groups[group].iter().filter(|p| p.ty == "token").count();
    if count == 0 {
        return bennett_sched::source::Trace::build(object, name, inherited, params, budget);
    }
    let aliases = (0..(count + usize::from(f.kind == "oracle")) as u64).collect::<Vec<_>>();
    bennett_sched::source::Trace::build_with_token_aliases(
        object, name, inherited, params, budget, &aliases,
    )
}
#[allow(dead_code)]
pub fn schedule_distinct_tokens(
    request: &mut bennett_sched::Request,
) -> bennett_sched::Result<bennett_sched::Response> {
    if request.token_aliases.is_none() {
        if let (Some(object), Some(hash)) = (&request.object, &request.region) {
            let f = object.region(hash)?;
            let group = if f.kind == "oracle" { 4 } else { 3 };
            let count = f.groups[group].iter().filter(|p| p.ty == "token").count();
            if count > 0 {
                let aliases =
                    (0..(count + usize::from(f.kind == "oracle")) as u64).collect::<Vec<_>>();
                request.set_token_aliases(&bennett_sched::canonical::to_vec(&json!(aliases))?)?;
            }
        }
    }
    request.schedule()
}
pub fn action(kind: &str, name: &str, signature: Value, header: &str, body: &str) -> Vec<u8> {
    let bounds = signature
        .get("bounds")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .map(|a| {
            format!(
                "regs {{  }} bounds {{{}}}\n",
                a.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .unwrap_or_default();
    let function = format!("{header}\n{bounds}^entry():\n{body}");
    let envelope = format!(
        "isa=0.9\nptr_width=32\n{}\n{}\n",
        String::from_utf8(canonical::to_vec(&signature).unwrap()).unwrap(),
        function
    );
    let hash = canonical::hash(envelope.as_bytes());
    let symbols =
        json!({"0":{"kind":kind,"name":name,"signature":signature,"interface_hash":hash}});
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=32\n\nsection symbols\n{}\n\nsection functions\n{function}\n",String::from_utf8(canonical::to_vec(&symbols).unwrap()).unwrap()).into_bytes()
}

#[allow(dead_code)]
pub fn caller(
    base: &[u8],
    kind: &str,
    name: &str,
    signature: Value,
    header: &str,
    body: &str,
) -> Vec<u8> {
    let object = bennett_sched::object::Object::parse(base).unwrap();
    let ptr_width = object.ptr_width;
    let bounds = signature["bounds"]
        .as_array()
        .filter(|a| !a.is_empty())
        .map(|a| {
            format!(
                "regs {{  }} bounds {{{}}}\n",
                a.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .unwrap_or_default();
    let function = format!("{header}\n{bounds}^entry():\n{body}");
    let mut refs = Vec::new();
    let mut substituted = body.to_owned();
    for token in body.split_whitespace() {
        if let Some(callee) = token.strip_prefix('@') {
            if !refs.iter().any(|(n, _)| n == callee) {
                let hash = object.closure_hash(callee).unwrap().to_owned();
                refs.push((callee.to_owned(), hash));
            }
        }
    }
    for (callee, hash) in &refs {
        substituted = substituted
            .lines()
            .map(|line| {
                line.split(' ')
                    .map(|t| {
                        if t == format!("@{callee}") {
                            format!("@{hash}")
                        } else {
                            t.to_owned()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    let suffix = refs
        .iter()
        .map(|(_, h)| format!("{h}\n"))
        .collect::<String>();
    let envelope = format!(
        "isa=0.9\nptr_width={ptr_width}\n{}\n{header}\n{bounds}^entry():\n{substituted}\n{suffix}",
        String::from_utf8(canonical::to_vec(&signature).unwrap()).unwrap()
    );
    let hash = canonical::hash(envelope.as_bytes());
    let mut symbols = object.metadata["symbols"].clone();
    let index = symbols.as_object().unwrap().len().to_string();
    symbols[&index] = json!({"name":name,"kind":kind,"signature":signature,"interface_hash":hash});
    let old = object
        .functions
        .values()
        .map(|f| f.canonical.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let layout = object
        .metadata
        .get("layout")
        .map(|v| {
            format!(
                "\nsection layout\n{}\n",
                String::from_utf8(canonical::to_vec(v).unwrap()).unwrap()
            )
        })
        .unwrap_or_default();
    let data = if object.data.is_empty() {
        String::new()
    } else {
        format!(
            "\nsection data\n{}\n",
            object
                .data
                .iter()
                .map(|(name, hex)| format!("{name} {hex}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width={ptr_width}\n\nsection symbols\n{}\n\nsection functions\n{}\n{function}\n{data}{layout}",String::from_utf8(canonical::to_vec(&symbols).unwrap()).unwrap(),old.trim_end_matches('\n')).into_bytes()
}

/// Acyclic test sources with scalar layout fields and no globals/mappings.
#[allow(dead_code)]
pub fn layout_sources(definitions: &[(&str, &str, Value, &str)], layout: Value) -> Vec<u8> {
    assert!(layout["globals"].as_object().unwrap().is_empty());
    assert!(layout["interface_regions"].as_object().unwrap().is_empty());
    let ptr = layout["ptr_width"].as_u64().unwrap();
    let jcs = |v: &Value| String::from_utf8(canonical::to_vec(v).unwrap()).unwrap();
    let mut leaves = layout
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| !["globals", "interface_regions"].contains(&k.as_str()))
        .map(|(k, v)| (jcs(&json!([k])), jcs(v)))
        .collect::<Vec<_>>();
    leaves.sort();
    let layout_suffix = leaves
        .iter()
        .map(|(p, v)| format!("layout {p} {v}\n"))
        .collect::<String>();
    let mut hashes = std::collections::BTreeMap::<String, String>::new();
    let mut symbols = json!({});
    for (index, (name, kind, signature, function)) in definitions.iter().enumerate() {
        let mut bound = function.to_string();
        let mut refs = Vec::new();
        for token in function.split_whitespace() {
            if let Some(reference) = token.strip_prefix('@').filter(|n| n != name) {
                let hash = hashes
                    .get(reference)
                    .expect("test source definitions must precede their callers");
                if !refs.contains(hash) {
                    refs.push(hash.clone());
                    bound = bound.replace(&format!("@{reference} "), &format!("@{hash} "));
                }
            }
        }
        let refs = refs.iter().map(|h| format!("{h}\n")).collect::<String>();
        let hash = canonical::hash(
            format!(
                "isa=0.9\nptr_width={ptr}\n{}\n{bound}\n{}{refs}",
                jcs(signature),
                if *kind == "oracle" {
                    layout_suffix.as_str()
                } else {
                    ""
                }
            )
            .as_bytes(),
        );
        symbols[index.to_string()] =
            json!({"name":name,"kind":kind,"signature":signature,"interface_hash":hash});
        hashes.insert(name.to_string(), hash);
    }
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width={ptr}\n\nsection symbols\n{}\n\nsection functions\n{}\n\nsection layout\n{}\n",jcs(&symbols),definitions.iter().map(|(_,_,_,f)|*f).collect::<Vec<_>>().join("\n"),jcs(&layout)).into_bytes()
}
