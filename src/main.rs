use clap::Parser;
use std::process;
use std::thread;
use std::time::{Duration, Instant};

mod idlemap;
mod pagemap;
mod scanner;

use idlemap::IdleMap;
use pagemap::Pagemap;
use scanner::Scanner;

// C code uses 0xffff880000000000LLU.
// In Rust, we can just use a high address check.
// User space usually ends much lower.
const PAGE_OFFSET: u64 = 0xffff880000000000;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Process ID to watch
    pid: i32,

    /// Duration in seconds to measure
    duration: f64,
}

fn main() {
    let args = Args::parse();

    if args.duration < 0.01 {
        eprintln!("Interval too short. Exiting.");
        process::exit(1);
    }

    println!(
        "Watching PID {} page references during {:.2} seconds...",
        args.pid, args.duration
    );

    // 1. Set idle flags
    let ts1 = Instant::now();
    if let Err(e) = IdleMap::set_idlemap() {
        eprintln!("Failed to set idlemap: {}", e);
        // Continue? C code exits on some errors but not all.
        // If we can't write idlemap, we can't reset tracking.
        process::exit(1);
    }
    let set_duration = ts1.elapsed();

    // 2. Sleep
    let sleep_duration = Duration::from_secs_f64(args.duration);
    thread::sleep(sleep_duration);
    let ts3 = Instant::now(); // Time after sleep

    // 3. Read idle flags
    // In C code: loadidlemap();
    let idle_map = match IdleMap::load() {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Failed to load idlemap: {}", e);
            process::exit(1);
        }
    };

    // 4. Walk maps
    let scanner = Scanner::new(args.pid);
    let regions = match scanner.get_maps() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to read maps for PID {}: {}", args.pid, e);
            process::exit(1);
        }
    };

    let mut pagemap = match Pagemap::new(args.pid) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to open pagemap for PID {}: {}", args.pid, e);
            process::exit(1);
        }
    };

    let mut total_active_pages = 0;
    let mut _total_walked_pages = 0;

    for region in regions {
        if region.start > PAGE_OFFSET {
            continue;
        }

        match pagemap.process_region(region.start, region.end, &idle_map) {
            Ok((active, walked)) => {
                total_active_pages += active;
                _total_walked_pages += walked;
            }
            Err(e) => {
                eprintln!(
                    "Error processing region {:x}-{:x}: {}",
                    region.start, region.end, e
                );
            }
        }
    }

    let ts4 = Instant::now();

    // Calculate times
    // C code: est_us = dur_us - (set_us / 2) - (read_us / 2);
    // dur_us = ts4 - ts1
    // set_us = ts2 - ts1 (we didn't measure ts2 explicitly but set_duration covers it)
    // read_us = ts4 - ts3 (includes loadidlemap + walkmaps)

    let total_duration = ts4.duration_since(ts1);
    let read_walk_duration = ts4.duration_since(ts3);

    // Estimated duration calculation from C code logic
    // est = total - (set / 2) - (read_walk / 2)
    let est_micros = total_duration.as_micros() as i64
        - (set_duration.as_micros() as i64 / 2)
        - (read_walk_duration.as_micros() as i64 / 2);

    let est_seconds = est_micros as f64 / 1_000_000.0;

    let page_size = 4096; // 4KB
    let ref_mb = (total_active_pages as f64 * page_size as f64) / (1024.0 * 1024.0);

    println!("{:<7} {:>10}", "Est(s)", "Ref(MB)");
    println!("{:<7.3} {:>10.2}", est_seconds, ref_mb);
}
