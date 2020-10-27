// SPDX-License-Identifier: GPL-3.0-or-later

use {
    super::{trb, CycleBit},
    crate::mem::allocator::page_box::PageBox,
    core::ops::{Index, IndexMut},
    x86_64::PhysAddr,
};

pub struct Ring(PageBox<[Trb]>);
impl Ring {
    pub fn new(num_trb: usize) -> Self {
        Self(PageBox::new_slice(num_trb))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn phys_addr(&self) -> PhysAddr {
        self.0.phys_addr()
    }
}
impl Index<usize> for Ring {
    type Output = Trb;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl IndexMut<usize> for Ring {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Trb(pub u128);
impl Trb {
    pub fn cycle_bit(self) -> CycleBit {
        self.into()
    }
}
impl From<trb::Trb> for Trb {
    fn from(trb: trb::Trb) -> Self {
        match trb {
            trb::Trb::Noop(noop) => Self(noop.0),
        }
    }
}
impl From<Trb> for CycleBit {
    fn from(raw: Trb) -> Self {
        Self((raw.0 >> 96) & 1 != 0)
    }
}
