//! §11 C ABI. Callers own handles; accessor pointers live until response_free.
//!
//! # Safety
//! Every non-null pointer must refer to the declared type and a live allocation.
//! Byte buffers must contain `len` readable bytes, C strings must terminate, output
//! pointers must be writable, and a handle must not be freed or mutated concurrently.
#![allow(clippy::missing_safety_doc)]
use crate::{Error, Outcome, Request, Response, Result, Status};
use std::{
    ffi::{CStr, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice,
};
fn status(f: impl FnOnce() -> Result<()>) -> Status {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => Status::Ok,
        Ok(Err(e)) => e.status,
        Err(_) => Status::InvalidState,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_guard_preserves_errors_and_contains_panics_at_the_abi_boundary() {
        assert_eq!(status(|| Ok(())), Status::Ok);
        assert_eq!(status(|| Err(Error::arg("invalid request"))), Status::Arg);
        assert_eq!(
            status(|| panic!("foreign boundary panic")),
            Status::InvalidState
        );
    }
}
unsafe fn request<'a>(r: *mut Request) -> Result<&'a mut Request> {
    unsafe { r.as_mut() }.ok_or_else(|| Error::arg("NULL request"))
}
unsafe fn bytes<'a>(data: *const u8, len: usize) -> Result<&'a [u8]> {
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() || len > isize::MAX as usize {
        return Err(Error::arg("invalid byte buffer"));
    }
    Ok(unsafe { slice::from_raw_parts(data, len) })
}
unsafe fn string<'a>(s: *const c_char) -> Result<&'a str> {
    if s.is_null() {
        return Err(Error::arg("NULL string"));
    }
    unsafe { CStr::from_ptr(s) }
        .to_str()
        .map_err(|_| Error::arg("C string must be UTF-8"))
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_new(out: *mut *mut Request) -> Status {
    status(|| {
        if out.is_null() {
            return Err(Error::arg("NULL request output"));
        }
        unsafe {
            *out = ptr::null_mut();
            *out = Box::into_raw(Box::new(Request::default()));
        }
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_object(
    r: *mut Request,
    data: *const u8,
    len: usize,
) -> Status {
    status(|| unsafe { request(r)?.set_object(bytes(data, len)?) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_target(
    r: *mut Request,
    data: *const u8,
    len: usize,
) -> Status {
    status(|| unsafe { request(r)?.set_target(bytes(data, len)?) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_region(
    r: *mut Request,
    hash: *const c_char,
    variant: *const c_char,
    controls: u32,
) -> Status {
    status(|| unsafe { request(r)?.set_region(string(hash)?, string(variant)?, controls) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_budget(
    r: *mut Request,
    steps: u64,
    heap: u64,
    stack: u64,
    depth: u64,
) -> Status {
    status(|| {
        unsafe { request(r)? }.budget = Some([steps, heap, stack, depth]);
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_params(
    r: *mut Request,
    data: *const u8,
    len: usize,
) -> Status {
    status(|| unsafe { request(r)?.set_params(bytes(data, len)?) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_accuracy(
    r: *mut Request,
    epsilon: *const c_char,
) -> Status {
    status(|| unsafe { request(r)?.set_accuracy(string(epsilon)?) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_token_aliases(
    r: *mut Request,
    data: *const u8,
    len: usize,
) -> Status {
    status(|| unsafe { request(r)?.set_token_aliases(bytes(data, len)?) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_set_capacity(
    r: *mut Request,
    class: *const c_char,
    peak: u64,
) -> Status {
    status(|| unsafe { request(r)?.set_capacity(string(class)?, peak) })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_request_free(r: *mut Request) {
    if !r.is_null() {
        drop(unsafe { Box::from_raw(r) });
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_schedule(
    r: *const Request,
    out: *mut *mut Response,
) -> Status {
    status(|| {
        if out.is_null() {
            return Err(Error::arg("NULL response output"));
        }
        unsafe {
            *out = ptr::null_mut();
        }
        let r = unsafe { r.as_ref() }.ok_or_else(|| Error::arg("NULL request"))?;
        let response = r.schedule()?;
        unsafe {
            *out = Box::into_raw(Box::new(response));
        }
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_response_outcome(r: *const Response) -> Outcome {
    unsafe { r.as_ref() }
        .map(|r| r.outcome)
        .unwrap_or(Outcome::Unsupported)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_response_reason(r: *const Response) -> *const c_char {
    unsafe { r.as_ref() }
        .map(|r| r.reason.as_ptr())
        .unwrap_or(c"invalid NULL response handle".as_ptr())
}
unsafe fn buffer(r: *const Response, len: *mut usize, certificate: bool) -> *const u8 {
    if len.is_null() {
        return ptr::null();
    }
    unsafe {
        *len = 0;
    }
    let Some(r) = (unsafe { r.as_ref() }) else {
        return ptr::null();
    };
    let bytes = if certificate {
        &r.certificate
    } else {
        &r.schedule
    };
    unsafe {
        *len = bytes.len();
    }
    if bytes.is_empty() {
        ptr::null()
    } else {
        bytes.as_ptr()
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_response_schedule(
    r: *const Response,
    len: *mut usize,
) -> *const u8 {
    unsafe { buffer(r, len, false) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_response_certificate(
    r: *const Response,
    len: *mut usize,
) -> *const u8 {
    unsafe { buffer(r, len, true) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bennett_sched_response_free(r: *mut Response) {
    if !r.is_null() {
        drop(unsafe { Box::from_raw(r) });
    }
}
