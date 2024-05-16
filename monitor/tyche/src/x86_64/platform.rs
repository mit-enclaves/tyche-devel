//! Platform specific configuration

use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

use capa_engine::config::{NB_CORES, NB_DOMAINS, NB_REMAP_REGIONS};
use capa_engine::context::{RegisterContext, RegisterGroup, RegisterState};
use capa_engine::utils::BitmapIterator;
use capa_engine::{
    AccessRights, CapaEngine, CapaError, Domain, GenArena, Handle, LocalCapa, MemOps, Remapper,
};
use mmu::eptmapper::EPT_ROOT_FLAGS;
use mmu::{EptMapper, FrameAllocator, IoPtFlag, IoPtMapper};
use spin::{Mutex, MutexGuard};
use stage_two_abi::{GuestInfo, Manifest};
use utils::{GuestPhysAddr, HostPhysAddr, HostVirtAddr};
use vmx::bitmaps::{exit_qualification, EptEntryFlags, ExceptionBitmap};
use vmx::errors::Trapnr;
use vmx::fields::VmcsField;
use vmx::{ActiveVmcs, VmxExitReason};
use vtd::Iommu;

use super::cpuid_filter::{filter_mpk, filter_tpause};
use super::guest::VmxState;
use super::init::NB_BOOTED_CORES;
use super::vmx_helper::{dump_host_state, load_host_state};
use super::{cpuid, vmx_helper};
use crate::allocator::{self, allocator};
use crate::arch::guest::HandlerResult;
use crate::monitor::{CoreUpdate, Monitor, PlatformState};
use crate::rcframe::{drop_rc, RCFrame, RCFramePool, EMPTY_RCFRAME};
use crate::sync::Barrier;

#[cfg(not(feature = "bare_metal"))]
pub fn remap_core(core: usize) -> usize {
    core
}

#[cfg(not(feature = "bare_metal"))]
pub fn remap_core_bitmap(bitmap: u64) -> u64 {
    bitmap
}

#[cfg(feature = "bare_metal")]
pub fn remap_core(core: usize) -> usize {
    // Our harware has hyper-threads, and renames all co-located threads
    if core < 8 {
        core * 2
    } else {
        (core - 8) * 2 + 1
    }
}

#[cfg(feature = "bare_metal")]
pub fn remap_core_bitmap(bitmap: u64) -> u64 {
    let mut new_bitmap = 0;
    for idx in 0..16 {
        if bitmap & (1 << idx) != 0 {
            new_bitmap |= 1 << remap_core(idx);
        }
    }

    new_bitmap
}

/// The constants for the register context.
const NB_16: usize = 20;
const NB_32: usize = 40;
const NB_64: usize = 49;
const NB_NAT: usize = 30;
const NB_GP: usize = 16;

/// Static values
static DOMAINS: [Mutex<DataX86>; NB_DOMAINS] = [EMPTY_DOMAIN; NB_DOMAINS];
static RC_VMCS: Mutex<RCFramePool> =
    Mutex::new(GenArena::new([EMPTY_RCFRAME; { NB_DOMAINS * NB_CORES }]));
static CONTEXTS: [[Mutex<ContextX86>; NB_CORES]; NB_DOMAINS] = [EMPTY_CONTEXT_ARRAY; NB_DOMAINS];
static IOMMU: Mutex<Iommu> =
    Mutex::new(unsafe { Iommu::new(HostVirtAddr::new(usize::max_value())) });
const FALSE: AtomicBool = AtomicBool::new(false);
static TLB_FLUSH_BARRIERS: [Barrier; NB_DOMAINS] = [Barrier::NEW; NB_DOMAINS];
static TLB_FLUSH: [AtomicBool; NB_DOMAINS] = [FALSE; NB_DOMAINS];

// —————————————————————————————— Empty values —————————————————————————————— //
const EMPTY_CONTEXT_ARRAY: [Mutex<ContextX86>; NB_CORES] = [EMPTY_CONTEXT; NB_CORES];
const EMPTY_CONTEXT: Mutex<ContextX86> = Mutex::new(ContextX86 {
    registers: RegistersX86 {
        dirty: capa_engine::context::Cache { bitmap: 0 },
        state_16: RegisterState::new(),
        state_32: RegisterState::new(),
        state_64: RegisterState::new(),
        state_nat: RegisterState::new(),
        state_gp: RegisterState::new(),
    },
    interrupted: false,
    vmcs: Handle::<RCFrame>::new_invalid(),
});
const EMPTY_DOMAIN: Mutex<DataX86> = Mutex::new(DataX86 {
    ept: None,
    ept_old: None,
    iopt: None,
    remapper: Remapper::new(),
});

type RegistersX86 = RegisterContext<NB_16, NB_32, NB_64, NB_NAT, NB_GP>;

pub struct ContextX86 {
    pub registers: RegistersX86,
    pub interrupted: bool,
    pub vmcs: Handle<RCFrame>,
}

impl ContextX86 {
    pub const DUMP_FRAME: [(VmcsField, VmcsField); 9] = [
        (VmcsField::GuestRbx, VmcsField::GuestRip),
        (VmcsField::GuestRcx, VmcsField::GuestRsp),
        (VmcsField::GuestRdx, VmcsField::GuestRflags),
        (VmcsField::GuestRsi, VmcsField::VmInstructionError),
        (VmcsField::GuestR8, VmcsField::VmExitReason),
        (VmcsField::GuestR9, VmcsField::VmExitIntrInfo),
        (VmcsField::GuestR10, VmcsField::VmExitIntrErrorCode),
        (VmcsField::GuestR11, VmcsField::VmExitInstructionLen),
        (VmcsField::GuestR12, VmcsField::VmInstructionError),
    ];
    fn copy_interrupt_frame(&mut self, src: &Self) -> Result<(), CapaError> {
        for i in Self::DUMP_FRAME {
            if let (Some((dest_group, dest_idx)), Some((src_group, src_idx))) =
                (translate_x86field(i.0), translate_x86field(i.1))
            {
                let value = src.registers.get(src_group, src_idx)?;
                self.registers.set(dest_group, dest_idx, value)?;
            }
        }
        Ok(())
    }

    pub fn set(
        &mut self,
        field: VmcsField,
        value: usize,
        vcpu: Option<&mut ActiveVmcs>,
    ) -> Result<(), CapaError> {
        let (group, idx) = translate_x86field(field).ok_or(CapaError::InvalidValue)?;
        self.registers.set(group, idx, value)?;
        if field.is_gp_register() {
            return Ok(());
        }
        if let Some(vcpu) = vcpu {
            vcpu.set(field, value).or(Err(CapaError::InvalidValue))?;
        }
        Ok(())
    }

    pub fn get(&mut self, field: VmcsField, vcpu: Option<&ActiveVmcs>) -> Result<usize, CapaError> {
        let (group, idx) = translate_x86field(field).ok_or(CapaError::InvalidValue)?;
        if vcpu.is_some() && !field.is_gp_register() {
            let value = vcpu.unwrap().get(field).or(Err(CapaError::InvalidValue))?;
            self.registers.set(group, idx, value)?;
        }
        Ok(self.registers.get(group, idx)?)
    }

    pub fn switch_flush(&mut self, rc_vmcs: &Mutex<RCFramePool>, vcpu: &mut ActiveVmcs) {
        let locked = rc_vmcs.lock();
        let rc_frame = locked.get(self.vmcs).unwrap();
        // Switch the frame.
        vcpu.switch_frame(rc_frame.frame).unwrap();
        // Load values that changed.
        self.flush(vcpu);
    }

    fn flush(&mut self, vcpu: &mut ActiveVmcs) {
        let update = |r: RegisterGroup, idx: usize, value: usize| {
            let field = translate_to_x86field(r, idx).unwrap();
            // Avoid the gp registers
            if field.is_gp_register() {
                return;
            }
            vcpu.set(field, value).unwrap();
        };
        self.registers.flush(update);
    }

    /// Read the vcpu, write context.
    fn load(&mut self, vcpu: &mut ActiveVmcs) {
        let mut get_set = |g: RegisterGroup, d: &[VmcsField]| {
            for i in 0..d.len() {
                let value = vcpu.get(d[i]).unwrap();
                self.registers.set(g, i, value).unwrap();
            }
        };
        // The 16-bits registers.
        get_set(RegisterGroup::Reg16, &X86_FIELDS16);
        // The 32-bits registers.
        get_set(RegisterGroup::Reg32, &X86_FIELDS32);
        // The 64-bits registers.
        get_set(RegisterGroup::Reg64, &X86_FIELDS64);
        // The Nat-bits registers.
        get_set(RegisterGroup::RegNat, &X86_FIELDSNAT);
    }
}

/// Domain data on x86
pub struct DataX86 {
    ept: Option<HostPhysAddr>,
    ept_old: Option<HostPhysAddr>,
    iopt: Option<HostPhysAddr>,
    remapper: Remapper<NB_REMAP_REGIONS>,
}

pub type StateX86 = VmxState;

impl StateX86 {
    unsafe fn free_ept(ept: HostPhysAddr, allocator: &impl FrameAllocator) {
        let mapper = EptMapper::new(allocator.get_physical_offset().as_usize(), ept);
        mapper.free_all(allocator);
    }

    unsafe fn free_iopt(iopt: HostPhysAddr, allocator: &impl FrameAllocator) {
        let mapper = IoPtMapper::new(allocator.get_physical_offset().as_usize(), iopt);
        mapper.free_all(allocator);
    }

    fn update_domain_iopt(
        domain_handle: Handle<Domain>,
        engine: &mut MutexGuard<CapaEngine>,
    ) -> bool {
        let mut domain = Self::get_domain(domain_handle);
        let allocator = allocator();
        if let Some(iopt) = domain.iopt {
            unsafe { Self::free_iopt(iopt, allocator) };
            // TODO: global invalidate context cache, PASID cache, and flush the IOTLB
        }

        let iopt_root = allocator
            .allocate_frame()
            .expect("Failed to allocate I/O PT root")
            .zeroed();
        let mut iopt_mapper = IoPtMapper::new(
            allocator.get_physical_offset().as_usize(),
            iopt_root.phys_addr,
        );

        // Traverse all regions of the I/O domain and maps them into the new iopt
        for range in engine.get_domain_permissions(domain_handle).unwrap() {
            if !range.ops.contains(MemOps::READ) {
                log::error!("there is a region without read permission: {}", range);
                continue;
            }
            let gpa = range.start;
            iopt_mapper.map_range(
                allocator,
                GuestPhysAddr::new(gpa),
                HostPhysAddr::new(range.start),
                range.size(),
                IoPtFlag::READ | IoPtFlag::WRITE | IoPtFlag::EXECUTE,
            )
        }

        domain.iopt = Some(iopt_root.phys_addr);

        // Update the IOMMU
        // TODO: @yuchen ideally we only need to change the 2nd stage page translation pointer on the
        //               context table, instead of reallocating the whole root table
        // Remap the DMA region on IOMMU
        let mut iommu = IOMMU.lock();
        if iommu.get_addr() as usize != 0 {
            let root_addr: HostPhysAddr =
                vtd::setup_iommu_context(iopt_mapper.get_root(), allocator);
            iommu.set_root_table_addr(root_addr.as_u64() | (0b00 << 10)); // Set legacy mode
            iommu.update_root_table_addr();
            iommu.enable_translation();
            log::info!("I/O MMU: {:?}", iommu.get_global_status());
            log::warn!("I/O MMU Fault: {:?}", iommu.get_fault_status());
        }

        false
    }

    fn update_domain_ept(
        domain_handle: Handle<Domain>,
        engine: &mut MutexGuard<CapaEngine>,
    ) -> bool {
        let mut domain = Self::get_domain(domain_handle);
        let allocator = allocator();
        if domain.ept_old.is_some() {
            panic!("We will replace an ept old that's not empty");
        }
        let ept_root = allocator
            .allocate_frame()
            .expect("Failled to allocate EPT root")
            .zeroed();
        let mut mapper = EptMapper::new(
            allocator.get_physical_offset().as_usize(),
            ept_root.phys_addr,
        );
        let permission_iter = engine.get_domain_permissions(domain_handle).unwrap();
        for range in domain.remapper.remap(permission_iter) {
            if !range.ops.contains(MemOps::READ) {
                log::error!("there is a region without read permission: {}", range);
                continue;
            }
            let mut flags = EptEntryFlags::READ;
            if range.ops.contains(MemOps::WRITE) {
                flags |= EptEntryFlags::WRITE;
            }
            if range.ops.contains(MemOps::EXEC) {
                if range.ops.contains(MemOps::SUPER) {
                    flags |= EptEntryFlags::SUPERVISOR_EXECUTE;
                } else {
                    flags |= EptEntryFlags::USER_EXECUTE;
                }
            }
            mapper.map_range(
                allocator,
                GuestPhysAddr::new(range.gpa),
                HostPhysAddr::new(range.hpa),
                range.size,
                flags,
            );
        }

        loop {
            match TLB_FLUSH[domain_handle.idx()].compare_exchange(
                false,
                true,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(false) => break,
                _ => continue,
            }
        }

        // The core needs exclusive access before updating the domain's EPT. Otherwise, we might have
        // miss freeing some EPT roots.
        // The contexts per core will be updated in the permission change update.
        domain.ept_old = domain.ept;
        domain.ept = Some(ept_root.phys_addr);

        true
    }

    fn switch_domain(
        vcpu: &mut ActiveVmcs<'static>,
        current_ctx: &mut MutexGuard<ContextX86>,
        next_ctx: &mut MutexGuard<ContextX86>,
        next_domain: MutexGuard<DataX86>,
        return_capa: LocalCapa,
    ) -> Result<(), CapaError> {
        // Safety check that both contexts have a valid vmcs.
        if current_ctx.vmcs.is_invalid() || next_ctx.vmcs.is_invalid() {
            log::error!(
                "VMCS are none during switch: curr:{:?}, next:{:?}",
                current_ctx.vmcs.is_invalid(),
                next_ctx.vmcs.is_invalid()
            );
            return Err(CapaError::InvalidSwitch);
        }

        // We have different cases:
        // 1. current(interrupted) -- interrupt --> next.
        // 2. current -- resume interrupted --> next(interrupted)
        // 3. current -- synchronous --> next
        if current_ctx.interrupted && next_ctx.interrupted {
            panic!("Two domains should never be both interrupted in a switch.");
        }
        // Case 1: copy the interrupted state.
        if current_ctx.interrupted {
            next_ctx.copy_interrupt_frame(current_ctx).unwrap();
            // Set the return values.
            next_ctx.set(VmcsField::GuestRax, 0, None)?;
            next_ctx.set(VmcsField::GuestRdi, return_capa.as_usize(), None)?;
        } else if next_ctx.interrupted {
            // Case 2: do not put the return capa.
            next_ctx.interrupted = false;
        } else {
            // Case 3: synchronous call.
            next_ctx.set(VmcsField::GuestRax, 0, None).unwrap();
            next_ctx
                .set(VmcsField::GuestRdi, return_capa.as_usize(), None)
                .unwrap();
        }

        // Now the logic for shared vs. private vmcs.
        if current_ctx.vmcs == next_ctx.vmcs {
            panic!("Why are the two vmcs the same?");
        }
        current_ctx.load(vcpu);

        // NOTE; it seems on hardware we need to save and restore the host context, but we don't know
        // why yet, we need further invesdigation to be able to optimise this.
        let mut values: [usize; 13] = [0; 13];
        dump_host_state(vcpu, &mut values).expect("Couldn't save host context");

        // Configure state of the next TD
        next_ctx.switch_flush(&RC_VMCS, vcpu);
        vcpu.set_ept_ptr(HostPhysAddr::new(
            next_domain.ept.unwrap().as_usize() | EPT_ROOT_FLAGS,
        ))
        .expect("Failed to update EPT");
        load_host_state(vcpu, &mut values).expect("Couldn't save host context");
        Ok(())
    }
}

impl PlatformState for StateX86 {
    type DomainData = DataX86;
    type Context = ContextX86;

    fn find_buff(
        engine: &MutexGuard<CapaEngine>,
        domain_handle: Handle<Domain>,
        addr: usize,
        end: usize,
    ) -> Option<usize> {
        let domain = Self::get_domain(domain_handle);
        let permission_iter = engine.get_domain_permissions(domain_handle).unwrap();
        for range in domain.remapper.remap(permission_iter) {
            let range_start = range.gpa;
            let range_end = range_start + range.size;
            if range_start <= addr
                && addr < range_end
                && range_start < end
                && end <= range_end
                && range.ops.contains(MemOps::WRITE)
            {
                // We found a valid region that encapsulate the buffer!
                // On x86_64 it is possible that we use some relocations, so compute the physical
                // address of the buffer.
                let gpa_to_hpa_offset = (range.gpa as isize) - (range.hpa as isize);
                let start = (addr as isize) - gpa_to_hpa_offset;
                return Some(start as usize);
            }
        }
        return None;
    }

    fn platform_init_io_mmu(&self, addr: usize) {
        let mut iommu = IOMMU.lock();
        iommu.set_addr(addr);
    }

    fn get_domain(domain: Handle<Domain>) -> MutexGuard<'static, Self::DomainData> {
        DOMAINS[domain.idx()].lock()
    }

    fn get_context(domain: Handle<Domain>, core: usize) -> MutexGuard<'static, Self::Context> {
        CONTEXTS[domain.idx()][core].lock()
    }

    fn remap_core(core: usize) -> usize {
        return remap_core(core);
    }

    fn remap_core_bitmap(bitmap: u64) -> u64 {
        return remap_core_bitmap(bitmap);
    }

    fn max_cpus() -> usize {
        NB_BOOTED_CORES.load(core::sync::atomic::Ordering::SeqCst) + 1
    }

    fn create_context(
        &mut self,
        _engine: MutexGuard<CapaEngine>,
        current: Handle<Domain>,
        domain: Handle<Domain>,
        core: usize,
    ) -> Result<(), CapaError> {
        let allocator = allocator();
        let mut rcvmcs = RC_VMCS.lock();
        let dest = &mut Self::get_context(domain, core);
        let frame = allocator.allocate_frame().unwrap();
        let rc = RCFrame::new(frame);
        drop_rc(&mut *rcvmcs, dest.vmcs);
        dest.vmcs = rcvmcs.allocate(rc).expect("Unable to allocate rc frame");
        // Init the frame it needs the identifier.
        self.vmxon.init_frame(frame);
        // Init the host state.
        {
            let current_ctxt = Self::get_context(current, cpuid());
            let mut values: [usize; 13] = [0; 13];
            dump_host_state(&mut self.vcpu, &mut values).or(Err(CapaError::InvalidValue))?;
            // Switch to the target frame.
            self.vcpu
                .switch_frame(rcvmcs.get(dest.vmcs).unwrap().frame)
                .unwrap();
            // Init to the default values.
            let info: GuestInfo = Default::default();
            vmx_helper::default_vmcs_config(&mut self.vcpu, &info, false);

            // Load the default values.
            load_host_state(&mut self.vcpu, &mut values).or(Err(CapaError::InvalidValue))?;

            // Switch back the frame.
            self.vcpu
                .switch_frame(rcvmcs.get(current_ctxt.vmcs).unwrap().frame)
                .unwrap();
        }
        return Ok(());
    }

    fn update_permission(domain: Handle<Domain>, engine: &mut MutexGuard<CapaEngine>) -> bool {
        if engine[domain].is_io() {
            Self::update_domain_iopt(domain, engine)
        } else {
            Self::update_domain_ept(domain, engine)
        }
    }

    fn create_domain(domain: Handle<Domain>) {
        let mut domain = Self::get_domain(domain);
        let allocator = allocator();
        if let Some(ept) = domain.ept {
            unsafe { Self::free_ept(ept, allocator) }
        }
        let ept_root = allocator
            .allocate_frame()
            .expect("Failed to allocate EPT root")
            .zeroed();
        domain.ept = Some(ept_root.phys_addr);
    }

    fn revoke_domain(_domain: Handle<Domain>) {
        // Noop for now, might need to send IPIs once we land multi-core
    }

    fn apply_core_update(
        &mut self,
        current_domain: &mut Handle<Domain>,
        core: usize,
        update: &CoreUpdate,
    ) {
        let vcpu = &mut self.vcpu;
        log::trace!("Core Update: {} on core {}", update, core);
        match update {
            CoreUpdate::TlbShootdown => {
                // Into a separate function so that we can drop the domain lock before starting to
                // wait on the TLB_FLUSH_BARRIER
                self.platform_shootdown(current_domain, core, false);
                log::trace!("core {} waits on tlb flush barrier", core);
                TLB_FLUSH_BARRIERS[current_domain.idx()].wait();
                log::trace!("core {} done waiting", core);
            }
            CoreUpdate::Switch {
                domain,
                return_capa,
            } => {
                log::trace!("Domain Switch on core {}", core);

                let mut current_ctx = Self::get_context(*current_domain, core);
                let mut next_ctx = Self::get_context(*domain, core);
                let next_domain = Self::get_domain(*domain);
                Self::switch_domain(
                    vcpu,
                    &mut current_ctx,
                    &mut next_ctx,
                    next_domain,
                    *return_capa,
                )
                .expect("Failed to perform the switch");
                // Update the current domain and context handle
                *current_domain = *domain;
            }
            CoreUpdate::Trap {
                manager: _manager,
                trap,
                info: _info,
            } => {
                log::trace!("Trap {} on core {}", trap, core);
                log::debug!(
                    "Exception Bitmap is {:b}",
                    vcpu.get_exception_bitmap().expect("Failed to read bitmpap")
                );
                todo!("Update this code path.");
            }
        }
    }

    fn platform_shootdown(&mut self, domain: &Handle<Domain>, core: usize, trigger: bool) {
        let dom = Self::get_domain(*domain);
        let new_epts = dom.ept.unwrap().as_usize() | EPT_ROOT_FLAGS;
        let mut context = Self::get_context(*domain, core);
        // We triggered the update.
        if trigger {
            context.set(VmcsField::EptPointer, new_epts, None).unwrap();
        } else {
            context
                .set(VmcsField::EptPointer, new_epts, Some(&mut self.vcpu))
                .unwrap();
        }
    }

    fn set_core(
        &mut self,
        engine: &mut MutexGuard<CapaEngine>,
        domain: &Handle<Domain>,
        core: usize,
        idx: usize,
        value: usize,
    ) -> Result<(), CapaError> {
        let mut ctxt = Self::get_context(*domain, core);
        let field = VmcsField::from_u32(idx as u32).ok_or(CapaError::InvalidValue)?;
        let (group, idx) = translate_x86field(field).ok_or(CapaError::InvalidValue)?;
        // Check the permissions.
        let (_, perm_write) = group.to_permissions();
        let bitmap = engine.get_domain_permission(*domain, perm_write);
        // Not allowed.
        if engine.is_domain_sealed(*domain) && ((1 << idx) & bitmap == 0) {
            return Err(CapaError::InsufficientPermissions);
        }
        ctxt.set(field, value, None)
    }

    fn get_core(
        &mut self,
        engine: &mut MutexGuard<CapaEngine>,
        domain: &Handle<Domain>,
        core: usize,
        idx: usize,
    ) -> Result<usize, CapaError> {
        let mut ctxt = Self::get_context(*domain, core);
        let field = VmcsField::from_u32(idx as u32).ok_or(CapaError::InvalidValue)?;
        let (group, idx) = translate_x86field(field).ok_or(CapaError::InvalidValue)?;
        // Check the permissions.
        let (perm_read, _) = group.to_permissions();
        let bitmap = engine.get_domain_permission(*domain, perm_read);
        // Not allowed.
        if engine.is_domain_sealed(*domain) && ((1 << idx) & bitmap == 0) {
            return Err(CapaError::InsufficientPermissions);
        }
        ctxt.get(field, None)
    }

    fn check_overlaps(
        &mut self,
        _engine: &mut MutexGuard<CapaEngine>,
        domain: Handle<Domain>,
        alias: usize,
        repeat: usize,
        region: &AccessRights,
    ) -> bool {
        let dom_dat = Self::get_domain(domain);
        dom_dat
            .remapper
            .overlaps(alias, repeat * (region.end - region.start))
    }

    fn map_region(
        &mut self,
        engine: &mut MutexGuard<CapaEngine>,
        domain: Handle<Domain>,
        alias: usize,
        repeat: usize,
        region: &AccessRights,
    ) -> Result<(), CapaError> {
        let mut dom_dat = Self::get_domain(domain);
        let _ = dom_dat
            .remapper
            .map_range(region.start, alias, region.end - region.start, repeat)
            .unwrap(); // Overlap is checked again but should not be triggered.
        engine.conditional_permission_update(domain);
        Ok(())
    }

    fn unmap_region(
        &mut self,
        _engine: &mut MutexGuard<CapaEngine>,
        domain: Handle<Domain>,
        alias: usize,
        size: usize,
    ) -> Result<(), CapaError> {
        let mut data = Self::get_domain(domain);
        let _ = data.remapper.unmap_gpa_range(alias, size).unwrap();
        Ok(())
    }

    fn prepare_notify(&mut self, domain: &Handle<Domain>, core_count: usize) {
        TLB_FLUSH_BARRIERS[domain.idx()].set_count(core_count);
    }

    fn notify_cores(&mut self, _domain: &Handle<Domain>, core_id: usize, core_map: usize) {
        for core in BitmapIterator::new(core_map as u64) {
            if core == core_id {
                continue;
            }
            x2apic::send_init_assert(core as u32);
        }
    }

    fn acknowledge_notify(&mut self, domain: &Handle<Domain>) {
        TLB_FLUSH_BARRIERS[domain.idx()].wait();
    }

    fn finish_notify(&mut self, domain: &Handle<Domain>) {
        let mut dom = Self::get_domain(*domain);
        let allocator = allocator();
        if let Some(ept) = dom.ept_old {
            unsafe { Self::free_ept(ept, allocator) };
        }
        dom.ept_old = None;
        TLB_FLUSH[domain.idx()].store(false, Ordering::SeqCst);
    }

    fn context_interrupted(&mut self, domain: &Handle<Domain>, core: usize) {
        let mut context = Self::get_context(*domain, core);
        context.interrupted = true;
    }
}

// ————————————————————— Monitor Implementation on X86 —————————————————————— //

pub struct MonitorX86 {}

impl Monitor<StateX86> for MonitorX86 {}

impl MonitorX86 {
    pub fn init(manifest: &'static Manifest, bsp: bool) -> (StateX86, Handle<Domain>) {
        let allocator = allocator::allocator();
        let vmxon_frame = allocator
            .allocate_frame()
            .expect("Failed to allocate VMXON frame")
            .zeroed();
        let vmxon = unsafe { vmx::vmxon(vmxon_frame).expect("Failed to execute VMXON") };
        let vmcs_frame = allocator
            .allocate_frame()
            .expect("Failed to allocate VMCS frame")
            .zeroed();
        let vmcs = unsafe {
            vmxon
                .create_vm_unsafe(vmcs_frame)
                .expect("Failed to create VMCS")
        };
        let vcpu = vmcs.set_as_active().expect("Failed to set VMCS as active");
        let mut state = VmxState { vcpu, vmxon };
        let domain = if bsp {
            Self::do_init(&mut state, manifest)
        } else {
            Self::get_initial_domain()
        };
        let dom = StateX86::get_domain(domain);
        let mut ctx = StateX86::get_context(domain, cpuid());
        let rcframe = RC_VMCS
            .lock()
            .allocate(RCFrame::new(*state.vcpu.frame()))
            .expect("Unable to allocate rcframe");
        ctx.vmcs = rcframe;
        state
            .vcpu
            .set_ept_ptr(HostPhysAddr::new(
                dom.ept.unwrap().as_usize() | EPT_ROOT_FLAGS,
            ))
            .expect("Failed to set initial EPT ptr");
        unsafe {
            vmx_helper::init_vcpu2(&mut state.vcpu, &manifest.info, &mut ctx);
        }
        (state, domain)
    }

    pub fn launch_guest(
        &mut self,
        manifest: &'static Manifest,
        state: StateX86,
        domain: Handle<Domain>,
    ) {
        if !manifest.info.loaded {
            log::warn!("No guest found, exiting");
            return;
        }
        log::info!("Staring main loop");
        self.main_loop(state, domain);
        qemu::exit(qemu::ExitCode::Success);
    }

    pub fn main_loop(&mut self, mut state: StateX86, mut domain: Handle<Domain>) {
        let core_id = cpuid();
        let mut result = unsafe {
            let mut context = StateX86::get_context(domain, core_id);
            state.vcpu.run(&mut context.registers.state_gp.values)
        };
        loop {
            let exit_reason = match result {
                Ok(exit_reason) => {
                    let res = self
                        .handle_exit(&mut state, exit_reason, &mut domain)
                        .expect("Failed to handle VM exit");

                    // Apply core-local updates before returning
                    Self::apply_core_updates(&mut state, &mut domain, core_id);

                    res
                }
                Err(err) => {
                    log::error!("Guest crash: {:?}", err);
                    log::error!("Domain: {:?}", domain);
                    log::error!("Vcpu: {:x?}", state.vcpu);
                    HandlerResult::Crash
                }
            };

            match exit_reason {
                HandlerResult::Resume => {
                    result = unsafe {
                        let mut context = StateX86::get_context(domain, core_id);
                        context.flush(&mut state.vcpu);
                        state.vcpu.run(&mut context.registers.state_gp.values)
                    };
                }
                _ => {
                    log::info!("Exiting guest: {:?}", exit_reason);
                    break;
                }
            }
        }
    }

    pub fn handle_exit(
        &mut self,
        vs: &mut StateX86,
        reason: VmxExitReason,
        domain: &mut Handle<Domain>,
    ) -> Result<HandlerResult, CapaError> {
        match reason {
            VmxExitReason::Vmcall => {
                let (vmcall, arg_1, arg_2, arg_3, arg_4, arg_5, arg_6) = {
                    let mut context = StateX86::get_context(*domain, cpuid());
                    let vmcall = context.get(VmcsField::GuestRax, None)?;
                    let arg_1 = context.get(VmcsField::GuestRdi, None)?;
                    let arg_2 = context.get(VmcsField::GuestRsi, None)?;
                    let arg_3 = context.get(VmcsField::GuestRdx, None)?;
                    let arg_4 = context.get(VmcsField::GuestRcx, None)?;
                    let arg_5 = context.get(VmcsField::GuestR8, None)?;
                    let arg_6 = context.get(VmcsField::GuestR9, None)?;
                    (vmcall, arg_1, arg_2, arg_3, arg_4, arg_5, arg_6)
                };
                let args: [usize; 6] = [arg_1, arg_2, arg_3, arg_4, arg_5, arg_6];
                let mut res: [usize; 6] = [0; 6];
                Self::do_monitor_call(vs, domain, vmcall, &args, &mut res)?;
                vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
                Ok(HandlerResult::Resume)
            }
        VmxExitReason::InitSignal /*if domain.idx() == 0*/ => {
            log::trace!("cpu {} received init signal", cpuid());
            Ok(HandlerResult::Resume)
        }
        VmxExitReason::Cpuid if domain.idx() == 0 => {
            let mut context = StateX86::get_context(*domain, cpuid());
            let input_eax = context.get(VmcsField::GuestRax, None)?;
            let input_ecx = context.get(VmcsField::GuestRcx, None)?;
            let mut eax: usize;
            let mut ebx: usize;
            let mut ecx: usize;
            let mut edx: usize;

            unsafe {
                // Note: LLVM reserves %rbx for its internal use, so we need to use a scratch
                // register for %rbx here.
                asm!(
                    "mov {tmp}, rbx",
                    "cpuid",
                    "mov rsi, rbx",
                    "mov rbx, {tmp}",
                    tmp = out(reg) _,
                    inout("rax") input_eax => eax,
                    inout("rcx") input_ecx => ecx,
                    out("rdx") edx,
                    out("rsi") ebx
                )
            }

            //Apply cpuid filters.
            filter_tpause(input_eax, input_ecx, &mut eax, &mut ebx, &mut ecx, &mut edx);
            filter_mpk(input_eax, input_ecx, &mut eax, &mut ebx, &mut ecx, &mut edx);

            context.set(VmcsField::GuestRax, eax as usize, None)?;
            context.set(VmcsField::GuestRbx, ebx as usize, None)?;
            context.set(VmcsField::GuestRcx, ecx as usize, None)?;
            context.set(VmcsField::GuestRdx, edx as usize, None)?;
            vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
            Ok(HandlerResult::Resume)
        }
        VmxExitReason::ControlRegisterAccesses if domain.idx() == 0 => {
            // Handle some of these only for dom0, the other domain's problems
            // are for now forwarded to the manager domain.
            let mut context = StateX86::get_context(*domain, cpuid());
            let qualification = vs.vcpu.exit_qualification().or(Err(CapaError::PlatformError))?.control_register_accesses();
            match qualification {
                exit_qualification::ControlRegisterAccesses::MovToCr(cr, reg) => {
                    log::info!("MovToCr {:?} into {:?} on domain {:?}", reg, cr, *domain);
                    if !cr.is_guest_cr() {
                        log::error!("Invalid register: {:x?}", cr);
                        panic!("VmExit reason for access to control register is not a control register.");
                    }
                    if cr == VmcsField::GuestCr4 {
                        let value = context.get(reg, Some(&mut vs.vcpu))? as usize;
                        context.set(VmcsField::Cr4ReadShadow, value, Some(&mut vs.vcpu))?;
                        let real_value = value | (1 << 13); // VMXE
                        context.set(cr, real_value, Some(&mut vs.vcpu))?;
                    } else {
                        todo!("Handle cr: {:?}", cr);
                    }

                    vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
                }
                _ => todo!("Emulation not yet implemented for {:?}", qualification),
            };
            Ok(HandlerResult::Resume)
        }
        VmxExitReason::EptViolation if domain.idx() == 0 => {
            let addr = vs.vcpu.guest_phys_addr().or(Err(CapaError::PlatformError))?;
            log::error!(
                "EPT Violation on dom0! virt: 0x{:x}, phys: 0x{:x}",
                vs.vcpu
                    .guest_linear_addr()
                    .expect("unable to get the virt addr")
                    .as_u64(),
                addr.as_u64(),
            );
            panic!("The vcpu {:x?}", vs.vcpu);
        }
        VmxExitReason::Exception if domain.idx() == 0 => {
            panic!("Received an exception on dom0?");
        }
        VmxExitReason::Xsetbv if domain.idx() == 0 => {
            let mut context = StateX86::get_context(*domain, cpuid());
            let ecx = context.get(VmcsField::GuestRcx, None)?;
            let eax = context.get(VmcsField::GuestRax, None)?;
            let edx = context.get(VmcsField::GuestRdx, None)?;

            let xrc_id = ecx & 0xFFFFFFFF; // Ignore 32 high-order bits
            if xrc_id != 0 {
                log::error!("Xsetbv: invalid rcx 0x{:x}", ecx);
                return Ok(HandlerResult::Crash);
            }

            unsafe {
                asm!(
                    "xsetbv",
                    in("ecx") ecx,
                    in("eax") eax,
                    in("edx") edx,
                );
            }

            vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
            Ok(HandlerResult::Resume)
        }
        VmxExitReason::Wrmsr if domain.idx() == 0 => {
            let mut context = StateX86::get_context(*domain, cpuid());
            let ecx = context.get(VmcsField::GuestRcx, None)?;
            if ecx >= 0x4B564D00 && ecx <= 0x4B564DFF {
                // Custom MSR range, used by KVM
                // See https://docs.kernel.org/virt/kvm/x86/msr.html
                // TODO: just ignore them for now, should add support in the future
                vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
                Ok(HandlerResult::Resume)
            } else {
                log::error!("Unknown MSR: 0x{:x}", ecx);
                Ok(HandlerResult::Crash)
            }
        }
        VmxExitReason::Rdmsr if domain.idx() == 0 => {
            let mut context = StateX86::get_context(*domain, cpuid());
            let ecx = context.get(VmcsField::GuestRcx, None)?;
            log::trace!("rdmsr 0x{:x}", ecx);
            if ecx >= 0xc0010000 && ecx <= 0xc0020000 {
                // Reading an AMD specific register, just ignore it
                // The other interval seems to be related to pmu...
                // TODO: figure this out and why it only works on certain hardware.
                vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
                log::trace!("rdmsr ignoring amd registers");
                Ok(HandlerResult::Resume)
            } else {
                let msr_reg = vmx::msr::Msr::new(ecx as u32);
                log::trace!("rdmsr: about to read");
                let (low, high) = unsafe { msr_reg.read_raw() };
                log::trace!("Emulated read of msr {:x} = h:{:x};l:{:x}", ecx, high, low);
                context.set(VmcsField::GuestRax, low as usize, None)?;
                context.set(VmcsField::GuestRdx, high as usize, None)?;
                vs.vcpu.next_instruction().or(Err(CapaError::PlatformError))?;
                Ok(HandlerResult::Resume)
            }
        }
        // Routing exits to the manager domains.
        VmxExitReason::EptViolation
        | VmxExitReason::ExternalInterrupt
        | VmxExitReason::IoInstruction
        | VmxExitReason::ControlRegisterAccesses
        | VmxExitReason::TripleFault
        | VmxExitReason::Cpuid
        | VmxExitReason::Exception
        | VmxExitReason::Wrmsr
        | VmxExitReason::Rdmsr
        | VmxExitReason::ApicWrite
        | VmxExitReason::InterruptWindow
        | VmxExitReason::Wbinvd
        | VmxExitReason::MovDR
        | VmxExitReason::VirtualizedEoi
        | VmxExitReason::ApicAccess
        | VmxExitReason::VmxPreemptionTimerExpired
        | VmxExitReason::Hlt => {
            log::trace!("Handling {:?} for dom {} on core {}", reason, domain.idx(), cpuid());
            if reason == VmxExitReason::ExternalInterrupt {
                /*let address_eoi = 0xfee000b0 as *mut u32;
                unsafe {
                    // Clear the eoi
                    *address_eoi = 0;
                }*/
                x2apic::send_eoi();
            }
            match Self::do_handle_violation(vs, domain) {
                Ok(_) => {
                    return Ok(HandlerResult::Resume);
                }
                Err(e) => {
                    log::error!("Unable to handle {:?}: {:?}", reason, e);
                    log::info!("The vcpu: {:x?}", vs.vcpu);
                    return Ok(HandlerResult::Crash);
                }
            }
        }
        _ => {
            log::error!(
                "Emulation is not yet implemented for exit reason: {:?}",
                reason
            );
            log::info!("{:?}", vs.vcpu);
            Ok(HandlerResult::Crash)
        }
        }
    }
}

// ———————————————————— Translate fields into registers ————————————————————— //

const X86_FIELDS16: [VmcsField; NB_16] = [
    VmcsField::VirtualProcessorId,
    VmcsField::PostedIntrNv,
    VmcsField::LastPidPointerIndex,
    VmcsField::GuestEsSelector,
    VmcsField::GuestCsSelector,
    VmcsField::GuestSsSelector,
    VmcsField::GuestDsSelector,
    VmcsField::GuestFsSelector,
    VmcsField::GuestGsSelector,
    VmcsField::GuestLdtrSelector,
    VmcsField::GuestTrSelector,
    VmcsField::GuestIntrStatus,
    VmcsField::GuestPmlIndex,
    VmcsField::HostEsSelector,
    VmcsField::HostCsSelector,
    VmcsField::HostSsSelector,
    VmcsField::HostDsSelector,
    VmcsField::HostFsSelector,
    VmcsField::HostGsSelector,
    VmcsField::HostTrSelector,
];

const X86_FIELDS32: [VmcsField; NB_32] = [
    VmcsField::IoBitmapA,
    VmcsField::IoBitmapB,
    VmcsField::MsrBitmap,
    VmcsField::VmExitMsrStoreAddr,
    VmcsField::VmExitMsrLoadAddr,
    VmcsField::VmEntryMsrLoadAddr,
    VmcsField::PmlAddress,
    VmcsField::TscOffset,
    VmcsField::VirtualApicPageAddr,
    VmcsField::ApicAccessAddr,
    VmcsField::PostedIntrDescAddr,
    VmcsField::VmFunctionControl,
    VmcsField::EptPointer,
    VmcsField::EoiExitBitmap0,
    VmcsField::EoiExitBitmap1,
    VmcsField::EoiExitBitmap2,
    VmcsField::EoiExitBitmap3,
    VmcsField::EptpListAddress,
    VmcsField::VmreadBitmap,
    VmcsField::VmwriteBitmap,
    VmcsField::XssExitBitmap,
    VmcsField::EnclsExitingBitmap,
    VmcsField::TscMultiplier,
    VmcsField::TertiaryVmExecControl,
    VmcsField::PidPointerTable,
    VmcsField::GuestPhysicalAddress,
    VmcsField::VmcsLinkPointer,
    VmcsField::GuestIa32Debugctl,
    VmcsField::GuestIa32Pat,
    VmcsField::GuestIa32Efer,
    VmcsField::GuestIa32PerfGlobalCtrl,
    VmcsField::GuestPdptr0,
    VmcsField::GuestPdptr1,
    VmcsField::GuestPdptr2,
    VmcsField::GuestPdptr3,
    VmcsField::GuestBndcfgs,
    VmcsField::GuestIa32RtitCtl,
    VmcsField::HostIa32Pat,
    VmcsField::HostIa32Efer,
    VmcsField::HostIa32PerfGlobalCtrl,
];

const X86_FIELDS64: [VmcsField; NB_64] = [
    VmcsField::PinBasedVmExecControl,
    VmcsField::CpuBasedVmExecControl,
    VmcsField::ExceptionBitmap,
    VmcsField::PageFaultErrorCodeMask,
    VmcsField::PageFaultErrorCodeMatch,
    VmcsField::Cr3TargetCount,
    VmcsField::VmExitControls,
    VmcsField::VmExitMsrStoreCount,
    VmcsField::VmExitMsrLoadCount,
    VmcsField::VmEntryControls,
    VmcsField::VmEntryMsrLoadCount,
    VmcsField::VmEntryIntrInfoField,
    VmcsField::VmEntryExceptionErrorCode,
    VmcsField::VmEntryInstructionLen,
    VmcsField::TprThreshold,
    VmcsField::SecondaryVmExecControl,
    VmcsField::PleGap,
    VmcsField::PleWindow,
    VmcsField::NotifyWindow,
    VmcsField::VmInstructionError,
    VmcsField::VmExitReason,
    VmcsField::VmExitIntrInfo,
    VmcsField::VmExitIntrErrorCode,
    VmcsField::IdtVectoringInfoField,
    VmcsField::IdtVectoringErrorCode,
    VmcsField::VmExitInstructionLen,
    VmcsField::VmxInstructionInfo,
    VmcsField::GuestEsLimit,
    VmcsField::GuestCsLimit,
    VmcsField::GuestSsLimit,
    VmcsField::GuestDsLimit,
    VmcsField::GuestFsLimit,
    VmcsField::GuestGsLimit,
    VmcsField::GuestLdtrLimit,
    VmcsField::GuestTrLimit,
    VmcsField::GuestGdtrLimit,
    VmcsField::GuestIdtrLimit,
    VmcsField::GuestEsArBytes,
    VmcsField::GuestCsArBytes,
    VmcsField::GuestSsArBytes,
    VmcsField::GuestDsArBytes,
    VmcsField::GuestFsArBytes,
    VmcsField::GuestGsArBytes,
    VmcsField::GuestLdtrArBytes,
    VmcsField::GuestTrArBytes,
    VmcsField::GuestInterruptibilityInfo,
    VmcsField::GuestActivityState,
    VmcsField::GuestSysenterCs,
    VmcsField::VmxPreemptionTimerValue,
];

const X86_FIELDSNAT: [VmcsField; NB_NAT] = [
    VmcsField::Cr0GuestHostMask,
    VmcsField::Cr4GuestHostMask,
    VmcsField::Cr0ReadShadow,
    VmcsField::Cr4ReadShadow,
    VmcsField::Cr3TargetValue0,
    VmcsField::Cr3TargetValue1,
    VmcsField::Cr3TargetValue2,
    VmcsField::Cr3TargetValue3,
    VmcsField::ExitQualification,
    VmcsField::GuestLinearAddress,
    VmcsField::GuestCr0,
    VmcsField::GuestCr3,
    VmcsField::GuestCr4,
    VmcsField::GuestEsBase,
    VmcsField::GuestCsBase,
    VmcsField::GuestSsBase,
    VmcsField::GuestDsBase,
    VmcsField::GuestFsBase,
    VmcsField::GuestGsBase,
    VmcsField::GuestLdtrBase,
    VmcsField::GuestTrBase,
    VmcsField::GuestGdtrBase,
    VmcsField::GuestIdtrBase,
    VmcsField::GuestDr7,
    VmcsField::GuestRsp,
    VmcsField::GuestRip,
    VmcsField::GuestRflags,
    VmcsField::GuestPendingDbgExceptions,
    VmcsField::GuestSysenterEsp,
    VmcsField::GuestSysenterEip,
];

const X86_FIELDSGP: [VmcsField; NB_GP] = [
    VmcsField::GuestRax,
    VmcsField::GuestRbx,
    VmcsField::GuestRcx,
    VmcsField::GuestRdx,
    VmcsField::GuestRbp,
    VmcsField::GuestRsi,
    VmcsField::GuestRdi,
    VmcsField::GuestR8,
    VmcsField::GuestR9,
    VmcsField::GuestR10,
    VmcsField::GuestR11,
    VmcsField::GuestR12,
    VmcsField::GuestR13,
    VmcsField::GuestR14,
    VmcsField::GuestR15,
    VmcsField::GuestLstar,
];

fn translate_to_x86field(r: RegisterGroup, idx: usize) -> Result<VmcsField, CapaError> {
    match r {
        RegisterGroup::Reg16 if idx < NB_16 => Ok(X86_FIELDS16[idx]),
        RegisterGroup::Reg32 if idx < NB_32 => Ok(X86_FIELDS32[idx]),
        RegisterGroup::Reg64 if idx < NB_64 => Ok(X86_FIELDS64[idx]),
        RegisterGroup::RegNat if idx < NB_NAT => Ok(X86_FIELDSNAT[idx]),
        RegisterGroup::RegGp if idx < NB_GP => Ok(X86_FIELDSGP[idx]),
        _ => Err(CapaError::InvalidValue),
    }
}

/// All the fields for x86.
fn translate_x86field(field: VmcsField) -> Option<(RegisterGroup, usize)> {
    match field {
        // The 16-bits registers.
        VmcsField::VirtualProcessorId => Some((RegisterGroup::Reg16, 0)),
        VmcsField::PostedIntrNv => Some((RegisterGroup::Reg16, 1)),
        VmcsField::LastPidPointerIndex => Some((RegisterGroup::Reg16, 2)),
        VmcsField::GuestEsSelector => Some((RegisterGroup::Reg16, 3)),
        VmcsField::GuestCsSelector => Some((RegisterGroup::Reg16, 4)),
        VmcsField::GuestSsSelector => Some((RegisterGroup::Reg16, 5)),
        VmcsField::GuestDsSelector => Some((RegisterGroup::Reg16, 6)),
        VmcsField::GuestFsSelector => Some((RegisterGroup::Reg16, 7)),
        VmcsField::GuestGsSelector => Some((RegisterGroup::Reg16, 8)),
        VmcsField::GuestLdtrSelector => Some((RegisterGroup::Reg16, 9)),
        VmcsField::GuestTrSelector => Some((RegisterGroup::Reg16, 10)),
        VmcsField::GuestIntrStatus => Some((RegisterGroup::Reg16, 11)),
        VmcsField::GuestPmlIndex => Some((RegisterGroup::Reg16, 12)),
        VmcsField::HostEsSelector => Some((RegisterGroup::Reg16, 13)),
        VmcsField::HostCsSelector => Some((RegisterGroup::Reg16, 14)),
        VmcsField::HostSsSelector => Some((RegisterGroup::Reg16, 15)),
        VmcsField::HostDsSelector => Some((RegisterGroup::Reg16, 16)),
        VmcsField::HostFsSelector => Some((RegisterGroup::Reg16, 17)),
        VmcsField::HostGsSelector => Some((RegisterGroup::Reg16, 18)),
        VmcsField::HostTrSelector => Some((RegisterGroup::Reg16, 19)),
        // The 32-bits registers.
        VmcsField::IoBitmapA => Some((RegisterGroup::Reg32, 0)),
        VmcsField::IoBitmapB => Some((RegisterGroup::Reg32, 1)),
        VmcsField::MsrBitmap => Some((RegisterGroup::Reg32, 2)),
        VmcsField::VmExitMsrStoreAddr => Some((RegisterGroup::Reg32, 3)),
        VmcsField::VmExitMsrLoadAddr => Some((RegisterGroup::Reg32, 4)),
        VmcsField::VmEntryMsrLoadAddr => Some((RegisterGroup::Reg32, 5)),
        VmcsField::PmlAddress => Some((RegisterGroup::Reg32, 6)),
        VmcsField::TscOffset => Some((RegisterGroup::Reg32, 7)),
        VmcsField::VirtualApicPageAddr => Some((RegisterGroup::Reg32, 8)),
        VmcsField::ApicAccessAddr => Some((RegisterGroup::Reg32, 9)),
        VmcsField::PostedIntrDescAddr => Some((RegisterGroup::Reg32, 10)),
        VmcsField::VmFunctionControl => Some((RegisterGroup::Reg32, 11)),
        VmcsField::EptPointer => Some((RegisterGroup::Reg32, 12)),
        VmcsField::EoiExitBitmap0 => Some((RegisterGroup::Reg32, 13)),
        VmcsField::EoiExitBitmap1 => Some((RegisterGroup::Reg32, 14)),
        VmcsField::EoiExitBitmap2 => Some((RegisterGroup::Reg32, 15)),
        VmcsField::EoiExitBitmap3 => Some((RegisterGroup::Reg32, 16)),
        VmcsField::EptpListAddress => Some((RegisterGroup::Reg32, 17)),
        VmcsField::VmreadBitmap => Some((RegisterGroup::Reg32, 18)),
        VmcsField::VmwriteBitmap => Some((RegisterGroup::Reg32, 19)),
        VmcsField::XssExitBitmap => Some((RegisterGroup::Reg32, 20)),
        VmcsField::EnclsExitingBitmap => Some((RegisterGroup::Reg32, 21)),
        VmcsField::TscMultiplier => Some((RegisterGroup::Reg32, 22)),
        VmcsField::TertiaryVmExecControl => Some((RegisterGroup::Reg32, 23)),
        VmcsField::PidPointerTable => Some((RegisterGroup::Reg32, 24)),
        VmcsField::GuestPhysicalAddress => Some((RegisterGroup::Reg32, 25)),
        VmcsField::VmcsLinkPointer => Some((RegisterGroup::Reg32, 26)),
        VmcsField::GuestIa32Debugctl => Some((RegisterGroup::Reg32, 27)),
        VmcsField::GuestIa32Pat => Some((RegisterGroup::Reg32, 28)),
        VmcsField::GuestIa32Efer => Some((RegisterGroup::Reg32, 29)),
        VmcsField::GuestIa32PerfGlobalCtrl => Some((RegisterGroup::Reg32, 30)),
        VmcsField::GuestPdptr0 => Some((RegisterGroup::Reg32, 31)),
        VmcsField::GuestPdptr1 => Some((RegisterGroup::Reg32, 32)),
        VmcsField::GuestPdptr2 => Some((RegisterGroup::Reg32, 33)),
        VmcsField::GuestPdptr3 => Some((RegisterGroup::Reg32, 34)),
        VmcsField::GuestBndcfgs => Some((RegisterGroup::Reg32, 35)),
        VmcsField::GuestIa32RtitCtl => Some((RegisterGroup::Reg32, 36)),
        VmcsField::HostIa32Pat => Some((RegisterGroup::Reg32, 37)),
        VmcsField::HostIa32Efer => Some((RegisterGroup::Reg32, 38)),
        VmcsField::HostIa32PerfGlobalCtrl => Some((RegisterGroup::Reg32, 39)),
        // The 64-bits registers.
        VmcsField::PinBasedVmExecControl => Some((RegisterGroup::Reg64, 0)),
        VmcsField::CpuBasedVmExecControl => Some((RegisterGroup::Reg64, 1)),
        VmcsField::ExceptionBitmap => Some((RegisterGroup::Reg64, 2)),
        VmcsField::PageFaultErrorCodeMask => Some((RegisterGroup::Reg64, 3)),
        VmcsField::PageFaultErrorCodeMatch => Some((RegisterGroup::Reg64, 4)),
        VmcsField::Cr3TargetCount => Some((RegisterGroup::Reg64, 5)),
        VmcsField::VmExitControls => Some((RegisterGroup::Reg64, 6)),
        VmcsField::VmExitMsrStoreCount => Some((RegisterGroup::Reg64, 7)),
        VmcsField::VmExitMsrLoadCount => Some((RegisterGroup::Reg64, 8)),
        VmcsField::VmEntryControls => Some((RegisterGroup::Reg64, 9)),
        VmcsField::VmEntryMsrLoadCount => Some((RegisterGroup::Reg64, 10)),
        VmcsField::VmEntryIntrInfoField => Some((RegisterGroup::Reg64, 11)),
        VmcsField::VmEntryExceptionErrorCode => Some((RegisterGroup::Reg64, 12)),
        VmcsField::VmEntryInstructionLen => Some((RegisterGroup::Reg64, 13)),
        VmcsField::TprThreshold => Some((RegisterGroup::Reg64, 14)),
        VmcsField::SecondaryVmExecControl => Some((RegisterGroup::Reg64, 15)),
        VmcsField::PleGap => Some((RegisterGroup::Reg64, 16)),
        VmcsField::PleWindow => Some((RegisterGroup::Reg64, 17)),
        VmcsField::NotifyWindow => Some((RegisterGroup::Reg64, 18)),
        VmcsField::VmInstructionError => Some((RegisterGroup::Reg64, 19)),
        VmcsField::VmExitReason => Some((RegisterGroup::Reg64, 20)),
        VmcsField::VmExitIntrInfo => Some((RegisterGroup::Reg64, 21)),
        VmcsField::VmExitIntrErrorCode => Some((RegisterGroup::Reg64, 22)),
        VmcsField::IdtVectoringInfoField => Some((RegisterGroup::Reg64, 23)),
        VmcsField::IdtVectoringErrorCode => Some((RegisterGroup::Reg64, 24)),
        VmcsField::VmExitInstructionLen => Some((RegisterGroup::Reg64, 25)),
        VmcsField::VmxInstructionInfo => Some((RegisterGroup::Reg64, 26)),
        VmcsField::GuestEsLimit => Some((RegisterGroup::Reg64, 27)),
        VmcsField::GuestCsLimit => Some((RegisterGroup::Reg64, 28)),
        VmcsField::GuestSsLimit => Some((RegisterGroup::Reg64, 29)),
        VmcsField::GuestDsLimit => Some((RegisterGroup::Reg64, 30)),
        VmcsField::GuestFsLimit => Some((RegisterGroup::Reg64, 31)),
        VmcsField::GuestGsLimit => Some((RegisterGroup::Reg64, 32)),
        VmcsField::GuestLdtrLimit => Some((RegisterGroup::Reg64, 33)),
        VmcsField::GuestTrLimit => Some((RegisterGroup::Reg64, 34)),
        VmcsField::GuestGdtrLimit => Some((RegisterGroup::Reg64, 35)),
        VmcsField::GuestIdtrLimit => Some((RegisterGroup::Reg64, 36)),
        VmcsField::GuestEsArBytes => Some((RegisterGroup::Reg64, 37)),
        VmcsField::GuestCsArBytes => Some((RegisterGroup::Reg64, 38)),
        VmcsField::GuestSsArBytes => Some((RegisterGroup::Reg64, 39)),
        VmcsField::GuestDsArBytes => Some((RegisterGroup::Reg64, 40)),
        VmcsField::GuestFsArBytes => Some((RegisterGroup::Reg64, 41)),
        VmcsField::GuestGsArBytes => Some((RegisterGroup::Reg64, 42)),
        VmcsField::GuestLdtrArBytes => Some((RegisterGroup::Reg64, 43)),
        VmcsField::GuestTrArBytes => Some((RegisterGroup::Reg64, 44)),
        VmcsField::GuestInterruptibilityInfo => Some((RegisterGroup::Reg64, 45)),
        VmcsField::GuestActivityState => Some((RegisterGroup::Reg64, 46)),
        VmcsField::GuestSysenterCs => Some((RegisterGroup::Reg64, 47)),
        VmcsField::VmxPreemptionTimerValue => Some((RegisterGroup::Reg64, 48)),
        // The NAT-bits registers.
        VmcsField::Cr0GuestHostMask => Some((RegisterGroup::RegNat, 0)),
        VmcsField::Cr4GuestHostMask => Some((RegisterGroup::RegNat, 1)),
        VmcsField::Cr0ReadShadow => Some((RegisterGroup::RegNat, 2)),
        VmcsField::Cr4ReadShadow => Some((RegisterGroup::RegNat, 3)),
        VmcsField::Cr3TargetValue0 => Some((RegisterGroup::RegNat, 4)),
        VmcsField::Cr3TargetValue1 => Some((RegisterGroup::RegNat, 5)),
        VmcsField::Cr3TargetValue2 => Some((RegisterGroup::RegNat, 6)),
        VmcsField::Cr3TargetValue3 => Some((RegisterGroup::RegNat, 7)),
        VmcsField::ExitQualification => Some((RegisterGroup::RegNat, 8)),
        VmcsField::GuestLinearAddress => Some((RegisterGroup::RegNat, 9)),
        VmcsField::GuestCr0 => Some((RegisterGroup::RegNat, 10)),
        VmcsField::GuestCr3 => Some((RegisterGroup::RegNat, 11)),
        VmcsField::GuestCr4 => Some((RegisterGroup::RegNat, 12)),
        VmcsField::GuestEsBase => Some((RegisterGroup::RegNat, 13)),
        VmcsField::GuestCsBase => Some((RegisterGroup::RegNat, 14)),
        VmcsField::GuestSsBase => Some((RegisterGroup::RegNat, 15)),
        VmcsField::GuestDsBase => Some((RegisterGroup::RegNat, 16)),
        VmcsField::GuestFsBase => Some((RegisterGroup::RegNat, 17)),
        VmcsField::GuestGsBase => Some((RegisterGroup::RegNat, 18)),
        VmcsField::GuestLdtrBase => Some((RegisterGroup::RegNat, 19)),
        VmcsField::GuestTrBase => Some((RegisterGroup::RegNat, 20)),
        VmcsField::GuestGdtrBase => Some((RegisterGroup::RegNat, 21)),
        VmcsField::GuestIdtrBase => Some((RegisterGroup::RegNat, 22)),
        VmcsField::GuestDr7 => Some((RegisterGroup::RegNat, 23)),
        VmcsField::GuestRsp => Some((RegisterGroup::RegNat, 24)),
        VmcsField::GuestRip => Some((RegisterGroup::RegNat, 25)),
        VmcsField::GuestRflags => Some((RegisterGroup::RegNat, 26)),
        VmcsField::GuestPendingDbgExceptions => Some((RegisterGroup::RegNat, 27)),
        VmcsField::GuestSysenterEsp => Some((RegisterGroup::RegNat, 28)),
        VmcsField::GuestSysenterEip => Some((RegisterGroup::RegNat, 29)),
        // The GP-bits registers.
        VmcsField::GuestRax => Some((RegisterGroup::RegGp, 0)),
        VmcsField::GuestRbx => Some((RegisterGroup::RegGp, 1)),
        VmcsField::GuestRcx => Some((RegisterGroup::RegGp, 2)),
        VmcsField::GuestRdx => Some((RegisterGroup::RegGp, 3)),
        VmcsField::GuestRbp => Some((RegisterGroup::RegGp, 4)),
        VmcsField::GuestRsi => Some((RegisterGroup::RegGp, 5)),
        VmcsField::GuestRdi => Some((RegisterGroup::RegGp, 6)),
        VmcsField::GuestR8 => Some((RegisterGroup::RegGp, 7)),
        VmcsField::GuestR9 => Some((RegisterGroup::RegGp, 8)),
        VmcsField::GuestR10 => Some((RegisterGroup::RegGp, 9)),
        VmcsField::GuestR11 => Some((RegisterGroup::RegGp, 10)),
        VmcsField::GuestR12 => Some((RegisterGroup::RegGp, 11)),
        VmcsField::GuestR13 => Some((RegisterGroup::RegGp, 12)),
        VmcsField::GuestR14 => Some((RegisterGroup::RegGp, 13)),
        VmcsField::GuestR15 => Some((RegisterGroup::RegGp, 14)),
        VmcsField::GuestLstar => Some((RegisterGroup::RegGp, 15)),
        _ => None,
    }
}
