# Documentation

This document explains the internal mechanisms of Argus, detailing how it interacts with the operating system and processes data.

## Process Attachment and Memory Mapping
Before any scanning occurs, the application must acquire a handle to the target process. This is done via the Win32 `OpenProcess` API. The scanner requests `PROCESS_ALL_ACCESS` to read and write to the process's memory space. 

To prevent scanning invalid or protected memory (which would cause the scanner to crash or trigger anti-cheat software), the `VirtualQueryEx` API is used. This function returns a map of the process's memory layout. The scanner filters this map to include only memory blocks that are committed to RAM (`MEM_COMMIT`) and excludes any memory marked as `PAGE_NOACCESS` or `PAGE_GUARD`.

## The Scanning Engine

The scanner is split into two primary operations: First Scan and Next Scan. Both operate under a multithreaded architecture using the `rayon` crate to distribute the workload across all CPU cores.

### First Scan
1. **Region Allocation**: The list of valid memory regions is acquired.
2. **Parallel Iteration**: Each memory region is distributed to an available CPU thread.
3. **Memory Reading**: The thread issues a `ReadProcessMemory` syscall to copy the entire region into local RAM.
4. **Matching**:
   - For exact value scans, a hardware-accelerated SIMD search (`memchr`) scans the buffer for the first byte of the target value. When a match is found, the subsequent bytes are validated.
   - For inequality scans (Greater Than, Less Than), the buffer is checked sequentially.
5. **Collection**: Results are packaged into a zero-allocation `ScanResult` structure and sent back to the main thread.
6. **Snapshots**: If the user selects "Unknown Initial Value", the scanner does not evaluate bytes. Instead, it reads all valid regions and stores them in a massive localized RAM snapshot for future comparison.

### Next Scan (Filtering)
Filtering is heavily optimized to avoid excessive system calls.
1. The memory map is re-queried to account for any allocations or deallocations that occurred in the target process.
2. For each region, a binary search determines which previous addresses fall inside its boundaries.
3. If previous addresses exist within a region, the region is read into local RAM once.
4. The local buffer is evaluated against the saved previous values to determine if the condition (e.g., "Increased", "Changed") is met.

## State Management and UI Polling

The GUI is powered by `egui`, an immediate-mode framework that repaints constantly. 

To ensure the UI does not freeze during intensive 4GB memory scans, the scanning engine runs on a detached background thread. Communication between the scanner and the UI is handled via a multi-producer, single-consumer (`mpsc`) channel.

During every UI frame, the application calls `try_recv()` on the channel. If a progress update or completion message is available, the UI state is updated. If the channel is empty, the UI continues rendering without waiting.

## The Freeze Thread
When a user "freezes" an address, they are locking its value in the target process. This is achieved by spawning a dedicated background thread containing an infinite loop. 

Every 100 milliseconds, the thread acquires a lock on a shared `Arc<Mutex<Vec<AddressEntry>>>` containing the frozen addresses. It iterates through the list and issues `WriteProcessMemory` calls to enforce the frozen values, overriding any changes made by the game engine.
