# Contribution Guide

Welcome! If you are new to the codebase, this guide provides a file-by-file breakdown of the repository, explaining why certain architectural decisions were made and demystifying the more complex code snippets.

## File Review

### `src/main.rs`
**Purpose:** The entry point.
**Why it's minimal:** In earlier iterations, `main.rs` contained all state, rendering, and logic. This led to an unmaintainable monolith. It is now strictly responsible for configuring the `eframe` window and delegating execution to the `App` module.

### `src/app.rs`
**Purpose:** Application state and logic.
**Complex Snippet:**
```rust
let (tx, rx) = mpsc::channel();
self.scan_receiver = Some(rx);
```
**Explanation:** `mpsc` stands for Multi-Producer, Single-Consumer. We use this to communicate between threads. The scanning thread gets the transmitter (`tx`) to send progress percentages. The UI thread gets the receiver (`rx`). During the UI render loop, we call `rx.try_recv()`. We use `try_recv()` instead of `recv()` because `recv()` halts the thread until a message arrives, which would completely freeze the graphical interface.

### `src/ui.rs`
**Purpose:** Pure rendering logic.
**Why it's separate:** Immediate-mode GUIs like `egui` execute the layout code 60 times a second. By isolating this code into `ui.rs`, we ensure that rendering bugs do not bleed into memory logic, and memory logic does not inadvertently slow down frame times.

### `src/memory.rs`
**Purpose:** Low-level Windows API interaction.
**Complex Snippet:**
```rust
unsafe impl Send for ProcessHandle {}
unsafe impl Sync for ProcessHandle {}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.handle); }
    }
}
```
**Explanation:** 
- **Send/Sync:** By default, raw pointers (`HANDLE`) cannot be passed between threads in Rust to prevent race conditions. Because Win32 Kernel Handles are inherently thread-safe at the OS level, we use `unsafe impl Send/Sync` to explicitly tell the Rust compiler it is safe to hand this object to our Rayon worker threads.
- **Drop (RAII):** C/C++ developers often forget to call `CloseHandle()`, leading to catastrophic resource leaks. Rust's `Drop` trait automatically executes when the variable goes out of scope. We enforce that the handle is closed automatically, making resource leaks mathematically impossible.

### `src/scanner.rs`
**Purpose:** The high-performance scanning engine.
**Complex Snippet:**
```rust
let results: Vec<ScanResult> = regions.par_iter().enumerate().flat_map(|(_i, region)| {
    // ...
    for offset in memchr::memchr_iter(first_byte, &data) {
```
**Explanation:**
- **`par_iter()`:** Provided by the `rayon` crate. Instead of a standard `for` loop, this converts the iterator into a parallel iterator. The runtime automatically spawns a thread pool matching your CPU core count and distributes the memory regions among them.
- **`memchr_iter`:** A standard `[u8]` equality check (e.g. `chunk == target`) invokes a native bounds-checked loop. For gigabytes of RAM, billions of bounds checks destroy performance. `memchr` leverages SIMD (Single Instruction, Multiple Data) processor architecture to check up to 32 bytes simultaneously in hardware, bypassing software bounds checking.
- **Why `ScanResult` uses `[u8; 32]`:** Previously, `ScanResult` held a `Vec<u8>`. A vector allocates memory on the heap. Finding 5 million matches meant allocating 10 million vectors, invoking the system memory allocator continuously and locking the OS. By using a fixed-size array `[u8; 32]`, the result is allocated entirely on the stack (zero allocations), allowing the program to generate millions of results in milliseconds without memory fragmentation.
