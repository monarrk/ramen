// SPDX-License-Identifier: GPL-3.0-or-later

pub mod bar;
mod common;
pub mod extended_capability;
pub mod type_spec;

use {
    self::common::Common,
    alloc::boxed::Box,
    bar::Bar,
    core::{
        convert::{From, TryFrom},
        iter,
        ops::Add,
    },
    extended_capability::ExtendedCapability,
    type_spec::TypeSpec,
    x86_64::{
        instructions::port::{Port, PortWriteOnly},
        PhysAddr,
    },
};

#[derive(Debug)]
pub struct Space {
    registers: Registers,
}

impl Space {
    pub fn new(bus: Bus, device: Device) -> Option<Self> {
        Some(Self {
            registers: Registers::new(bus, device)?,
        })
    }

    pub fn is_xhci(&self) -> bool {
        self.common().is_xhci()
    }

    pub fn is_pci(&self) -> bool {
        self.common().is_ahci()
    }

    pub fn base_address(&self, index: bar::Index) -> PhysAddr {
        self.type_spec().base_address(index)
    }

    pub fn init_msi_for_xhci(&self) -> Result<(), Error> {
        if self.is_xhci() {
            self.init_msi_or_msi_x();
            Ok(())
        } else {
            Err(Error::NotXhciDevice)
        }
    }

    fn iter_capability_registers<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = ExtendedCapability> + 'a> {
        let capability_pointer = CapabilityPointer::new(&self.registers, &self.common());
        match capability_pointer {
            None => Box::new(iter::empty()),
            Some(capability_pointer) => Box::new(extended_capability::Iter::new(
                &self.registers,
                RegisterIndex::from(capability_pointer),
            )),
        }
    }

    fn init_msi_or_msi_x(&self) {
        for capability in self.iter_capability_registers() {
            if capability.init_for_xhci(&self.type_spec()).is_ok() {
                return;
            }
        }

        unreachable!()
    }

    fn type_spec(&self) -> TypeSpec {
        TypeSpec::new(&self.registers, &self.common())
    }

    fn common(&self) -> Common {
        Common::new(&self.registers)
    }
}

#[derive(Debug)]
pub enum Error {
    NotXhciDevice,
}

#[derive(Debug)]
pub struct Registers {
    bus: Bus,
    device: Device,
}
impl Registers {
    fn new(bus: Bus, device: Device) -> Option<Self> {
        if Self::valid(bus, device) {
            Some(Self { bus, device })
        } else {
            None
        }
    }

    fn valid(bus: Bus, device: Device) -> bool {
        let config_addr = ConfigAddress::new(bus, device, Function::zero(), RegisterIndex::zero());
        let id = unsafe { config_addr.read() };

        id != !0
    }

    fn get(&self, index: RegisterIndex) -> u32 {
        let accessor = ConfigAddress::new(self.bus, self.device, Function::zero(), index);
        unsafe { accessor.read() }
    }

    fn set(&self, index: RegisterIndex, value: u32) {
        let accessor = ConfigAddress::new(self.bus, self.device, Function::zero(), index);
        unsafe { accessor.write(value) }
    }
}

struct ConfigAddress {
    bus: Bus,
    device: Device,
    function: Function,
    register: RegisterIndex,
}

impl ConfigAddress {
    const PORT_CONFIG_ADDR: PortWriteOnly<u32> = PortWriteOnly::new(0xcf8);
    const PORT_CONFIG_DATA: Port<u32> = Port::new(0xcfc);

    #[allow(clippy::too_many_arguments)]
    fn new(bus: Bus, device: Device, function: Function, register: RegisterIndex) -> Self {
        Self {
            bus,
            device,
            function,
            register,
        }
    }

    fn as_u32(&self) -> u32 {
        const VALID: u32 = 0x8000_0000;
        let bus = self.bus.as_u32();
        let device = self.device.as_u32();
        let function = self.function.as_u32();
        let register = u32::try_from(self.register.as_usize()).unwrap();

        VALID | bus << 16 | device << 11 | function << 8 | register << 2
    }

    /// Safety: `self` must contain the valid config address.
    unsafe fn read(&self) -> u32 {
        let mut addr = Self::PORT_CONFIG_ADDR;
        addr.write(self.as_u32());

        let mut data = Self::PORT_CONFIG_DATA;
        data.read()
    }

    unsafe fn write(&self, value: u32) {
        let mut addr = Self::PORT_CONFIG_ADDR;
        addr.write(self.as_u32());

        let mut data = Self::PORT_CONFIG_DATA;
        data.write(value);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Bus(u32);
impl Bus {
    pub const MAX: u32 = 256;
    pub fn new(bus: u32) -> Self {
        assert!(bus < Self::MAX);
        Self(bus)
    }

    fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Device(u32);
impl Device {
    pub const MAX: u32 = 32;
    pub fn new(device: u32) -> Self {
        assert!(device < Self::MAX);
        Self(device)
    }

    fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Copy, Clone)]
pub struct Function(u32);
impl Function {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RegisterIndex(usize);
impl RegisterIndex {
    const MAX: usize = 64;
    pub fn new(offset: usize) -> Self {
        assert!(offset < Self::MAX, "Too large register index: {}", offset);
        Self(offset)
    }

    fn zero() -> Self {
        Self(0)
    }

    fn as_usize(self) -> usize {
        self.0
    }

    fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl Add<usize> for RegisterIndex {
    type Output = RegisterIndex;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CapabilityPointer<'a> {
    registers: &'a Registers,
}
impl<'a> CapabilityPointer<'a> {
    pub fn new(registers: &'a Registers, common: &Common) -> Option<Self> {
        if common.has_capability_ptr() {
            Some(Self { registers })
        } else {
            None
        }
    }
}
impl<'a> From<CapabilityPointer<'a>> for RegisterIndex {
    fn from(capability_pointer: CapabilityPointer) -> Self {
        let pointer =
            usize::try_from(capability_pointer.registers.get(RegisterIndex::new(0x0d)) & 0xff)
                .unwrap();
        RegisterIndex::new(pointer >> 2)
    }
}
