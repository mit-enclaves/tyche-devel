//! Architecture specific monitor state, independant of the CapaEngine.

use capa_engine::{
    permission, AccessRights, CapaEngine, CapaInfo, Context, Domain, Handle, LocalCapa,
    NextCapaToken, N,
};
use mmu::eptmapper::EPT_ROOT_FLAGS;
use mmu::{EptMapper, FrameAllocator};
use spin::{Mutex, MutexGuard};
use stage_two_abi::Manifest;
use utils::{GuestPhysAddr, HostPhysAddr};
use vmx::bitmaps::EptEntryFlags;
use vmx::ActiveVmcs;

use super::cpuid;
use crate::allocator::allocator;
use crate::statics::NB_CORES;

// ————————————————————————— Statics & Backend Data ————————————————————————— //

static CAPA_ENGINE: Mutex<CapaEngine> = Mutex::new(CapaEngine::new());
static INITIAL_DOMAIN: Mutex<Option<Handle<Domain>>> = Mutex::new(None);
static DOMAINS: [Mutex<DomainData>; N] = [EMPTY_DOMAIN; N];
static CORES: [Mutex<CoreData>; NB_CORES] = [EMPTY_CORE; NB_CORES];
static CONTEXTS: [Mutex<ContextData>; N] = [EMPTY_CONTEXT; N];

pub struct DomainData {
    ept: Option<HostPhysAddr>,
}

pub struct CoreData {
    domain: Handle<Domain>,
}

pub struct ContextData {
    cr3: usize,
    rip: usize,
    rsp: usize,
}

const EMPTY_DOMAIN: Mutex<DomainData> = Mutex::new(DomainData { ept: None });
const EMPTY_CORE: Mutex<CoreData> = Mutex::new(CoreData {
    domain: Handle::new_invalid(),
});
const EMPTY_CONTEXT: Mutex<ContextData> = Mutex::new(ContextData {
    cr3: 0,
    rip: 0,
    rsp: 0,
});

// ————————————————————————————— Initialization ————————————————————————————— //

pub fn init(manifest: &'static Manifest) {
    let mut engine = CAPA_ENGINE.lock();
    let domain = engine.create_manager_domain(permission::ALL).unwrap();
    apply_updates(&mut engine);
    engine
        .create_region(
            domain,
            AccessRights {
                start: 0,
                end: manifest.poffset as usize,
            },
        )
        .unwrap();
    apply_updates(&mut engine);

    // Save the initial domain
    let mut initial_domain = INITIAL_DOMAIN.lock();
    *initial_domain = Some(domain);
}

pub fn init_vcpu(vcpu: &mut ActiveVmcs<'static>) -> Handle<Domain> {
    let initial_domain = INITIAL_DOMAIN
        .lock()
        .expect("CapaEngine is not initialized yet");
    let domain = get_domain(initial_domain);
    vcpu.set_ept_ptr(HostPhysAddr::new(
        domain.ept.unwrap().as_usize() | EPT_ROOT_FLAGS,
    ))
    .expect("Failed to set initial EPT PTR");
    let mut core = get_core(cpuid());
    core.domain = initial_domain;
    initial_domain
}

// ———————————————————————————————— Helpers ————————————————————————————————— //

fn get_domain(domain: Handle<Domain>) -> MutexGuard<'static, DomainData> {
    DOMAINS[domain.idx()].lock()
}

fn get_context(context: Handle<Context>) -> MutexGuard<'static, ContextData> {
    CONTEXTS[context.idx()].lock()
}

fn get_core(cpuid: usize) -> MutexGuard<'static, CoreData> {
    CORES[cpuid].lock()
}

// ————————————————————————————— Monitor Calls —————————————————————————————— //

pub fn do_create_domain(current: Handle<Domain>) -> Result<LocalCapa, ()> {
    let mut engine = CAPA_ENGINE.lock();
    let management_capa = engine.create_domain(current).expect("TODO: handle failure");
    apply_updates(&mut engine);
    Ok(management_capa)
}

pub fn do_seal(
    current: Handle<Domain>,
    domain: LocalCapa,
    cr3: usize,
    rip: usize,
    rsp: usize,
) -> Result<LocalCapa, ()> {
    let mut engine = CAPA_ENGINE.lock();
    let (capa, context) = engine.seal(current, domain).expect("TODO: handle failure");
    let mut context = get_context(context);
    context.cr3 = cr3;
    context.rip = rip;
    context.rsp = rsp;
    Ok(capa)
}

pub fn do_send(current: Handle<Domain>, capa: LocalCapa, to: LocalCapa) -> Result<(), ()> {
    let mut engine = CAPA_ENGINE.lock();
    engine
        .send(current, capa, to)
        .expect("TODO: handle failure");
    Ok(())
}

pub fn do_enumerate(
    current: Handle<Domain>,
    token: NextCapaToken,
) -> Option<(CapaInfo, NextCapaToken)> {
    let mut engine = CAPA_ENGINE.lock();
    engine.enumerate(current, token)
}

// ———————————————————————————————— Updates ————————————————————————————————— //

fn apply_updates(engine: &mut MutexGuard<CapaEngine>) {
    while let Some(update) = engine.pop_update() {
        match update {
            capa_engine::Update::PermissionUpdate { domain } => update_permission(domain, engine),
            capa_engine::Update::RevokeDomain { domain } => revoke_domain(domain),
            capa_engine::Update::CreateDomain { domain } => create_domain(domain),
            capa_engine::Update::None => todo!(),
        }
    }
}

fn create_domain(domain: Handle<Domain>) {
    let mut domain = get_domain(domain);
    let allocator = allocator();
    if let Some(_ept) = domain.ept {
        // TODO: free all frames.
        // unsafe {
        //     allocator.free_frame(ept).unwrap();
        // }
    }

    let ept_root = allocator
        .allocate_frame()
        .expect("Failled to allocate EPT root")
        .zeroed();
    domain.ept = Some(ept_root.phys_addr);
}

fn revoke_domain(_domain: Handle<Domain>) {
    // Noop for now, might need to send IPIs once we land multi-core
}

fn update_permission(domain_handle: Handle<Domain>, engine: &mut MutexGuard<CapaEngine>) {
    // TODO: handle granular access rights
    let flags = EptEntryFlags::USER_EXECUTE
        | EptEntryFlags::SUPERVISOR_EXECUTE
        | EptEntryFlags::READ
        | EptEntryFlags::WRITE
        | EptEntryFlags::SUPERVISOR_EXECUTE;

    let mut domain = get_domain(domain_handle);
    let allocator = allocator();
    if let Some(_ept) = domain.ept {
        // TODO: free all frames.
        // unsafe {
        //     allocator.free_frame(ept).unwrap();
        // }
    }

    let ept_root = allocator
        .allocate_frame()
        .expect("Failled to allocate EPT root")
        .zeroed();
    let mut mapper = EptMapper::new(
        allocator.get_physical_offset().as_usize(),
        ept_root.phys_addr,
    );

    for range in engine[domain_handle].regions().permissions() {
        mapper.map_range(
            allocator,
            GuestPhysAddr::new(range.start),
            HostPhysAddr::new(range.start),
            range.size(),
            flags,
        )
    }

    domain.ept = Some(ept_root.phys_addr);
}
