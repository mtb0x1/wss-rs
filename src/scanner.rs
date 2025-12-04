use std::fs::File;
use std::io::{self, BufRead, BufReader};

pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub _perms: String,
    pub _pathname: String,
}

pub struct Scanner {
    pid: i32,
}

impl Scanner {
    pub fn new(pid: i32) -> Self {
        Scanner { pid }
    }

    pub fn get_maps(&self) -> io::Result<Vec<MemoryRegion>> {
        let path = format!("/proc/{}/maps", self.pid);
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut regions = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Some(region) = parse_map_line(&line) {
                // Filter out kernel memory if needed, C code checks > PAGE_OFFSET
                // C code: if (mapstart > PAGE_OFFSET) continue;
                // PAGE_OFFSET is 0xffff880000000000LLU in C code.
                // We can add that check in the main loop or here.
                regions.push(region);
            }
        }

        Ok(regions)
    }
}

fn parse_map_line(line: &str) -> Option<MemoryRegion> {
    // Format: 00400000-0040b000 r-xp 00000000 08:01 123456 /path/to/file
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 1 {
        return None;
    }

    let range_parts: Vec<&str> = parts[0].split('-').collect();
    if range_parts.len() != 2 {
        return None;
    }

    let start = u64::from_str_radix(range_parts[0], 16).ok()?;
    let end = u64::from_str_radix(range_parts[1], 16).ok()?;
    let perms = parts.get(1).unwrap_or(&"").to_string();
    let pathname = parts.get(5).unwrap_or(&"").to_string();

    Some(MemoryRegion {
        start,
        end,
        _perms: perms,
        _pathname: pathname,
    })
}
