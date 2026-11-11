#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{Outcome, Status, ffi::*};
use std::{
    ffi::{CStr, CString},
    ptr,
};

#[test]
fn feature_query_is_immutable_concurrent_and_rejects_nulls_without_writes() {
    const MANIFEST: &[u8] =
        br#"{"abi":"sched/0.9","features":["token_aliases/1"],"schema":"bennett.abi-features/1"}"#;
    let query = || unsafe {
        let mut bytes = ptr::null();
        let mut len = 0;
        assert_eq!(bennett_sched_features(&mut bytes, &mut len), Status::Ok);
        assert!(!bytes.is_null());
        assert_eq!(std::slice::from_raw_parts(bytes, len), MANIFEST);
        bennett_sched::canonical::require_jcs(std::slice::from_raw_parts(bytes, len)).unwrap();
        (bytes as usize, len)
    };
    let original = query();
    unsafe {
        let mut bytes = original.0 as *const u8;
        let mut len = usize::MAX;
        assert_eq!(
            bennett_sched_features(ptr::null_mut(), &mut len),
            Status::Arg
        );
        assert_eq!(len, usize::MAX);
        assert_eq!(
            bennett_sched_features(&mut bytes, ptr::null_mut()),
            Status::Arg
        );
        assert_eq!(bytes as usize, original.0);
        assert_eq!(
            bennett_sched_features(ptr::null_mut(), ptr::null_mut()),
            Status::Arg
        );
        let mut request = ptr::null_mut();
        assert_eq!(bennett_sched_request_new(&mut request), Status::Ok);
        assert_eq!(
            bennett_sched_request_set_token_aliases(request, b"[0,1]".as_ptr(), 5),
            Status::Ok
        );
        assert_eq!(query(), original);
        assert_eq!((*request).token_aliases, Some(vec![0, 1]));
        bennett_sched_request_free(request);
    }
    let queries: Vec<_> = (0..8).map(|_| std::thread::spawn(query)).collect();
    for result in queries {
        assert_eq!(result.join().unwrap(), original);
    }
    assert_eq!(query(), original);
}

#[test]
fn binary_api_rejects_nulls_and_preserves_owned_buffers() {
    unsafe {
        assert_eq!(bennett_sched_request_new(ptr::null_mut()), Status::Arg);
        let mut request = ptr::null_mut();
        assert_eq!(bennett_sched_request_new(&mut request), Status::Ok);
        assert_eq!(
            bennett_sched_request_set_object(request, ptr::null(), 1),
            Status::Arg
        );
        let mut object = std::fs::read(pins::spec(
            "conformance/fixtures/controlled-action-09/controlled.bisa",
        ))
        .unwrap();
        assert_eq!(
            bennett_sched_request_set_object(request, object.as_ptr(), object.len()),
            Status::Ok
        );
        object.fill(0);
        let mut target = std::fs::read(pins::spec(
            "conformance/fixtures/controlled-action-09/target.json",
        ))
        .unwrap();
        assert_eq!(
            bennett_sched_request_set_target(request, target.as_ptr(), target.len()),
            Status::Ok
        );
        target.fill(0);
        let hash = CString::new("df131e972083b8d3b3b7d9341342b55252ca7a1963c4b7b6f702b9cdf3fbde12")
            .unwrap();
        let variant = CString::new("forward").unwrap();
        assert_eq!(
            bennett_sched_request_set_region(request, hash.as_ptr(), variant.as_ptr(), 0),
            Status::Ok
        );
        assert_eq!(
            bennett_sched_request_set_budget(request, 0, 0, 0, 0),
            Status::Ok
        );
        assert_eq!(
            bennett_sched_request_set_params(request, b"[]".as_ptr(), 2),
            Status::Ok
        );
        assert_eq!(
            bennett_sched_request_set_accuracy(request, c"0".as_ptr()),
            Status::Ok
        );
        let mut response = ptr::null_mut();
        assert_eq!(bennett_sched_schedule(request, &mut response), Status::Ok);
        bennett_sched_request_free(request);
        assert_eq!(bennett_sched_response_outcome(response), Outcome::Ok);
        assert_eq!(
            CStr::from_ptr(bennett_sched_response_reason(response))
                .to_str()
                .unwrap(),
            "OK"
        );
        let mut len = usize::MAX;
        let bytes = bennett_sched_response_schedule(response, &mut len);
        assert!(!bytes.is_null());
        assert!(len > 0);
        let original = std::slice::from_raw_parts(bytes, len).to_vec();
        let mut certlen = 0;
        assert!(!bennett_sched_response_certificate(response, &mut certlen).is_null());
        assert!(certlen > 0);
        assert_eq!(std::slice::from_raw_parts(bytes, len), original);
        bennett_sched_response_free(response);
        bennett_sched_response_free(ptr::null_mut());
        bennett_sched_request_free(ptr::null_mut());
    }
}

#[test]
fn failing_schedule_clears_output_before_returning_status() {
    unsafe {
        let mut r = ptr::null_mut();
        bennett_sched_request_new(&mut r);
        let mut out = ptr::dangling_mut();
        assert_eq!(bennett_sched_schedule(r, &mut out), Status::Arg);
        assert!(out.is_null());
        bennett_sched_request_free(r);
    }
}

#[test]
fn alias_setter_checks_pointers_and_copies_input_atomically() {
    unsafe {
        assert_eq!(
            bennett_sched_request_set_token_aliases(ptr::null_mut(), b"[]".as_ptr(), 2),
            Status::Arg
        );
        let mut request = ptr::null_mut();
        assert_eq!(bennett_sched_request_new(&mut request), Status::Ok);
        let mut aliases = b"[0,1,0]".to_vec();
        assert_eq!(
            bennett_sched_request_set_token_aliases(request, aliases.as_ptr(), aliases.len()),
            Status::Ok
        );
        aliases.fill(0);
        for (data, len) in [
            (ptr::null(), 1),
            (ptr::null(), 0),
            (b"[]".as_ptr(), usize::MAX),
            (b"[1]".as_ptr(), 3),
        ] {
            assert_eq!(
                bennett_sched_request_set_token_aliases(request, data, len),
                Status::Arg
            );
            assert_eq!((*request).token_aliases, Some(vec![0, 1, 0]));
        }
        bennett_sched_request_free(request);
    }
}
