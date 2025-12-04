use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

pub const IDLEMAP_PATH: &str = "/sys/kernel/mm/page_idle/bitmap";
// 8 bytes per chunk (64 bits)

// Buffer size for setting idle map (same as in C version)
const IDLEMAP_BUF_SIZE: usize = 4096;

pub struct IdleMap {
    pub data: Vec<u8>,
}

impl IdleMap {
    /// Sets the entire idlemap flags to 1 (idle).
    /// This resets the idle tracking.
    pub fn set_idlemap() -> io::Result<()> {
        let path = Path::new(IDLEMAP_PATH);
        if !path.exists() {
            // If we are not on Linux or the module is not loaded, this might fail.
            // For development on Windows, we might want to mock or just warn.
            // But strictly following the C code, we try to open it.
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{} not found. Are you on Linux with CONFIG_IDLE_PAGE_TRACKING?",
                    IDLEMAP_PATH
                ),
            ));
        }

        let mut file = OpenOptions::new().write(true).open(path)?;
        let buf = [0xffu8; IDLEMAP_BUF_SIZE];

        // The C code loops writing 0xff until it's done.
        // "only sets user memory bits; kernel is silently ignored"
        // We just write continuously until we can't write anymore or error?
        // The C code does: while (write(idlefd, &buf, sizeof(buf)) > 0) {;}
        // This implies it writes until EOF or error.

        loop {
            match file.write(&buf) {
                Ok(0) => break, // EOF
                Ok(_) => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// Loads the idlemap into memory.
    pub fn load() -> io::Result<Self> {
        let path = Path::new(IDLEMAP_PATH);
        let mut file = File::open(path)?;
        let mut data = Vec::new();

        // The C code allocates MAX_IDLEMAP_SIZE (20MB).
        // We can just read to end.
        file.read_to_end(&mut data)?;

        Ok(IdleMap { data })
    }

    /// Returns the raw bit for a PFN.
    /// The C code does: idlebits = g_idlebuf[idlemapp];
    /// if (!(idlebits & (1ULL << (pfn % 64)))) { active }
    /// My is_idle logic above splits it into bytes.
    /// Let's double check the C logic.
    /// idlemapp = (pfn / 64) * 8; // byte offset of the 64-bit chunk
    /// idlebits = g_idlebuf[idlemapp / 8]; // reading u64
    /// bit check: 1ULL << (pfn % 64)
    ///
    /// In Rust Vec<u8>:
    /// Byte index = pfn / 8.
    /// Bit index in byte = pfn % 8.
    ///
    /// Example: PFN 0.
    /// C: idlemapp = 0. idlebits = buf[0] (first 8 bytes). check bit 0.
    /// Rust: byte_index = 0. byte = data[0]. check bit 0.
    /// Matches.
    pub fn is_page_active(&self, pfn: u64) -> bool {
        let byte_idx = (pfn / 8) as usize;
        if byte_idx >= self.data.len() {
            return false;
        }
        let byte = self.data[byte_idx];
        let bit = pfn % 8;
        (byte & (1 << bit)) == 0 // 0 means accessed (active), 1 means idle
    }
}
