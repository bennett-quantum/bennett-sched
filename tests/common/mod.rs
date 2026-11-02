use bennett_sched::canonical;
use serde_json::{Value, json};
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
        "isa=0.9\nptr_width=32\n{}\n{header}\n{bounds}^entry():\n{substituted}\n{suffix}",
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
    format!("bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=32\n\nsection symbols\n{}\n\nsection functions\n{}\n{function}\n{data}{layout}",String::from_utf8(canonical::to_vec(&symbols).unwrap()).unwrap(),old.trim_end_matches('\n')).into_bytes()
}
