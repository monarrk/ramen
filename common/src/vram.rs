// SPDX-License-Identifier: GPL-3.0-or-later

use {
    core::convert::TryFrom,
    os_units::{Bytes, Size},
    uefi::proto::console::gop,
    x86_64::PhysAddr,
};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Info {
    bpp: u32,
    screen_x: u32,
    screen_y: u32,
    ptr: PhysAddr,
}

impl Info {
    pub fn new_from_gop(gop: &mut gop::GraphicsOutput) -> Self {
        let (screen_x, screen_y) = gop.current_mode_info().resolution();

        Self {
            bpp: 32,
            screen_x: u32::try_from(screen_x).unwrap(),
            screen_y: u32::try_from(screen_y).unwrap(),
            ptr: PhysAddr::new(gop.frame_buffer().as_mut_ptr() as u64),
        }
    }

    #[must_use]
    pub fn bpp(&self) -> u32 {
        self.bpp
    }

    #[must_use]
    pub fn resolution(&self) -> (u32, u32) {
        (self.screen_x, self.screen_y)
    }

    #[must_use]
    pub fn phys_ptr(&self) -> PhysAddr {
        self.ptr
    }

    #[must_use]
    pub fn bytes(&self) -> Size<Bytes> {
        Size::new(
            usize::try_from(self.screen_x * self.screen_y * self.bpp / 8)
                .expect("The bytes of VRAM must not be negative"),
        )
    }
}
