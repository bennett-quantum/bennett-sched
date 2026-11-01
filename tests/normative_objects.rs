use bennett_sched::object::Object;
#[test]
fn every_printed_09_example_object_has_its_published_interface_hash() {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    let examples = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("## 17.")
        .next()
        .unwrap();
    let mut count = 0;
    for block in examples.split("```").skip(1).step_by(2) {
        let block = block.trim_start_matches('\n');
        if block.starts_with("bennett-object ") {
            count += 1;
            Object::parse(block.as_bytes())
                .unwrap_or_else(|e| panic!("normative object #{count}: {e}"));
        }
    }
    assert_eq!(count, 10, "contract example inventory changed");
}
