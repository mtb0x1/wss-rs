use crate::idlemap::IdleMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

pub const PAGEMAP_ENTRY_SIZE: usize = 8;
const PFN_MASK: u64 = 0x7FFFFFFFFFFFFF; // Bits 0-54
const PRESENT_MASK: u64 = 1 << 63;

pub struct Pagemap {
    file: File,
    _pid: i32,
}

impl Pagemap {
    pub fn new(pid: i32) -> io::Result<Self> {
        let path = format!("/proc/{}/pagemap", pid);
        let file = File::open(path)?;
        Ok(Pagemap { file, _pid: pid })
    }

    /// Processes a memory region and counts active pages.
    /// Returns (active_pages, walked_pages).
    pub fn process_region(
        &mut self,
        start_addr: u64,
        end_addr: u64,
        idle_map: &IdleMap,
    ) -> io::Result<(usize, usize)> {
        let page_size = 4096; // Assuming 4KB pages for now, as per C code assumption
        let num_pages = (end_addr - start_addr) / page_size;

        // Seek to the correct offset in pagemap
        let offset = start_addr / page_size * PAGEMAP_ENTRY_SIZE as u64;
        self.file.seek(SeekFrom::Start(offset))?;

        // Read pagemap entries in chunks
        // C code uses PAGEMAP_CHUNK_SIZE * (mapend - mapstart) / pagesize
        // But it reads in one go if possible.
        // Let's read in reasonable chunks to avoid massive allocations for huge regions.
        const CHUNK_SIZE: usize = 1024; // Read 1024 entries at a time
        let mut buffer = [0u8; CHUNK_SIZE * PAGEMAP_ENTRY_SIZE];

        let mut active_pages = 0;
        let mut walked_pages = 0;
        let mut pages_processed = 0;

        while pages_processed < num_pages {
            let pages_to_read =
                std::cmp::min(CHUNK_SIZE as u64, num_pages - pages_processed) as usize;
            let bytes_to_read = pages_to_read * PAGEMAP_ENTRY_SIZE;

            self.file.read_exact(&mut buffer[..bytes_to_read])?;

            for i in 0..pages_to_read {
                let entry_bytes = &buffer[i * PAGEMAP_ENTRY_SIZE..(i + 1) * PAGEMAP_ENTRY_SIZE];
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

            pages_processed += pages_to_read as u64;
        }

        Ok((active_pages, walked_pages))
    }
}
