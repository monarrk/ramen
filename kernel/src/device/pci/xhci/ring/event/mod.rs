// SPDX-License-Identifier: GPL-3.0-or-later

use super::{trb::Trb, CycleBit, Raw};

mod segment_table;

struct EventRing<'a> {
    raw: Raw<'a>,
    current_cycle_bit: CycleBit,
    dequeue_ptr: usize,
}
impl<'a> EventRing<'a> {
    fn new(len: usize) -> Self {
        Self {
            raw: Raw::new(len),
            current_cycle_bit: CycleBit::new(true),
            dequeue_ptr: 0,
        }
    }

    fn increment(&mut self) {
        self.dequeue_ptr += 1;
        if self.dequeue_ptr >= self.len() {
            self.dequeue_ptr %= self.len();
            self.current_cycle_bit.toggle();
        }
    }

    fn len(&self) -> usize {
        self.raw.len()
    }
}
