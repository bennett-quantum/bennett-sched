//! Owned Rust request/response model behind the §11 binary interface.
use crate::{
    Error, Result, Status, canonical,
    decimal::{self as d, Rational},
    object::Object,
    target::Target,
};
use num_traits::Signed;
use serde_json::Value;
use std::{collections::BTreeMap, ffi::CString, sync::Arc};
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub object: Option<Arc<Object>>,
    pub target: Option<Arc<Target>>,
    pub region: Option<String>,
    pub variant: Option<String>,
    pub inherited_controls: u32,
    pub budget: Option<[u64; 4]>,
    pub params: Option<Value>,
    pub epsilon: Option<Rational>,
    pub capacities: BTreeMap<String, u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Outcome {
    Ok = 0,
    Infeasible = 1,
    Unsupported = 2,
}
#[derive(Clone, Debug)]
pub struct Response {
    pub outcome: Outcome,
    pub reason: CString,
    pub schedule: Vec<u8>,
    pub certificate: Vec<u8>,
}
impl Response {
    pub fn failure(outcome: Outcome, reason: &str) -> Self {
        Self {
            outcome,
            reason: CString::new(reason.replace('\0', "\\0")).expect("NUL sanitized"),
            schedule: vec![],
            certificate: vec![],
        }
    }
}
impl Request {
    pub fn set_object(&mut self, bytes: &[u8]) -> Result<()> {
        let object = Object::parse(bytes)?;
        self.object = Some(Arc::new(object));
        Ok(())
    }
    pub fn set_target(&mut self, bytes: &[u8]) -> Result<()> {
        let target = Target::parse(bytes)?;
        self.target = Some(Arc::new(target));
        Ok(())
    }
    pub fn set_region(&mut self, hash: &str, variant: &str, inherited: u32) -> Result<()> {
        if !canonical::is_hash(hash) || variant.is_empty() || variant.contains('\0') {
            return Err(Error::arg("§11: invalid closure hash or empty variant"));
        }
        self.region = Some(hash.into());
        self.variant = Some(variant.into());
        self.inherited_controls = inherited;
        Ok(())
    }
    pub fn set_params(&mut self, bytes: &[u8]) -> Result<()> {
        let v = canonical::require_jcs(bytes)?;
        if !v.is_array() {
            return Err(Error::arg("§12.3: params must be a JCS args array"));
        }
        self.params = Some(v);
        Ok(())
    }
    pub fn set_accuracy(&mut self, s: &str) -> Result<()> {
        let eps = d::parse(s)?;
        if eps.is_negative() {
            return Err(Error::arg("§11: accuracy must be nonnegative"));
        }
        self.epsilon = Some(eps);
        Ok(())
    }
    pub fn set_capacity(&mut self, class: &str, capacity: u64) -> Result<()> {
        if class.is_empty() || class == "any" || class.contains('\0') {
            return Err(Error::arg("§11: capacity class must be concrete"));
        }
        self.capacities.insert(class.into(), capacity);
        Ok(())
    }
    pub fn schedule(&self) -> Result<Response> {
        let result = crate::engine::run(self);
        match result {
            Err(e) if e.status == Status::ScheduleInfeasible => {
                Ok(Response::failure(Outcome::Infeasible, &e.reason))
            }
            Err(e) if e.status == Status::GateUnsupported => {
                Ok(Response::failure(Outcome::Unsupported, &e.reason))
            }
            r => r,
        }
    }
}
