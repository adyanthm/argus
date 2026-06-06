use std::mem;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Memory::{
    MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS, VirtualQueryEx,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
    PROCESS_VM_WRITE,
};

pub struct ProcessHandle {
    handle: HANDLE,
    pub pid: u32,
    pub name: String,
}

unsafe impl Send for ProcessHandle {}
unsafe impl Sync for ProcessHandle {}

impl ProcessHandle {
    pub fn open(pid: u32, name: String) -> Result<Self, String> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION,
                false,
                pid,
            )
        }
        .map_err(|e| format!("OpenProcess failed: {e}"))?;

        Ok(Self { handle, pid, name })
    }

    pub fn raw(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[derive(Clone)]
pub struct MemoryRegion {
    pub base_address: usize,
    pub size: usize,
    pub protect: u32,
}

impl MemoryRegion {
    pub fn is_readable(&self) -> bool {
        (self.protect & PAGE_NOACCESS.0) == 0 && (self.protect & PAGE_GUARD.0) == 0
    }
}

pub fn query_regions(handle: &ProcessHandle) -> Vec<MemoryRegion> {
    let mut address: usize = 0;
    let mut regions = Vec::new();

    loop {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        let result = unsafe {
            VirtualQueryEx(
                handle.raw(),
                Some(address as _),
                &mut mbi,
                mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if result == 0 {
            break;
        }

        if mbi.State == MEM_COMMIT {
            let region = MemoryRegion {
                base_address: mbi.BaseAddress as usize,
                size: mbi.RegionSize,
                protect: mbi.Protect.0,
            };
            if region.is_readable() {
                regions.push(region);
            }
        }

        match (mbi.BaseAddress as usize).checked_add(mbi.RegionSize) {
            Some(next) => address = next,
            None => break,
        }
    }

    regions
}

pub fn read_memory(handle: &ProcessHandle, address: usize, size: usize) -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; size];
    let mut bytes_read = 0;

    let result = unsafe {
        ReadProcessMemory(
            handle.raw(),
            address as _,
            buffer.as_mut_ptr() as _,
            size,
            Some(&mut bytes_read),
        )
    };

    if result.is_ok() {
        buffer.truncate(bytes_read);
        Some(buffer)
    } else {
        None
    }
}

pub fn write_memory(handle: &ProcessHandle, address: usize, data: &[u8]) -> bool {
    let result = unsafe {
        WriteProcessMemory(
            handle.raw(),
            address as _,
            data.as_ptr() as _,
            data.len(),
            None,
        )
    };
    result.is_ok()
}
