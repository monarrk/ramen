// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![feature(start, asm)]
#![no_main]
#![deny(clippy::pedantic)]
#![deny(clippy::all)]

#[macro_use]
#[allow(unused_imports)]
extern crate common;

#[macro_use]
extern crate log;

extern crate x86_64;

mod acpi;
mod exit;
mod fs;
mod gop;
mod mem;

use common::{kernelboot, mem::reserved};
use core::{convert::TryFrom, ptr, ptr::NonNull, slice};
use fs::kernel;
use mem::{free_page, paging, stack};
use uefi::{
    prelude::{Boot, Handle, SystemTable},
    table::{boot, boot::MemoryType},
    ResultExt,
};

#[start]
#[no_mangle]
pub fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> ! {
    init_libs(&system_table);

    let vram_info = gop::init(system_table.boot_services());

    let (phys_kernel_addr, bytes_kernel) = kernel::deploy(system_table.boot_services());
    let (entry_addr, actual_mem_size) =
        kernel::fetch_entry_address_and_memory_size(phys_kernel_addr, bytes_kernel);

    let stack_addr = stack::allocate(system_table.boot_services());
    let free_page = free_page::allocate(system_table.boot_services());
    let reserved_regions = reserved::Map::new(
        &reserved::KernelPhysRange::new(phys_kernel_addr, actual_mem_size),
        stack_addr,
        &vram_info,
        free_page,
    );
    let mem_map = terminate_boot_services(image, system_table);

    exit::bootx64(kernelboot::Info::new(
        entry_addr,
        vram_info,
        mem_map,
        reserved_regions,
    ));
}

fn init_libs(system_table: &SystemTable<Boot>) {
    initialize_uefi_utilities(&system_table);
    reset_console(&system_table);
    info!("Hello World!");
}

fn initialize_uefi_utilities(system_table: &SystemTable<Boot>) {
    uefi_services::init(system_table).expect_success("Failed to initialize_uefi_utilities");
}

fn reset_console(system_table: &SystemTable<Boot>) {
    system_table
        .stdout()
        .reset(false)
        .expect_success("Failed to reset stdout");
}

fn terminate_boot_services(image: Handle, system_table: SystemTable<Boot>) -> common::mem::Map {
    info!("Goodbye, boot services...");
    let memory_map_buf = NonNull::new(
        system_table
            .boot_services()
            .allocate_pool(
                MemoryType::LOADER_DATA,
                system_table.boot_services().memory_map_size(),
            )
            .expect_success("Failed to allocate memory for memory map"),
    )
    .unwrap()
    .cast::<boot::MemoryDescriptor>();

    let buf_for_exiting = system_table
        .boot_services()
        .allocate_pool(
            MemoryType::LOADER_DATA,
            system_table.boot_services().memory_map_size() * 2,
        )
        .expect_success("Failed to allocate memory to exit boot services");
    let buf_for_exiting = unsafe {
        slice::from_raw_parts_mut(
            buf_for_exiting,
            system_table.boot_services().memory_map_size() * 2,
        )
    };

    let (_, descriptors_iter) = system_table
        .exit_boot_services(image, buf_for_exiting)
        .expect("Failed to exit boot services")
        .unwrap();

    let mut num_descriptors = 0;
    for (index, descriptor) in descriptors_iter.enumerate() {
        unsafe {
            ptr::write(
                memory_map_buf
                    .as_ptr()
                    .offset(isize::try_from(index).unwrap()),
                *descriptor,
            );
        }

        num_descriptors += 1;
    }

    common::mem::Map::new(memory_map_buf, num_descriptors)
}
