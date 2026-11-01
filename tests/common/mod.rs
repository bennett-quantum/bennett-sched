use bennett_sched::canonical;
use serde_json::{Value, json};
pub fn action(kind: &str, name: &str, signature: Value, header: &str, body: &str) -> Vec<u8> {
    let function = format!("{header}\n^entry():\n{body}");
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
