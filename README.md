# Argus

A high-performance, multithreaded memory scanner and editor built in Rust **exclusively for Windows**. It provides rapid memory reading, live address table tracking, and persistent value freezing capabilities.

![Demo](demo.gif)

## Architecture

The project is split into a non-blocking GUI frontend powered by `eframe` and a backend engine interacting directly with the Windows API.

Key technical specifications:
* **Hardware Accelerated Scanning**: Exact value scans utilize SIMD/AVX instructions via `memchr` to scan gigabytes of memory sequentially.
* **Multithreading**: Memory regions are queried and processed in parallel across all CPU cores using `rayon`.
* **Zero-Allocation Data Structures**: Scan results utilize fixed-size arrays (`[u8; 32]`) to eliminate heap allocations during processing loops, preventing garbage collection stutters.
* **Batched Syscalls**: Scan filtering groups addresses by their corresponding physical memory region. Instead of performing a `ReadProcessMemory` syscall per address, it reads the entire region once into local RAM and filters internally.

## Supported Types

The scanner natively parses, formats, and scans for 8 value types:
* `i8`, `i16`, `i32`, `i64`
* `f32`, `f64`
* ASCII Strings
* Array of Bytes (AOB)

## Building and Running

Requires Windows. Target processes with elevated permissions require the scanner to be run as Administrator.

```bash
cargo build --release
cargo run --release
```

## Dummy Game Testing

An interactive command-line Dungeon Crawler game is included to safely test the memory scanner (e.g. freezing HP or Gold) without hooking into external software.

```bash
rustc dummy_game.rs
./dummy_game.exe
```
