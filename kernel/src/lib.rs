// SPDX-License-Identifier: GPL-3.0-or-later

#![no_std]
#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(wake_trait)]
#![feature(asm)]
#![feature(panic_info_message)]
#![feature(start)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]
#![deny(clippy::pedantic)]
#![deny(clippy::all)]

#[macro_use]
#[allow(unused_imports)]
extern crate common;
extern crate alloc;
#[macro_use]
extern crate log;
extern crate x86_64;

mod allocator;
mod device;
mod gdt;
mod idt;
mod interrupt;
mod multitask;
mod panic;

#[macro_use]
mod graphics;

use {
    allocator::{FrameManager, ALLOCATOR},
    common::{
        constant::{BYTES_KERNEL_HEAP, KERNEL_HEAP_ADDR},
        kernelboot,
    },
    core::convert::TryFrom,
    device::{keyboard, mouse},
    graphics::{screen, Vram},
    multitask::{executor::Executor, task::Task},
};

#[no_mangle]
#[start]
pub extern "win64" fn os_main(boot_info: kernelboot::Info) -> ! {
    initialization(&boot_info);

    run_tasks();
}

fn initialization(boot_info: &kernelboot::Info) {
    Vram::init(&boot_info);

    gdt::init();
    idt::init();
    interrupt::init_pic();

    FrameManager::init(boot_info.mem_map());

    unsafe {
        ALLOCATOR.lock().init(
            usize::try_from(KERNEL_HEAP_ADDR.as_u64()).unwrap(),
            BYTES_KERNEL_HEAP.as_usize(),
        )
    }

    screen::log::init().unwrap();

    graphics::screen::draw_desktop();

    info!("Hello Ramen OS!");
    info!("Vram information: {}", Vram::display());

    interrupt::set_init_pic_bits();
}

fn run_tasks() -> ! {
    let mut executor = Executor::new();
    executor.spawn(Task::new(keyboard::task()));
    executor.spawn(Task::new(mouse::task()));
    executor.run();
}

#[cfg(feature = "qemu_test")]
fn main_loop(mouse_device: &mut mouse::Device, mouse_cursor: &mut screen::MouseCursor) -> ! {
    // Because of `hlt` instruction, running `loop_main` many times is impossible.
    loop_main(mouse_device, mouse_cursor);

    // If you change the value `0xf4` and `0x10`, don't forget to change the correspond values in
    // `Makefile`!
    qemu_exit::x86::exit::<u32, 0xf4>(0x10);
}
