use crate::graphics;
use crate::queue;
use crate::x86_64::instructions::port::Port;

extern crate lazy_static;

lazy_static::lazy_static! {
    pub static ref QUEUE:spin::Mutex<queue::Queue> = spin::Mutex::new(queue::Queue::new());
}

struct MouseButtons {
    left: bool,
    center: bool,
    right: bool,
}

impl MouseButtons {
    fn new() -> Self {
        Self {
            left: false,
            right: false,
            center: false,
        }
    }

    fn purse_data(data: u32) -> Self {
        Self {
            left: data & 0x01 != 0,
            right: data & 0x02 != 0,
            center: data & 0x04 != 0,
        }
    }
}

#[derive(PartialEq, Eq)]
enum DevicePhase {
    Init,
    NoData,
    OneData,
    TwoData,
    ThreeData,
}

pub struct Device<'a> {
    data_from_device: [u32; 3],
    phase: DevicePhase,

    speed: graphics::screen::TwoDimensionalVec<i32>,

    buttons: MouseButtons,

    vram: &'a graphics::Vram,
}

impl<'a> Device<'a> {
    pub fn new(vram: &'a graphics::Vram) -> Self {
        Self {
            data_from_device: [0; 3],
            phase: DevicePhase::Init,
            speed: graphics::screen::TwoDimensionalVec::new(0, 0),
            buttons: MouseButtons::new(),
            vram,
        }
    }

    pub fn enable(&self) -> () {
        super::wait_kbc_sendready();
        unsafe { Port::new(super::PORT_KEY_CMD).write(super::KEY_CMD_SEND_TO_MOUSE) };
        super::wait_kbc_sendready();
        unsafe { Port::new(super::PORT_KEYDATA).write(super::MOUSE_CMD_ENABLE) };
    }

    pub fn data_available(&self) -> bool {
        self.phase == DevicePhase::ThreeData
    }

    pub fn put_data(&mut self, data: u32) -> () {
        match self.phase {
            DevicePhase::Init => {
                let is_correct_startup = data == 0xfa;
                if is_correct_startup {
                    self.phase = DevicePhase::NoData
                }
            }

            DevicePhase::NoData => {
                if Self::is_correct_first_byte_from_device(data) {
                    self.data_from_device[0] = data;
                    self.phase = DevicePhase::OneData;
                }
            }
            DevicePhase::OneData => {
                self.data_from_device[1] = data;
                self.phase = DevicePhase::TwoData;
            }
            DevicePhase::TwoData => {
                self.data_from_device[2] = data;
                self.phase = DevicePhase::ThreeData;
            }
            DevicePhase::ThreeData => {}
        }
    }

    // To sync phase, and data sent from mouse device
    fn is_correct_first_byte_from_device(data: u32) -> bool {
        data & 0xC8 == 0x08
    }

    fn clear_stack(&mut self) -> () {
        self.phase = DevicePhase::NoData;
    }

    pub fn purse_data(&mut self) -> () {
        self.buttons = MouseButtons::purse_data(self.data_from_device[0]);
        self.speed.x = self.data_from_device[1] as i32;
        self.speed.y = self.data_from_device[2] as i32;

        if self.data_from_device[0] & 0x10 != 0 {
            self.speed.x = (self.speed.x as u32 | 0xFFFFFF00) as i32;
        }

        if self.data_from_device[0] & 0x20 != 0 {
            self.speed.y = (self.speed.y as u32 | 0xFFFFFF00) as i32;
        }

        self.speed.y = -self.speed.y;

        self.clear_stack();
    }

    pub fn get_speed(&self) -> graphics::screen::Coord<isize> {
        graphics::screen::Coord::new(self.speed.x as isize, self.speed.y as isize)
    }

    pub fn print_buf_data(&mut self) -> () {
        use crate::print_with_pos;
        let mut screen: graphics::screen::Screen = graphics::screen::Screen::new(self.vram);

        screen.draw_rectangle(
            graphics::RGB::new(0x008484),
            graphics::screen::Coord::new(32, 16),
            graphics::screen::Coord::new(32 + 15 * 8 - 1, 31),
        );

        print_with_pos!(
            self.vram,
            graphics::screen::Coord::new(32, 16),
            graphics::RGB::new(0xFFFFFF),
            "[{}{}{} {:4}{:4}]",
            if self.buttons.left { 'L' } else { 'l' },
            if self.buttons.center { 'C' } else { 'c' },
            if self.buttons.right { 'R' } else { 'r' },
            self.speed.x,
            self.speed.y
        );
    }
}