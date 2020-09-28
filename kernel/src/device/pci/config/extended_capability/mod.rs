// SPDX-License-Identifier: GPL-3.0-or-later

mod msi_x;

use {
    super::{Common, Offset, RegisterIndex, Registers, TypeSpec},
    alloc::vec::Vec,
};

#[derive(Debug)]
pub struct ExtendedCapabilities<'a>(Vec<ExtendedCapability<'a>>);

impl<'a> ExtendedCapabilities<'a> {
    pub fn new(raw: &Registers, common: &Common, type_spec: &TypeSpec) -> Option<Self> {
        let mut base = Self::parse_raw_to_get_capability_ptr(raw, common)?;
        let mut capabilities = Vec::new();

        while {
            let extended_capability = ExtendedCapability::new(&raw, base, type_spec);
            base = extended_capability.next_ptr();
            info!("Extended Capability: {:?}", extended_capability);
            capabilities.push(extended_capability);

            !base.is_null()
        } {}

        Some(Self(capabilities))
    }

    fn parse_raw_to_get_capability_ptr(raw: &Registers, common: &Common) -> Option<RegisterIndex> {
        if common.has_capability_ptr() {
            Some(
                Offset::new((raw.get(RegisterIndex::new(0x0d)) & 0xfc) as usize)
                    .as_register_index(),
            )
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct ExtendedCapability<'a> {
    id: Id,
    next_ptr: RegisterIndex,
    capability_spec: Option<CapabilitySpec<'a>>,
}

impl<'a> ExtendedCapability<'a> {
    fn new(raw: &Registers, offset: RegisterIndex, type_spec: &TypeSpec) -> Self {
        let id = Id::parse_raw(raw, offset);
        let next_ptr = RegisterIndex::new(((raw.get(offset) >> 8) & 0xff) as usize);
        let capability_spec = CapabilitySpec::new(raw, offset, id, type_spec);

        Self {
            id,
            next_ptr,
            capability_spec,
        }
    }

    fn next_ptr(&self) -> RegisterIndex {
        self.next_ptr
    }
}

#[derive(Debug)]
enum CapabilitySpec<'a> {
    MsiX(msi_x::CapabilitySpec<'a>),
}

impl<'a> CapabilitySpec<'a> {
    fn new(raw: &Registers, offset: RegisterIndex, id: Id, type_spec: &TypeSpec) -> Option<Self> {
        if id.0 == 0x11 {
            Some(Self::MsiX(msi_x::CapabilitySpec::new(
                raw, offset, type_spec,
            )))
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct Id(u8);
impl Id {
    fn parse_raw(raw: &Registers, offset: RegisterIndex) -> Self {
        Self((raw.get(offset) & 0xff) as u8)
    }
}
