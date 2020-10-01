// SPDX-License-Identifier: GPL-3.0-or-later

mod event_ring;
mod register;
mod transfer_ring;

use {
    super::config::{
        self, bar,
        extended_capability::{msi_x, CapabilitySpec},
        type_spec::TypeSpec,
    },
    crate::mem::paging::pml4::PML4,
    core::convert::TryFrom,
    register::{
        hc_capability_registers::HCCapabilityRegisters,
        hc_operational_registers::HCOperationalRegisters,
        runtime_base_registers::RuntimeBaseRegisters,
        usb_legacy_support_capability::UsbLegacySupportCapability,
    },
    transfer_ring::{
        transfer_request_block::{Command, Event},
        RingQueue,
    },
    x86_64::{structures::paging::MapperAllSizes, VirtAddr},
};

pub struct Xhci<'a> {
    usb_legacy_support_capability: UsbLegacySupportCapability<'a>,
    hc_capability_registers: HCCapabilityRegisters<'a>,
    hc_operational_registers: HCOperationalRegisters<'a>,
    dcbaa: DeviceContextBaseAddressArray,
    command_ring: RingQueue<'a, Command>,
    event_ring: RingQueue<'a, Event>,
    runtime_base_registers: RuntimeBaseRegisters<'a>,
    event_ring_segment_table: event_ring::SegmentTable<'a>,
    config_space: config::Space,
}

impl<'a> Xhci<'a> {
    pub fn init(&mut self) {
        self.get_ownership_from_bios();
        self.wait_until_controller_is_ready();
        self.set_num_of_enabled_slots();
        self.set_dcbaap();
        self.set_command_ring_pointer();
        self.init_msi_x_table();
        self.set_event_ring_dequeue_pointer();
        self.enable_msi_x_interrupt();
        self.enable_system_bus_interrupt_generation();
        self.init_event_ring_segment_table();
        self.run();
    }

    fn get_ownership_from_bios(&mut self) {
        info!("Getting ownership from BIOS...");

        let usb_leg_sup = &mut self.usb_legacy_support_capability.usb_leg_sup;

        usb_leg_sup.request_hc_ownership(true);

        while {
            let bios_owns = usb_leg_sup.bios_owns_hc();
            let os_owns = usb_leg_sup.os_owns_hc();

            !os_owns || bios_owns
        } {}
    }

    fn wait_until_controller_is_ready(&self) {
        info!("Waiting until controller is ready...");
        while self.hc_operational_registers.usb_sts.controller_not_ready() {}
        info!("Controller is ready");
    }

    fn set_num_of_enabled_slots(&mut self) {
        info!("Setting the number of slots...");
        let num_of_slots = self
            .hc_capability_registers
            .hcs_params_1
            .number_of_device_slots();

        self.hc_operational_registers
            .config
            .set_max_device_slots_enabled(num_of_slots);
    }

    fn set_dcbaap(&mut self) {
        info!("Set DCBAAP...");
        let phys_addr_of_dcbaa = PML4
            .lock()
            .translate_addr(VirtAddr::new(&self.dcbaa as *const _ as u64))
            .expect("Failed to fetch the physical address of DCBAA");

        self.hc_operational_registers
            .dcbaap
            .set_ptr(phys_addr_of_dcbaa);
    }

    fn set_command_ring_pointer(&mut self) {
        let virt_addr = self.command_ring.addr();
        let phys_addr = PML4.lock().translate_addr(virt_addr).unwrap();

        self.hc_operational_registers.crcr.set_ptr(phys_addr);
    }

    fn init_msi_x_table(&mut self) {
        let bar_index = self.get_bir();
        let base_address = self.config_space.base_address(bar_index);
        self.handle_msi_x(|msi_x| {
            let mut table = msi_x.table(base_address);
            let local_apic_id = unsafe { *(0xfee0_0020 as *const u32) >> 24 };
            table[0]
                .message_address()
                .set_destination_id(u8::try_from(local_apic_id).unwrap());
            table[0].message_address().set_redirection_hint(true);
            table[0].message_data().set_level_trigger();
            table[0].message_data().set_vector(0x40);
            table[0].set_mask(false);
        })
    }

    fn init_event_ring_segment_table(&mut self) {
        let ring_addr = self.event_ring.addr().as_u64();
        self.event_ring_segment_table.edit(|table| {
            table[0].set_base_address(ring_addr);
            table[0].set_segment_size(16);
        });

        self.set_event_ring_segment_table_size();
        self.set_event_ring_segment_table_address();
    }

    fn set_event_ring_segment_table_size(&mut self) {
        self.runtime_base_registers.erst_sz.set(1)
    }

    fn set_event_ring_segment_table_address(&mut self) {
        self.runtime_base_registers
            .erst_ba
            .set(self.event_ring_segment_table.address())
    }

    fn set_event_ring_dequeue_pointer(&mut self) {
        self.runtime_base_registers
            .erd_p
            .set_address(self.event_ring_segment_table.address())
    }

    fn enable_msi_x_interrupt(&mut self) {
        self.handle_msi_x(|msi_x| {
            msi_x.enable_interrupt();
        })
    }

    fn enable_system_bus_interrupt_generation(&mut self) {
        self.hc_operational_registers
            .usb_cmd
            .set_interrupt_enable(true)
    }

    fn get_bir(&mut self) -> bar::Index {
        self.handle_msi_x(|msi_x| msi_x.bir())
    }

    fn handle_msi_x<T, U>(&mut self, f: T) -> U
    where
        T: Fn(msi_x::CapabilitySpec) -> U,
    {
        let capability_iter = self.config_space.iter_capability_registers();
        for capability in capability_iter {
            let capability_spec = capability.capability_spec();
            if let Some(CapabilitySpec::MsiX(msi_x)) = capability_spec {
                return f(msi_x);
            }
        }

        unreachable!()
    }

    fn run(&mut self) {
        self.hc_operational_registers.usb_cmd.set_run_stop(true)
    }

    fn new(config_space: config::Space) -> Result<Self, Error> {
        if config_space.is_xhci() {
            Ok(Self::generate(config_space))
        } else {
            Err(Error::NotXhciDevice)
        }
    }

    fn generate(config_space: config::Space) -> Self {
        info!("xHC found.");

        let TypeSpec::NonBridge(non_bridge) = config_space.type_spec();
        let mmio_base = non_bridge.base_addr(bar::Index::new(0));

        info!("Getting HCCapabilityRegisters...");
        let mut hc_capability_registers = HCCapabilityRegisters::new(mmio_base);

        info!("Getting UsbLegacySupportCapability...");
        let usb_legacy_support_capability =
            UsbLegacySupportCapability::new(mmio_base, &hc_capability_registers);

        info!("Getting HCOperationalRegisters...");
        let hc_operational_registers =
            HCOperationalRegisters::new(mmio_base, &mut hc_capability_registers.cap_length);

        info!("Getting DCBAA...");
        let dcbaa = DeviceContextBaseAddressArray::new();

        let runtime_base_registers =
            RuntimeBaseRegisters::new(mmio_base, hc_capability_registers.rts_off.get() as usize);

        let event_ring_segment_table = event_ring::SegmentTable::new();

        Self {
            usb_legacy_support_capability,
            hc_capability_registers,
            hc_operational_registers,
            dcbaa,
            command_ring: RingQueue::new(),
            config_space,
            event_ring: RingQueue::new(),
            runtime_base_registers,
            event_ring_segment_table,
        }
    }
}

const MAX_DEVICE_SLOT: usize = 255;

struct DeviceContextBaseAddressArray([usize; MAX_DEVICE_SLOT]);

impl DeviceContextBaseAddressArray {
    fn new() -> Self {
        Self([0; MAX_DEVICE_SLOT])
    }
}

#[derive(Debug)]
enum Error {
    NotXhciDevice,
}

pub fn iter_devices<'a>() -> impl Iterator<Item = Xhci<'a>> {
    super::iter_devices().filter_map(|device| {
        if device.is_xhci() {
            Xhci::new(device).ok()
        } else {
            None
        }
    })
}
