//! Scheduler and cost model for Bennett ISA 0.9, §11.
pub mod admission;
pub mod borrow;
pub mod canonical;
pub mod cleanup;
pub mod decimal;
pub mod engine;
pub mod expansion;
mod expansion_env;
pub mod ffi;
pub mod layout;
pub mod lproc;
pub mod object;
pub mod oracle;
mod oracle_activation;
mod oracle_calls;
mod oracle_memory;
mod oracle_pairs;
pub mod oracle_state;
mod qinplace;
pub mod ref_contract;
pub mod request;
pub mod resources;
pub mod schedule;
pub mod source;
pub mod source_ir;
pub mod source_registry;
pub mod target;
pub use request::{Outcome, Request, Response};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Status {
    Ok = 0,
    Version = 1,
    PtrWidth = 2,
    DeviceMissing = 3,
    GateUnsupported = 4,
    Target = 5,
    Budget = 6,
    QregNotClean = 7,
    InvalidState = 8,
    NotReversible = 9,
    ScheduleInfeasible = 10,
    Arg = 11,
    NoMem = 12,
}
#[derive(Clone, Debug)]
pub struct Error {
    pub status: Status,
    pub reason: String,
}
impl Error {
    pub fn new(status: Status, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
        }
    }
    pub fn arg(reason: impl Into<String>) -> Self {
        Self::new(Status::Arg, reason)
    }
    pub fn infeasible(reason: impl Into<String>) -> Self {
        Self::new(Status::ScheduleInfeasible, reason)
    }
    pub fn target(reason: impl Into<String>) -> Self {
        Self::new(Status::Target, reason)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}
impl std::error::Error for Error {}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::arg(format!("JSON: {e}"))
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// Minor component of sched 0.9 (compatibility probe, outside the normative ABI).
#[unsafe(no_mangle)]
pub extern "C" fn bennett_sched_version() -> u32 {
    9
}
