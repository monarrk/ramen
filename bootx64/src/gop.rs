use core::mem::MaybeUninit;
use uefi::prelude::{Boot, SystemTable};
use uefi::proto::console::gop;
use uefi::proto::console::gop::PixelFormat;
use uefi::ResultExt;

pub struct VramInfo {
    _bpp: u16,
    _screen_x: u16,
    _screen_y: u16,
    _ptr: u64,
}

impl VramInfo {
    fn new_from_gop(gop: &mut gop::GraphicsOutput) -> Self {
        let (screen_x, screen_y) = gop.current_mode_info().resolution();

        Self {
            _bpp: 32,
            _screen_x: screen_x as u16,
            _screen_y: screen_y as u16,
            _ptr: gop.frame_buffer().as_mut_ptr() as u64,
        }
    }
}

fn is_usable_gop_mode(mode: &gop::ModeInfo) -> bool {
    if mode.pixel_format() != PixelFormat::BGR {
        return false;
    }

    // According to UEFI Specification 2.8 Errata A, P.479,
    // . : Pixel
    // P : Padding
    // ..........................................PPPPPPPPPP
    // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^|^^^^^^^^^^
    //             HorizontalResolution         | Paddings
    // ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //                    PixelsPerScanLine
    //
    // This OS doesn't deal with pixel paddings, so return an error if pixel paddings exist.
    let (width, _) = mode.resolution();
    if width != mode.stride() {
        return false;
    }

    true
}

fn get_the_maximum_resolution_and_mode(gop: &gop::GraphicsOutput) -> (usize, usize, gop::Mode) {
    let mut max_height = 0;
    let mut max_width = 0;
    let mut preferred_mode = MaybeUninit::<gop::Mode>::uninit();

    for mode in gop.modes() {
        let mode = mode.expect("Failed to get gop mode.");

        let (width, height) = mode.info().resolution();
        if height > max_height && width > max_width && is_usable_gop_mode(&mode.info()) {
            max_height = height;
            max_width = width;
            unsafe { preferred_mode.as_mut_ptr().write(mode) }
        }
    }

    (max_height, max_width, unsafe {
        preferred_mode.assume_init()
    })
}

fn set_resolution(gop: &mut gop::GraphicsOutput) -> () {
    let (width, height, mode) = get_the_maximum_resolution_and_mode(gop);

    gop.set_mode(&mode)
        .expect_success("Failed to set resolution.");

    info!("width: {} height: {}", width, height);
}

fn fetch_gop<'a>(system_table: &'a SystemTable<Boot>) -> &'a mut gop::GraphicsOutput<'a> {
    let gop = system_table
        .boot_services()
        .locate_protocol::<gop::GraphicsOutput>()
        .expect_success("Your computer does not support Graphics Output Protocol!");

    unsafe { &mut *gop.get() }
}

pub fn init(system_table: &SystemTable<Boot>) -> VramInfo {
    let gop = fetch_gop(system_table);
    set_resolution(gop);

    VramInfo::new_from_gop(gop)
}