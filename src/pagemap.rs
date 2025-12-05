use crate::idlemap::IdleMap;
use memmap2::Mmap;
use std::fs::File;
use std::io;

pub const PAGEMAP_ENTRY_SIZE: usize = 8;
const PFN_MASK: u64 = 0x7FFFFFFFFFFFFF; // Bits 0-54
const PRESENT_MASK: u64 = 1 << 63;

pub struct Pagemap {
    mmap: Mmap,
    _pid: i32,
}

impl Pagemap {
    pub fn new(pid: i32) -> io::Result<Self> {
        let path = format!("/proc/{}/pagemap", pid);
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Pagemap { mmap, _pid: pid })
    }

    /// Processes a memory region and counts active pages.
    /// Returns (active_pages, walked_pages).
    pub fn process_region(
        &self,
        start_addr: u64,
        end_addr: u64,
        idle_map: &IdleMap,
    ) -> io::Result<(usize, usize)> {
        let page_size = 4096; // Assuming 4KB pages for now, as per C code assumption
        let num_pages = (end_addr - start_addr) / page_size;

        // Calculate offset in pagemap
        let offset = (start_addr / page_size * PAGEMAP_ENTRY_SIZE as u64) as usize;

        let mut active_pages = 0;
        let mut walked_pages = 0;

        for i in 0..num_pages as usize {
            let entry_offset = offset + i * PAGEMAP_ENTRY_SIZE;
            
            // Check bounds
            if entry_offset + PAGEMAP_ENTRY_SIZE > self.mmap.len() {
                break;
            }

            let entry_bytes = &self.mmap[entry_offset..entry_offset + PAGEMAP_ENTRY_SIZE];
            let entry = u64::from_ne_bytes(entry_bytes.try_into().unwrap());

            // Check if page is present
            if (entry & PRESENT_MASK) == 0 {
                continue;
            }

            let pfn = entry & PFN_MASK;
            if pfn == 0 {
                continue;
            }

            if idle_map.is_page_active(pfn) {
                active_pages += 1;
            }
            walked_pages += 1;
        }

        Ok((active_pages, walked_pages))
    }
}
