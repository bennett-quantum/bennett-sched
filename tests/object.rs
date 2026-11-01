use bennett_sched::{canonical, object::Object};
use serde_json::json;

fn inc8() -> String {
    concat!(
"bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=32\n\nsection symbols\n",
"{\"0\":{\"interface_hash\":\"ed9865300b85a519c2379de98e36e635e72b5b78d8bdfca03dccf9a09aa3df5e\",\"kind\":\"oracle\",\"name\":\"inc8\",\"signature\":{\"abi\":\"0.9\",\"args\":[],\"fault\":4,\"in\":[8],\"kind\":\"oracle\",\"mem\":[],\"tgt\":8,\"tier\":\"q\"}}}\n\nsection functions\n",
"oracle @inc8 (x:wire[8] in) (t:wire[8]) (f:wire[4]) () ()\nregs { one:reg[8], r:reg[8] }\n^entry():\none = lift.const.i8 1\nr = lift.add x one\nlift.retval r\n").into()
}

#[test]
fn normative_inc8_identity_and_positional_arguments() {
    let o = Object::parse(inc8().as_bytes()).unwrap();
    assert_eq!(
        o.closure_hash("inc8").unwrap(),
        "ed9865300b85a519c2379de98e36e635e72b5b78d8bdfca03dccf9a09aa3df5e"
    );
    assert!(o.validate_params("inc8", &json!([])).is_ok());
    assert!(o.validate_params("inc8", &json!(["0"])).is_err());
    assert_eq!(
        o.functions["inc8"]
            .body
            .iter()
            .map(|n| n.point.as_str())
            .collect::<Vec<_>>(),
        ["entry:0", "entry:1", "entry:2"]
    );
}

#[test]
fn published_controlled_action_hash_and_object_metadata_exclusion() {
    let fixture = std::fs::read(
        "../bennett-isa-spec/conformance/fixtures/controlled-action-09/controlled.bisa",
    )
    .unwrap();
    let o = Object::parse(&fixture).unwrap();
    assert_eq!(
        o.closure_hash("controlled").unwrap(),
        "df131e972083b8d3b3b7d9341342b55252ca7a1963c4b7b6f702b9cdf3fbde12"
    );
    assert_eq!(
        o.object_hash,
        "45bbb506541e3a8bb28a18e1bb0aef16ba01ee1e7cac74d5a190deb3ad36ef5a"
    );
    assert_eq!(
        canonical::hash(&canonical::to_vec(&json!([])).unwrap()),
        "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
    );
}

#[test]
fn binding_values_preserve_wide_unsigned_f64_bits_and_token_tuples() {
    let args = json!([
        "18446744073709551617",
        "0x8000000000000000",
        ["2", "0", "0", "0"]
    ]);
    assert_eq!(
        canonical::hash(&canonical::to_vec(&args).unwrap()),
        "9f628a694fa7259bf899bfb677aabce1c084e268a738a40f7f643eb6d8c9c093"
    );
}

#[test]
fn malformed_object_metadata_and_bad_identity_are_rejected() {
    let source = inc8();
    assert!(Object::parse(source.replace("ed986530", "00000000").as_bytes()).is_err());
    assert!(
        Object::parse(
            source
                .replace("\n\nsection functions", "\nsection functions")
                .as_bytes()
        )
        .is_err()
    );
    assert!(Object::parse(source.replace("isa=0.9", "isa=0.8").as_bytes()).is_err());
}
