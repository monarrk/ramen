use crate::common_items::constant::*;
use crate::common_items::size::{Byte, Size};
use crate::x86_64::addr::{PhysAddr, VirtAddr};
use core::ptr;
use uefi::table::boot;
use uefi::table::boot::MemoryType;

struct PageMapInfo {
    virt: VirtAddr,
    phys: PhysAddr,
    bytes: Size<Byte>,
}

impl PageMapInfo {
    fn new(virt: VirtAddr, phys: PhysAddr, bytes: Size<Byte>) -> Self {
        Self { virt, phys, bytes }
    }

    fn map(&self, mem_map: &mut [boot::MemoryDescriptor]) -> () {
        map_virt_to_phys(self.virt, self.phys, self.bytes, mem_map);
    }
}

pub fn init(mem_map: &mut [boot::MemoryDescriptor], vram: &common_items::VramInfo) -> () {
    remove_table_protection();

    let map_info = [
        PageMapInfo::new(
            KERNEL_ADDR,
            PhysAddr::new(0x0020_0000),
            Size::new((512 + 4 + 128) * 1024),
        ),
        PageMapInfo::new(VRAM_ADDR, vram.phys_ptr(), vram.bytes()),
    ];

    for info in &map_info {
        info.map(mem_map);
    }
}

fn remove_table_protection() -> () {
    unsafe {
        asm!(
            "mov rax, cr0
        and eax, 0xfffeffff
        mov cr0, rax"
        )
    }
}

fn map_virt_to_phys(
    virt: VirtAddr,
    phys: PhysAddr,
    bytes: Size<Byte>,
    mem_map: &mut [boot::MemoryDescriptor],
) -> () {
    let num_of_pages = bytes.as_num_of_pages().as_usize();

    for i in 0..num_of_pages {
        virt_points_phys(virt + BYTES_OF_PAGE * i, phys + BYTES_OF_PAGE * i, mem_map);
    }
}

fn virt_points_phys(virt: VirtAddr, phys: PhysAddr, mem_map: &mut [boot::MemoryDescriptor]) -> () {
    virt_points_phys_recur(virt, phys, get_pml4_addr(), mem_map, TableType::Pml4);
}

fn get_pml4_addr() -> PhysAddr {
    let addr;
    unsafe {
        asm!("mov rax, cr3",out("rax") addr,options(nomem, preserves_flags, nostack));
    }

    PhysAddr::new(addr)
}

fn virt_points_phys_recur(
    virt: VirtAddr,
    phys: PhysAddr,
    table_addr: PhysAddr,
    mem_map: &mut [boot::MemoryDescriptor],
    table: TableType,
) -> () {
    let ptr_to_entry = ptr_to_entry(virt, table_addr, table);

    if let TableType::Pt = table {
        return unsafe { ptr::write(ptr_to_entry, phys.as_u64() as usize | PAGE_EXISTS) };
    }

    let mut entry = unsafe { ptr::read(ptr_to_entry) };

    if !entry_exists(entry) {
        entry = create_table(mem_map).as_u64() as usize | PAGE_EXISTS;
        unsafe { ptr::write(ptr_to_entry, entry) }
    }

    virt_points_phys_recur(
        virt,
        phys,
        get_addr_from_table_entry(entry),
        mem_map,
        table.next_table().unwrap(), // `table` can't be `Pt`. This `unwrap` always succeeds.
    )
}

fn get_offset_of_entry(virt_addr: VirtAddr, table: TableType) -> usize {
    (virt_addr.as_u64()
        >> match table {
            TableType::Pml4 => 39,
            TableType::Pdpt => 30,
            TableType::Pd => 21,
            TableType::Pt => 12,
        }
        & 0x1ff) as usize
        * TABLE_ENTRY_SIZE
}

fn ptr_to_entry(virt: VirtAddr, table_addr: PhysAddr, table: TableType) -> *mut usize {
    (table_addr + get_offset_of_entry(virt, table)).as_u64() as _
}

fn entry_exists(entry: usize) -> bool {
    entry & PAGE_EXISTS == 1
}

fn create_table(mem_map: &mut [boot::MemoryDescriptor]) -> PhysAddr {
    let addr = allocate_page_for_page_table(mem_map);
    unsafe { initialize_page_table(addr) }

    addr
}

fn get_addr_from_table_entry(entry: usize) -> PhysAddr {
    PhysAddr::new(entry as u64 & 0xffff_ffff_ffff_f000)
}

fn allocate_page_for_page_table(mem_map: &mut [boot::MemoryDescriptor]) -> PhysAddr {
    for descriptor in mem_map.iter_mut() {
        if descriptor.ty == MemoryType::CONVENTIONAL && descriptor.page_count > 0 {
            let addr = descriptor.phys_start;
            descriptor.phys_start += BYTES_OF_PAGE as u64;
            descriptor.page_count -= 1;

            return PhysAddr::new(addr as _);
        }
    }

    // Shouldn't reach here.
    panic!("Failed to allocate memory for a page table.");
}

unsafe fn initialize_page_table(table_addr: PhysAddr) -> () {
    ptr::write_bytes(table_addr.as_u64() as *mut u8, 0, BYTES_OF_PAGE)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum TableType {
    Pml4,
    Pdpt,
    Pd,
    Pt,
}

impl TableType {
    fn next_table(&self) -> Option<TableType> {
        match self {
            TableType::Pt => None,
            TableType::Pd => Some(TableType::Pt),
            TableType::Pdpt => Some(TableType::Pd),
            TableType::Pml4 => Some(TableType::Pdpt),
        }
    }
}

const TABLE_ENTRY_SIZE: usize = 8;

const PAGE_EXISTS: usize = 1;
const BYTES_OF_PAGE: usize = 0x1000;
