#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use kernel::println;
use kernel::vmx;

use bootloader::{entry_point, BootInfo};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("=========== Start QEMU ===========");

    kernel::init();
    let vma_allocator =
        unsafe { kernel::init_memory(boot_info).expect("Failed to initialize memory") };

    unsafe {
        println!("VMX:    {:?}", vmx::vmx_available());
        println!("VMXON:  {:?}", vmx::vmxon(&vma_allocator));

        let vmcs = vmx::VmcsRegion::new(&vma_allocator);
        if let Err(err) = vmcs {
            println!("VMCS:   Err({:?})", err);
        } else {
            println!("VMCS:   Ok(())");
        }

        println!("VMXOFF: {:?}", vmx::vmxoff());
    }

    #[cfg(test)]
    test_main();

    kernel::qemu::exit(kernel::qemu::ExitCode::Success);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    kernel::qemu::exit(kernel::qemu::ExitCode::Failure);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kernel::test_panic_handler(info);
}
