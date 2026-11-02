use bennett_sched::{Status, canonical, object::Object};
use serde_json::{Value, json};

fn example(index: usize) -> String {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    spec.split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("## 17.")
        .next()
        .unwrap()
        .split("```")
        .skip(1)
        .step_by(2)
        .map(|s| s.trim_start_matches('\n'))
        .filter(|s| s.starts_with("bennett-object "))
        .nth(index - 1)
        .unwrap()
        .to_owned()
}
fn changed(index: usize, change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let source = example(index);
    let old = source
        .split("section layout\n")
        .nth(1)
        .unwrap()
        .lines()
        .next()
        .unwrap();
    let mut layout: Value = serde_json::from_str(old).unwrap();
    change(&mut layout);
    source
        .replace(
            old,
            &String::from_utf8(canonical::to_vec(&layout).unwrap()).unwrap(),
        )
        .into_bytes()
}
fn rejected(index: usize, change: impl FnOnce(&mut Value), reason: &str) {
    let error = Object::parse(&changed(index, change)).expect_err(reason);
    assert_eq!(error.status, Status::Arg, "{error}");
    assert!(error.reason.contains(reason), "{error}, expected {reason}");
}

#[test]
fn layout_loader_checks_schema_and_mathematical_ranges_before_scheduling() {
    rejected(2, |v| v["surprise"] = json!(0), "unknown field");
    rejected(
        2,
        |v| v["globals"]["zero"]["extra"] = json!(0),
        "unknown field",
    );
    rejected(2, |v| v["ptr_width"] = json!(64), "ptr_width");
    rejected(2, |v| v["H"] = json!(33), "H");
    rejected(2, |v| v["null_size"] = json!(0), "null_size");
    rejected(2, |v| v["heap_base"] = json!("4294967295"), "address space");
    rejected(2, |v| v["globals"]["zero"]["base"] = json!(0), "overlap");
    rejected(2, |v| v["globals"]["zero"]["base"] = json!(4112), "overlap");
    rejected(2, |v| v["heap_base"] = json!(4113), "lifted address space");
    rejected(2, |v| v["A_tab"] = json!(0), "pre-entries");
}

#[test]
fn globals_are_exact_data_images_and_mapped_regions_use_exact_owner_and_parameter_names() {
    rejected(
        2,
        |v| v["globals"]["zero"]["image"] = json!("other"),
        "image",
    );
    rejected(2, |v| v["globals"]["zero"]["size"] = json!(2), "size");
    rejected(
        2,
        |v| v["interface_regions"]["sum_db"]["db"]["base"] = json!(16),
        "overlap",
    );
    rejected(
        2,
        |v| v["interface_regions"]["sum_db"] = json!({}),
        "mapping",
    );
    rejected(2, |v| v["interface_regions"]["sum"] = json!({}), "oracle");
    rejected(
        2,
        |v| v["interface_regions"]["sum_db"]["other"] = json!({"base":30}),
        "mapping",
    );
    // The two oracles may independently map their inputs to overlapping addresses.
    let mut object = Object::parse(example(10).as_bytes()).unwrap();
    object.metadata.get_mut("layout").unwrap()["interface_regions"]["plus_db"]["db"]["base"] =
        json!(16);
    assert!(bennett_sched::layout::Layout::from_object(&object).is_ok());
}

#[test]
fn zero_extent_globals_do_not_overlap_and_block_images_are_complete_sectors() {
    let source = example(2);
    let object = Object::parse(source.as_bytes()).unwrap();
    let old_signature = object.symbols["zero"].signature.clone();
    let mut signature = old_signature.clone();
    signature["size"] = json!(0);
    // Use the public metadata validator below on an otherwise parsed object;
    // interface hashes are independently exercised by normative_objects.
    let mut object = object;
    object.symbols.get_mut("zero").unwrap().signature = signature;
    object.data.insert("zero".into(), String::new());
    let v = object.metadata.get_mut("layout").unwrap();
    v["globals"]["zero"]["size"] = json!(0);
    v["globals"]["zero"]["base"] = json!(4112);
    assert!(bennett_sched::layout::Layout::from_object(&object).is_ok());
    object.metadata.get_mut("layout").unwrap()["block_image"] = json!("zero");
    assert!(bennett_sched::layout::Layout::from_object(&object).is_ok());
    rejected(2, |v| v["block_image"] = json!("zero"), "512");
}

#[test]
fn unsigned_endpoints_are_checked_without_machine_wrap_or_power_allocation() {
    let mut object = Object::parse(example(2).as_bytes()).unwrap();
    object.ptr_width = 64;
    let v = object.metadata.get_mut("layout").unwrap();
    v["ptr_width"] = json!(64);
    v["H"] = json!(64);
    v["heap_base"] = json!("18446744073709547536");
    // The exclusive endpoint 2^64 is valid; 2^64+1 is not.
    assert!(bennett_sched::layout::Layout::from_object(&object).is_ok());
    object.metadata.get_mut("layout").unwrap()["heap_limit_max"] = json!(4081);
    assert!(bennett_sched::layout::Layout::from_object(&object).is_err());
    object.ptr_width = u64::MAX;
    let v = object.metadata.get_mut("layout").unwrap();
    v["ptr_width"] = json!(u64::MAX.to_string());
    v["H"] = json!(u64::MAX.to_string());
    assert!(bennett_sched::layout::Layout::from_object(&object).is_ok());
}

#[test]
fn enormous_symbolic_region_extents_report_host_exhaustion_before_materializing_them() {
    let mut object = Object::parse(example(10).as_bytes()).unwrap();
    object.ptr_width = 100_000_001;
    let layout = object.metadata.get_mut("layout").unwrap();
    layout["ptr_width"] = json!(100_000_001);
    layout["H"] = json!(100_000_001);
    layout["heap_base"] = json!(0);
    layout["heap_limit_max"] = json!(0);
    object.functions.get_mut("through").unwrap().groups[3][0].ty = "region[100000000,8]".into();
    let error = match bennett_sched::layout::Layout::from_object(&object) {
        Err(error) => error,
        Ok(_) => panic!("large region extent must exhaust the bounded host descriptor workspace"),
    };
    assert_eq!(error.status, Status::NoMem);
}
