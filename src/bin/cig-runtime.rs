// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! The runtime: `cig`, exactly as `src/main.rs` in CigScript starts it.
//! A program is this binary with its script appended; with none appended,
//! this is `cig` of the version cigcompile pins.
//!
//! Built with `--features alloc-count` (a bench-only build, #30), it counts
//! every allocation and the bytes asked for, and writes the two numbers to
//! the file named by `CIG_ALLOC_FILE` as it exits; `cigcompile bench
//! --alloc` reads them. A release build has none of this.

#[cfg(feature = "alloc-count")]
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    static ALLOCS: AtomicU64 = AtomicU64::new(0);
    static BYTES: AtomicU64 = AtomicU64::new(0);

    /// The system allocator, counting. A `realloc` counts as one allocation
    /// of its new size, so growing a vector shows as the allocations it costs.
    pub struct Counting;

    // SAFETY: every method forwards to `System` with the arguments it was
    // given and returns what it returns; the counters are atomics.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOCS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size() as u64, Relaxed);
            System.alloc(layout)
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            ALLOCS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size() as u64, Relaxed);
            System.alloc_zeroed(layout)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout)
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            ALLOCS.fetch_add(1, Relaxed);
            BYTES.fetch_add(new_size as u64, Relaxed);
            System.realloc(ptr, layout, new_size)
        }
    }

    /// Write the counts to `CIG_ALLOC_FILE`, read before the write so the
    /// report does not count itself.
    pub fn report() {
        let (n, b) = (ALLOCS.load(Relaxed), BYTES.load(Relaxed));
        if let Some(path) = std::env::var_os("CIG_ALLOC_FILE") {
            let _ = std::fs::write(path, format!("{n} {b}\n"));
        }
    }

    #[cfg(unix)]
    extern "C" fn at_exit() {
        report();
    }

    /// Unix: at process exit, wherever the script exits from. Elsewhere the
    /// report is written when the entry point returns.
    pub fn arm() {
        #[cfg(unix)]
        // SAFETY: `at_exit` is a plain `extern "C" fn()` that lives forever.
        unsafe {
            libc::atexit(at_exit);
        }
    }
}

#[cfg(feature = "alloc-count")]
#[global_allocator]
static COUNTING: counting::Counting = counting::Counting;

fn main() {
    #[cfg(feature = "alloc-count")]
    counting::arm();
    let code = cigscript::cli::entry();
    #[cfg(all(feature = "alloc-count", not(unix)))]
    counting::report();
    std::process::exit(code);
}
