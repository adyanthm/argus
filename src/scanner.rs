use rayon::prelude::*;
use std::sync::mpsc;
use std::thread;

use crate::memory::{self, ProcessHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Ascii,
    Aob,
}

impl ValueType {
    pub fn byte_size(&self) -> Option<usize> {
        match self {
            Self::I8 => Some(1),
            Self::I16 => Some(2),
            Self::I32 => Some(4),
            Self::I64 => Some(8),
            Self::F32 => Some(4),
            Self::F64 => Some(8),
            Self::Ascii | Self::Aob => None,
        }
    }

    pub fn is_variable_length(&self) -> bool {
        self.byte_size().is_none()
    }

    pub fn format_bytes(&self, bytes: &[u8]) -> String {
        match self {
            Self::I8 => {
                if bytes.is_empty() {
                    return "???".into();
                }
                (bytes[0] as i8).to_string()
            }
            Self::I16 => {
                if bytes.len() < 2 {
                    return "???".into();
                }
                i16::from_le_bytes(bytes[..2].try_into().unwrap()).to_string()
            }
            Self::I32 => {
                if bytes.len() < 4 {
                    return "???".into();
                }
                i32::from_le_bytes(bytes[..4].try_into().unwrap()).to_string()
            }
            Self::I64 => {
                if bytes.len() < 8 {
                    return "???".into();
                }
                i64::from_le_bytes(bytes[..8].try_into().unwrap()).to_string()
            }
            Self::F32 => {
                if bytes.len() < 4 {
                    return "???".into();
                }
                format!("{:.4}", f32::from_le_bytes(bytes[..4].try_into().unwrap()))
            }
            Self::F64 => {
                if bytes.len() < 8 {
                    return "???".into();
                }
                format!("{:.4}", f64::from_le_bytes(bytes[..8].try_into().unwrap()))
            }
            Self::Ascii => bytes
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect(),
            Self::Aob => bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub fn parse_value(&self, input: &str) -> Option<Vec<u8>> {
        match self {
            Self::I8 => input.parse::<i8>().ok().map(|v| v.to_le_bytes().to_vec()),
            Self::I16 => input.parse::<i16>().ok().map(|v| v.to_le_bytes().to_vec()),
            Self::I32 => input.parse::<i32>().ok().map(|v| v.to_le_bytes().to_vec()),
            Self::I64 => input.parse::<i64>().ok().map(|v| v.to_le_bytes().to_vec()),
            Self::F32 => input.parse::<f32>().ok().map(|v| v.to_le_bytes().to_vec()),
            Self::F64 => input.parse::<f64>().ok().map(|v| v.to_le_bytes().to_vec()),
            Self::Ascii => {
                if input.is_empty() {
                    return None;
                }
                Some(input.as_bytes().to_vec())
            }
            Self::Aob => {
                let hex: String = input.chars().filter(|c| !c.is_whitespace()).collect();
                if hex.is_empty() || hex.len().is_multiple_of(2) {
                    return None;
                }
                (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
                    .collect()
            }
        }
    }

    pub const ALL: &[ValueType] = &[
        ValueType::I8,
        ValueType::I16,
        ValueType::I32,
        ValueType::I64,
        ValueType::F32,
        ValueType::F64,
        ValueType::Ascii,
        ValueType::Aob,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::I8 => "Byte (i8)",
            Self::I16 => "2 Bytes (i16)",
            Self::I32 => "4 Bytes (i32)",
            Self::I64 => "8 Bytes (i64)",
            Self::F32 => "Float (f32)",
            Self::F64 => "Double (f64)",
            Self::Ascii => "String (ASCII)",
            Self::Aob => "Array of Bytes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Exact,
    GreaterThan,
    LessThan,
    UnknownInitial,
}

impl ScanMode {
    pub const ALL: &[ScanMode] = &[
        ScanMode::Exact,
        ScanMode::GreaterThan,
        ScanMode::LessThan,
        ScanMode::UnknownInitial,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Exact => "Exact Value",
            Self::GreaterThan => "Greater Than",
            Self::LessThan => "Less Than",
            Self::UnknownInitial => "Unknown Initial Value",
        }
    }

    pub fn needs_value(&self) -> bool {
        !matches!(self, Self::UnknownInitial)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Exact,
    Changed,
    Unchanged,
    Increased,
    Decreased,
    GreaterThan,
    LessThan,
}

impl FilterMode {
    pub const ALL: &[FilterMode] = &[
        FilterMode::Exact,
        FilterMode::Changed,
        FilterMode::Unchanged,
        FilterMode::Increased,
        FilterMode::Decreased,
        FilterMode::GreaterThan,
        FilterMode::LessThan,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Exact => "Exact Value",
            Self::Changed => "Changed Value",
            Self::Unchanged => "Unchanged Value",
            Self::Increased => "Increased Value",
            Self::Decreased => "Decreased Value",
            Self::GreaterThan => "Greater Than",
            Self::LessThan => "Less Than",
        }
    }

    pub fn needs_value(&self) -> bool {
        matches!(self, Self::Exact | Self::GreaterThan | Self::LessThan)
    }
}

#[derive(Clone)]
pub struct ScanResult {
    pub address: usize,
    pub size: u8,
    pub value: [u8; 32],
    pub previous: [u8; 32],
}

pub enum ScanResults {
    Addresses(Vec<ScanResult>),
    Snapshot(Vec<(usize, Vec<u8>)>),
}

impl ScanResults {
    pub fn is_snapshot(&self) -> bool {
        matches!(self, Self::Snapshot(_))
    }
}

pub enum ScanMessage {
    Progress(f32),
    Done(ScanResults),
    Error(String),
}

fn compare_bytes(a: &[u8], b: &[u8], vtype: ValueType) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match vtype {
        ValueType::I8 => (a[0] as i8).cmp(&(b[0] as i8)),
        ValueType::I16 => {
            let va = i16::from_le_bytes(a[..2].try_into().unwrap());
            let vb = i16::from_le_bytes(b[..2].try_into().unwrap());
            va.cmp(&vb)
        }
        ValueType::I32 => {
            let va = i32::from_le_bytes(a[..4].try_into().unwrap());
            let vb = i32::from_le_bytes(b[..4].try_into().unwrap());
            va.cmp(&vb)
        }
        ValueType::I64 => {
            let va = i64::from_le_bytes(a[..8].try_into().unwrap());
            let vb = i64::from_le_bytes(b[..8].try_into().unwrap());
            va.cmp(&vb)
        }
        ValueType::F32 => {
            let va = f32::from_le_bytes(a[..4].try_into().unwrap());
            let vb = f32::from_le_bytes(b[..4].try_into().unwrap());
            va.partial_cmp(&vb).unwrap_or(Ordering::Equal)
        }
        ValueType::F64 => {
            let va = f64::from_le_bytes(a[..8].try_into().unwrap());
            let vb = f64::from_le_bytes(b[..8].try_into().unwrap());
            va.partial_cmp(&vb).unwrap_or(Ordering::Equal)
        }
        ValueType::Ascii | ValueType::Aob => a.cmp(b),
    }
}

fn matches_scan(data: &[u8], target: &[u8], vtype: ValueType, mode: ScanMode) -> bool {
    use std::cmp::Ordering;
    match mode {
        ScanMode::Exact => data == target,
        ScanMode::GreaterThan => compare_bytes(data, target, vtype) == Ordering::Greater,
        ScanMode::LessThan => compare_bytes(data, target, vtype) == Ordering::Less,
        ScanMode::UnknownInitial => true,
    }
}

fn matches_filter(
    current: &[u8],
    previous: &[u8],
    target: Option<&[u8]>,
    vtype: ValueType,
    mode: FilterMode,
) -> bool {
    use std::cmp::Ordering;
    match mode {
        FilterMode::Exact => target.is_some_and(|t| current == t),
        FilterMode::Changed => current != previous,
        FilterMode::Unchanged => current == previous,
        FilterMode::Increased => compare_bytes(current, previous, vtype) == Ordering::Greater,
        FilterMode::Decreased => compare_bytes(current, previous, vtype) == Ordering::Less,
        FilterMode::GreaterThan => {
            target.is_some_and(|t| compare_bytes(current, t, vtype) == Ordering::Greater)
        }
        FilterMode::LessThan => {
            target.is_some_and(|t| compare_bytes(current, t, vtype) == Ordering::Less)
        }
    }
}

pub fn first_scan(
    pid: u32,
    process_name: String,
    vtype: ValueType,
    mode: ScanMode,
    target: Option<Vec<u8>>,
    tx: mpsc::Sender<ScanMessage>,
) {
    thread::spawn(move || {
        let handle = match ProcessHandle::open(pid, process_name) {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(ScanMessage::Error(e));
                return;
            }
        };

        let regions = memory::query_regions(&handle);
        let total = regions.len();

        if mode == ScanMode::UnknownInitial {
            if vtype.is_variable_length() {
                let _ = tx.send(ScanMessage::Error(
                    "Unknown Initial not supported for variable-length types".into(),
                ));
                return;
            }
            let mut snapshot = Vec::new();
            for (i, region) in regions.iter().enumerate() {
                if let Some(data) = memory::read_memory(&handle, region.base_address, region.size) {
                    snapshot.push((region.base_address, data));
                }
                let _ = tx.send(ScanMessage::Progress((i + 1) as f32 / total as f32));
            }
            let _ = tx.send(ScanMessage::Done(ScanResults::Snapshot(snapshot)));
            return;
        }

        let target = match target {
            Some(t) => t,
            None => {
                let _ = tx.send(ScanMessage::Error("No value provided".into()));
                return;
            }
        };

        let val_size = match vtype.byte_size() {
            Some(s) => s,
            None => target.len(),
        };

        let tx_sync = std::sync::Mutex::new(tx.clone());
        let completed = std::sync::atomic::AtomicUsize::new(0);

        let results: Vec<ScanResult> = regions.par_iter().enumerate().flat_map(|(_i, region)| {
            let mut local_results = Vec::new();
            if let Some(data) = memory::read_memory(&handle, region.base_address, region.size) {
                if data.len() >= val_size {
                    if mode == ScanMode::Exact && !target.is_empty() {
                        // fast simd exact match
                        let first_byte = target[0];
                        for offset in memchr::memchr_iter(first_byte, &data) {
                            if offset + val_size <= data.len() {
                                let chunk = &data[offset..offset + val_size];
                                if chunk == target {
                                    let mut val = [0u8; 32];
                                    let size = val_size.min(32);
                                    val[..size].copy_from_slice(&chunk[..size]);
                                    local_results.push(ScanResult {
                                        address: region.base_address + offset,
                                        size: size as u8,
                                        value: val,
                                        previous: val,
                                    });
                                }
                            }
                        }
                    } else {
                        // fallback match
                        for offset in 0..=(data.len() - val_size) {
                            let chunk = &data[offset..offset + val_size];
                            if matches_scan(chunk, &target, vtype, mode) {
                                let mut val = [0u8; 32];
                                let size = val_size.min(32);
                                val[..size].copy_from_slice(&chunk[..size]);
                                local_results.push(ScanResult {
                                    address: region.base_address + offset,
                                    size: size as u8,
                                    value: val,
                                    previous: val,
                                });
                            }
                        }
                    }
                }
            }
            let count = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if count % 10 == 0 {
                let _ = tx_sync.lock().unwrap().send(ScanMessage::Progress(count as f32 / total as f32));
            }
            local_results
        }).collect();

        let _ = tx.send(ScanMessage::Done(ScanResults::Addresses(results)));
    });
}

pub fn next_scan(
    pid: u32,
    process_name: String,
    vtype: ValueType,
    mode: FilterMode,
    target: Option<Vec<u8>>,
    previous_results: ScanResults,
    tx: mpsc::Sender<ScanMessage>,
) {
    thread::spawn(move || {
        let handle = match ProcessHandle::open(pid, process_name) {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(ScanMessage::Error(e));
                return;
            }
        };

        let target_ref = target.as_deref();

        match previous_results {
            ScanResults::Addresses(prev) => {
                let regions = memory::query_regions(&handle);
                let total = regions.len();
                let tx_sync = std::sync::Mutex::new(tx.clone());
                let completed = std::sync::atomic::AtomicUsize::new(0);

                let results: Vec<ScanResult> = regions.par_iter().enumerate().flat_map(|(_i, region)| {
                    let mut local_results = Vec::new();

                    // filter addresses by region
                    let start_idx = prev.partition_point(|x| x.address < region.base_address);
                    let end_idx = prev.partition_point(|x| x.address < region.base_address + region.size);
                    let region_addrs = &prev[start_idx..end_idx];

                    if !region_addrs.is_empty() {
                        local_results.reserve(region_addrs.len());
                        if let Some(data) = memory::read_memory(&handle, region.base_address, region.size) {
                            for entry in region_addrs {
                                let offset = entry.address - region.base_address;
                                let size = entry.size as usize;
                                if offset + size <= data.len() {
                                    let new_chunk = &data[offset..offset + size];
                                    let old_chunk = &entry.value[..size];
                                    if matches_filter(new_chunk, old_chunk, target_ref, vtype, mode) {
                                        let mut val = [0u8; 32];
                                        val[..size].copy_from_slice(new_chunk);
                                        local_results.push(ScanResult {
                                            address: entry.address,
                                            size: size as u8,
                                            value: val,
                                            previous: entry.value,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let count = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 10 == 0 {
                        let _ = tx_sync.lock().unwrap().send(ScanMessage::Progress(count as f32 / total as f32));
                    }
                    local_results
                }).collect();

                let _ = tx.send(ScanMessage::Done(ScanResults::Addresses(results)));
            }
            ScanResults::Snapshot(snapshot) => {
                let val_size = vtype.byte_size().unwrap_or(1);
                let total = snapshot.len();
                let tx_sync = std::sync::Mutex::new(tx.clone());
                let completed = std::sync::atomic::AtomicUsize::new(0);

                let results: Vec<ScanResult> = snapshot.par_iter().enumerate().flat_map(|(_i, (base, old_data))| {
                    let mut local_results = Vec::new();
                    if let Some(new_data) = memory::read_memory(&handle, *base, old_data.len()) {
                        let len = old_data.len().min(new_data.len());
                        if len >= val_size {
                            for offset in 0..=(len - val_size) {
                                let old_chunk = &old_data[offset..offset + val_size];
                                let new_chunk = &new_data[offset..offset + val_size];
                                if matches_filter(new_chunk, old_chunk, target_ref, vtype, mode) {
                                    let mut val = [0u8; 32];
                                    let mut prev_val = [0u8; 32];
                                    let size = val_size.min(32);
                                    val[..size].copy_from_slice(&new_chunk[..size]);
                                    prev_val[..size].copy_from_slice(&old_chunk[..size]);
                                    
                                    local_results.push(ScanResult {
                                        address: *base + offset,
                                        size: size as u8,
                                        value: val,
                                        previous: prev_val,
                                    });
                                }
                            }
                        }
                    }
                    let count = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if count % 10 == 0 {
                        let _ = tx_sync.lock().unwrap().send(ScanMessage::Progress(count as f32 / total as f32));
                    }
                    local_results
                }).collect();

                let _ = tx.send(ScanMessage::Done(ScanResults::Addresses(results)));
            }
        }
    });
}
