// SPDX-License-Identifier: GPL-3.0-or-later

use {
    crate::{
        device::pci::xhci::register::hc_capability_registers::HCCapabilityRegisters,
        mem::accessor::Accessor,
    },
    bitfield::bitfield,
    os_units::Bytes,
    x86_64::PhysAddr,
};

pub struct HCOperationalRegisters {
    usb_cmd: Accessor<UsbCommandRegister>,
    usb_sts: Accessor<UsbStatusRegister>,
    crcr: Accessor<CommandRingControlRegister>,
    dcbaap: Accessor<DeviceContextBaseAddressArrayPointer>,
    config: Accessor<ConfigureRegister>,
    port_sc: Accessor<[PortStatusAndControlRegister]>,
}

impl HCOperationalRegisters {
    pub fn new(mmio_base: PhysAddr, capabilities: &HCCapabilityRegisters) -> Self {
        let operational_base = mmio_base + capabilities.len();

        let usb_cmd = Accessor::new(operational_base, Bytes::new(0x00));
        let usb_sts = Accessor::new(operational_base, Bytes::new(0x04));
        let crcr = Accessor::new(operational_base, Bytes::new(0x18));
        let dcbaap = Accessor::new(operational_base, Bytes::new(0x30));
        let config = Accessor::new(operational_base, Bytes::new(0x38));
        let port_sc = Accessor::new_slice(operational_base, Bytes::new(0x400), 10);

        Self {
            usb_cmd,
            usb_sts,
            crcr,
            dcbaap,
            config,
            port_sc,
        }
    }

    pub fn reset_hc(&mut self) {
        if self.usb_sts.hc_halted() {
            return;
        }
        self.usb_cmd.reset();
    }

    pub fn wait_until_hc_is_ready(&self) {
        self.usb_sts.wait_until_hc_is_ready();
    }

    pub fn set_num_of_device_slots(&mut self, num: u32) {
        self.config.set_max_device_slots_enabled(num)
    }

    pub fn set_dcbaa_ptr(&mut self, addr: PhysAddr) {
        self.dcbaap.set_ptr(addr)
    }

    pub fn set_command_ring_ptr(&mut self, addr: PhysAddr) {
        self.crcr.set_ptr(addr)
    }

    pub fn enable_interrupt(&mut self) {
        self.usb_cmd.set_interrupt_enable(true)
    }

    pub fn run(&mut self) {
        self.usb_cmd.set_run_stop(true);
        while self.usb_sts.hc_halted() {}
    }
}

bitfield! {
    #[repr(transparent)]
    struct UsbCommandRegister(u32);

    run_stop,set_run_stop: 0;
    hc_reset,set_hc_reset: 1;
    interrupt_enable,set_interrupt_enable: 2;
}
impl UsbCommandRegister {
    fn reset(&mut self) {
        self.set_hc_reset(true);
        self.wait_until_hc_is_reset();
    }

    fn wait_until_hc_is_reset(&self) {
        while self.hc_reset() {}
    }
}

bitfield! {
    #[repr(transparent)]
    struct UsbStatusRegister(u32);

    hc_halted, _: 0;
    controller_not_ready,_:11;
}
impl UsbStatusRegister {
    fn wait_until_hc_is_ready(&self) {
        while self.controller_not_ready() {}
    }
}

bitfield! {
    #[repr(transparent)]
    struct CommandRingControlRegister(u64);

    ptr,set_pointer:63,6;
}

impl CommandRingControlRegister {
    fn set_ptr(&mut self, ptr: PhysAddr) {
        let ptr = ptr.as_u64() >> 6;

        self.set_pointer(ptr);
    }
}
#[repr(transparent)]
struct DeviceContextBaseAddressArrayPointer(u64);

impl DeviceContextBaseAddressArrayPointer {
    fn set_ptr(&mut self, ptr: PhysAddr) {
        assert!(
            ptr.as_u64().trailing_zeros() >= 6,
            "Wrong address: {:?}",
            ptr
        );

        self.0 = ptr.as_u64();
    }
}

bitfield! {
    #[repr(transparent)]
     struct ConfigureRegister(u32);

    max_device_slots_enabled,set_max_device_slots_enabled:7,0;
}

bitfield! {
    #[repr(transparent)]
     struct PortStatusAndControlRegister(u32);

     current_connect_status, _: 0;
     port_enabled_disabled, _: 1;
     port_reset, _: 4;
     port_power, _: 9;
}

impl PortStatusAndControlRegister {
    fn disconnected(&self) -> bool {
        self.port_power()
            && !self.current_connect_status()
            && !self.port_enabled_disabled()
            && !self.port_reset()
    }
}
