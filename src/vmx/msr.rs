//! VMX Model Specific Registers
//!
//! A collection of some model specific registers relevant to VMX.

pub use x86_64::registers::model_specific::Msr;

pub const FEATURE_CONTROL: Msr = Msr::new(0x3A);
pub const SYSENTER_CS: Msr = Msr::new(0x174);
pub const SYSENTER_ESP: Msr = Msr::new(0x175);
pub const SYSENTER_EIP: Msr = Msr::new(0x176);
pub const VMX_BASIC: Msr = Msr::new(0x480);
pub const VMX_PINBASED_CTLS: Msr = Msr::new(0x481);
pub const VMX_PROCBASED_CTL: Msr = Msr::new(0x482);
pub const VMX_EXIT_CTLS: Msr = Msr::new(0x483);
pub const VMX_ENTRY_CTLS: Msr = Msr::new(0x484);
pub const VMX_TRUE_PINBASED_CTLS: Msr = Msr::new(0x48D);
pub const VMX_TRUE_PROCBASED_CTLS: Msr = Msr::new(0x48E);
pub const VMX_TRUE_EXIT_CTLS: Msr = Msr::new(0x48F);
pub const VMX_TRUE_ENTRY_CTLS: Msr = Msr::new(0x490);
