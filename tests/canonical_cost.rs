use bennett_sched::{canonical, decimal};
use num_rational::BigRational;
use serde_json::json;

#[test]
fn jcs_uses_ecmascript_numbers_utf16_order_and_rejects_duplicates() {
    let input = br#"{ "z":1.0,"a":-0,"n":1e-7,"s":"\u000f" }"#;
    assert_eq!(
        canonical::canonicalize(input).unwrap(),
        br#"{"a":0,"n":1e-7,"s":"\u000f","z":1}"#
    );
    assert!(canonical::parse(br#"{"x":0,"x":1}"#).is_err());
    assert!(canonical::require_jcs(input).is_err());
    assert!(canonical::parse(b"\x0b{}").is_err());
    let v = json!({"\u{e000}": 1,"\u{10000}": 2});
    assert_eq!(
        String::from_utf8(canonical::to_vec(&v).unwrap()).unwrap(),
        "{\"𐀀\":2,\"\":1}"
    );
}

#[test]
fn decision_decimals_preserve_all_digits_and_shares_round_down() {
    let q = decimal::parse("0.123456789012345678901234567890123456789").unwrap();
    assert_eq!(
        decimal::exact(&q).unwrap(),
        "0.123456789012345678901234567890123456789"
    );
    let tiny = decimal::parse("1e-200").unwrap();
    let share = decimal::share(&tiny, 3).unwrap();
    assert!(share > decimal::integer(0));
    assert!(share * decimal::integer(3) <= tiny);
}

#[test]
fn published_k12_upward_34_digit_encodings_are_exact() {
    for (n, d, want) in [
        (0, 1, "0"),
        (2, 1, "2"),
        (1, 8, "0.125"),
        (1, 3, "0.3333333333333333333333333333333334"),
        (2, 3, "0.6666666666666666666666666666666667"),
        (1, 30, "0.03333333333333333333333333333333334"),
        (10, 3, "3.333333333333333333333333333333334"),
    ] {
        assert_eq!(
            decimal::upper(&BigRational::new(n.into(), d.into())).unwrap(),
            want
        );
    }
    assert_eq!(
        decimal::parse("0.000999").unwrap(),
        BigRational::new(999.into(), 1000000.into())
    );
    assert_eq!(
        decimal::parse("-0").unwrap(),
        decimal::parse("0.0").unwrap()
    );
    assert!(decimal::parse("NaN").is_err());
}

#[test]
fn published_expansion_catalog_identity_pins_packaging_bytes() {
    // This is an integrity check. Named-interface and expansion tests execute
    // the catalog separately; a digest does not establish semantic coverage.
    let bytes = include_bytes!("../data/ref-0.9.json");
    let v = canonical::require_jcs(bytes).unwrap();
    assert_eq!(v["recipes"].as_array().unwrap().len(), 219);
    assert_eq!(
        canonical::hash(
            &[
                b"isa=0.9\nexpansion=ref\nversion=0.9\n".as_slice(),
                bytes,
                b"\n"
            ]
            .concat()
        ),
        "5de4e4995350313e50217880ba1b95de39cd043283b726337d4894efb005d197"
    );
}

#[test]
fn host_nesting_exhaustion_is_distinct_from_a_malformed_request() {
    let source = format!("{}0{}", "[".repeat(258), "]".repeat(258));
    assert_eq!(
        canonical::parse(source.as_bytes()).unwrap_err().status,
        bennett_sched::Status::NoMem
    );
    let library = bennett_sched::expansion::Library::published().unwrap();
    let mut body = json!([]);
    for _ in 0..258 {
        body = json!([{"inverse":true,"body":body}]);
    }
    let error = library
        .body(
            body.as_array().unwrap(),
            &bennett_sched::expansion::Env::new(),
            &[],
        )
        .unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::NoMem);
}

#[test]
fn certificate_domain_integers_use_numbers_through_the_exact_json_boundary() {
    use num_bigint::BigUint;
    let cases = [
        ("0", json!(0)),
        ("1", json!(1)),
        ("9007199254740991", json!(9007199254740991u64)),
        ("9007199254740992", json!("9007199254740992")),
        ("18446744073709551617", json!("18446744073709551617")),
    ];
    for (decimal, expected) in cases {
        let value = BigUint::parse_bytes(decimal.as_bytes(), 10).unwrap();
        assert_eq!(canonical::unsigned_integer(&value), expected);
    }
}
