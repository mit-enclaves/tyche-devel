//! Application Binary Interface
#![allow(unused)]

use crate::arena::{Handle, TypedArena};
use crate::statics::{Statics, NB_DOMAINS, NB_REGIONS_PER_DOMAIN};

// ——————————————————————————————— Hypercalls ——————————————————————————————— //

#[rustfmt::skip]
pub mod vmcalls {
    pub const DOMAIN_GET_OWN_ID: usize    = 0x100;
    pub const DOMAIN_CREATE: usize        = 0x101;
    pub const DOMAIN_REGISTER_GATE: usize = 0x102;
    pub const DOMAIN_SEAL: usize          = 0x103;
    pub const EXIT: usize                 = 0x500;
}

// —————————————————————————————— Error Codes ——————————————————————————————— //

#[repr(usize)]
pub enum ErrorCode {
    Success = 0,
    Failure = 1,
    UnknownVmCall = 2,
    OutOfMemory = 3,
}

// —————————————————————————————————— ABI ——————————————————————————————————— //

pub struct Parameters {
    pub vmcall: usize,
    pub arg_1: usize,
    pub arg_2: usize,
    pub arg_3: usize,
}

pub struct Registers {
    pub value_1: usize,
    pub value_2: usize,
    pub value_3: usize,
}

pub type HypercallResult = Result<Registers, ErrorCode>;

impl Default for Registers {
    fn default() -> Self {
        Self {
            value_1: 0,
            value_2: 0,
            value_3: 0,
        }
    }
}

// ——————————————————————————————— ABI Types ———————————————————————————————— //

pub struct Domain {
    pub sealed: bool,
    pub regions: [RegionCapability; NB_REGIONS_PER_DOMAIN],
}

/// Each region has a single owner and can be marked either as owned or exclusive.
pub struct RegionCapability {
    pub do_own: bool,
    pub is_shared: bool,
    pub is_valid: bool,
    pub index: usize,
}

pub struct Region {
    pub ref_count: usize,
    pub start: usize,
    pub end: usize,
}

// ———————————————————————————————— VM Calls ———————————————————————————————— //

pub struct Hypercalls {
    root_domain: Handle<Domain>,
    current_domain: &'static mut Handle<Domain>,
    domains_arena: TypedArena<Domain>,
}

impl Hypercalls {
    pub fn new(statics: &mut Statics) -> Self {
        let current_domain = statics
            .current_domain
            .take()
            .expect("Missing current_domain_static");
        let domains_arena = statics
            .domains_arena
            .take()
            .expect("Missing domains_arena static");
        let mut domains_arena = TypedArena::new(domains_arena);
        let root_domain = Self::create_root_domain(&mut domains_arena);

        Self {
            root_domain,
            current_domain,
            domains_arena,
        }
    }

    fn create_root_domain(domains_arena: &mut TypedArena<Domain>) -> Handle<Domain> {
        let handle = domains_arena
            .allocate()
            .expect("Failed to allocate root domain");
        let root_domain = &mut domains_arena[handle.clone()];
        root_domain.sealed = true;

        handle
    }

    pub fn dispatch(&mut self, params: Parameters) -> HypercallResult {
        match params.vmcall {
            vmcalls::DOMAIN_GET_OWN_ID => self.domain_get_own_id(),
            vmcalls::DOMAIN_CREATE => self.domain_create(),
            _ => Err(ErrorCode::UnknownVmCall),
        }
    }

    pub fn is_exit(&self, params: &Parameters) -> bool {
        params.vmcall == vmcalls::EXIT
    }
}

impl Hypercalls {
    /// Returns the Domain ID of the current domain.
    fn domain_get_own_id(&mut self) -> HypercallResult {
        let domain = *self.current_domain;
        Ok(Registers {
            value_1: domain.into(),
            ..Default::default()
        })
    }

    /// Creates a fresh domain.
    fn domain_create(&mut self) -> HypercallResult {
        let handle = self
            .domains_arena
            .allocate()
            .ok_or(ErrorCode::OutOfMemory)?;
        let domain = &mut self.domains_arena[handle];
        domain.sealed = false;

        Ok(Registers {
            value_1: handle.into(),
            ..Default::default()
        })
    }
}
