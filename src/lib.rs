//! bennett-sched: scheduler and cost model (sched 0.3).
//!
//! Implements the scheduling interface of Bennett-ISA-Spec.md section 11.

/// Version of the scheduling interface implemented (sched 0.3, ISA spec section 11).
/// Returned as the minor version number of the major.minor pair "0.3".
#[unsafe(no_mangle)]
pub extern "C" fn bennett_sched_version() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_three() {
        assert_eq!(super::bennett_sched_version(), 3);
    }
}
