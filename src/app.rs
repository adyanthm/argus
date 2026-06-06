use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use argus::memory::{self, ProcessHandle};
use argus::process::ProcessInfo;
use argus::scanner::{
    FilterMode, ScanMessage, ScanMode, ScanResult, ScanResults, ValueType,
};

pub const MAX_DISPLAY: usize = 2000;

#[derive(Clone)]
pub struct AddressEntry {
    pub address: usize,
    pub description: String,
    pub value_type: ValueType,
    pub frozen: bool,
    pub frozen_value: String,
    pub display_value: String,
}

pub struct App {
    pub process: Option<Arc<ProcessHandle>>,
    pub process_search: String,
    pub process_list: Vec<ProcessInfo>,
    pub process_popup: bool,

    pub value_type: ValueType,
    pub scan_mode: ScanMode,
    pub filter_mode: FilterMode,
    pub scan_value: String,

    pub scan_results: Option<ScanResults>,
    pub display_results: Vec<ScanResult>,
    pub result_count: usize,
    pub has_scanned: bool,

    pub scanning: bool,
    pub scan_progress: f32,
    pub scan_rx: Option<mpsc::Receiver<ScanMessage>>,
    pub scan_error: Option<String>,

    pub address_table: Vec<AddressEntry>,
    pub frozen_entries: Arc<Mutex<Vec<AddressEntry>>>,
    pub freeze_running: Arc<Mutex<bool>>,

    pub last_refresh: Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            process: None,
            process_search: String::new(),
            process_list: Vec::new(),
            process_popup: false,
            value_type: ValueType::I32,
            scan_mode: ScanMode::Exact,
            filter_mode: FilterMode::Exact,
            scan_value: String::new(),
            scan_results: None,
            display_results: Vec::new(),
            result_count: 0,
            has_scanned: false,
            scanning: false,
            scan_progress: 0.0,
            scan_rx: None,
            scan_error: None,
            address_table: Vec::new(),
            frozen_entries: Arc::new(Mutex::new(Vec::new())),
            freeze_running: Arc::new(Mutex::new(false)),
            last_refresh: Instant::now(),
        }
    }
}

impl App {
    pub fn start_first_scan(&mut self) {
        let process = match &self.process {
            Some(p) => p,
            None => return,
        };

        let target = if self.scan_mode.needs_value() {
            match self.value_type.parse_value(&self.scan_value) {
                Some(v) => Some(v),
                None => {
                    self.scan_error = Some("Invalid value".into());
                    return;
                }
            }
        } else {
            None
        };

        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        self.scanning = true;
        self.scan_progress = 0.0;
        self.scan_error = None;
        self.display_results.clear();

        argus::scanner::first_scan(
            process.pid,
            process.name.clone(),
            self.value_type,
            self.scan_mode,
            target,
            tx,
        );
    }

    pub fn start_next_scan(&mut self) {
        let process = match &self.process {
            Some(p) => p,
            None => return,
        };

        let previous = match self.scan_results.take() {
            Some(r) => r,
            None => return,
        };

        let target = if self.filter_mode.needs_value() {
            match self.value_type.parse_value(&self.scan_value) {
                Some(v) => Some(v),
                None => {
                    self.scan_error = Some("Invalid value".into());
                    self.scan_results = Some(previous);
                    return;
                }
            }
        } else {
            None
        };

        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        self.scanning = true;
        self.scan_progress = 0.0;
        self.scan_error = None;

        argus::scanner::next_scan(
            process.pid,
            process.name.clone(),
            self.value_type,
            self.filter_mode,
            target,
            previous,
            tx,
        );
    }

    pub fn reset_scan(&mut self) {
        self.scan_results = None;
        self.display_results.clear();
        self.result_count = 0;
        self.has_scanned = false;
        self.scan_error = None;
    }

    pub fn poll_scan(&mut self) {
        if let Some(rx) = &self.scan_rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ScanMessage::Progress(p) => self.scan_progress = p,
                    ScanMessage::Done(results) => {
                        self.scanning = false;
                        self.has_scanned = true;
                        self.scan_progress = 1.0;
                        match &results {
                            ScanResults::Addresses(addrs) => {
                                self.result_count = addrs.len();
                                let limit = MAX_DISPLAY.min(addrs.len());
                                self.display_results = addrs[..limit].to_vec();
                            }
                            ScanResults::Snapshot(_) => {
                                self.result_count = 0;
                                self.display_results.clear();
                            }
                        }
                        self.scan_results = Some(results);
                    }
                    ScanMessage::Error(e) => {
                        self.scanning = false;
                        self.scan_error = Some(e);
                    }
                }
            }
        }
    }

    pub fn add_to_table(&mut self, result: &ScanResult) {
        if self.address_table.iter().any(|e| e.address == result.address) {
            return;
        }
        let display = self.value_type.format_bytes(&result.value[..result.size as usize]);
        self.address_table.push(AddressEntry {
            address: result.address,
            description: String::new(),
            value_type: self.value_type,
            frozen: false,
            frozen_value: display.clone(),
            display_value: display,
        });
    }

    pub fn refresh_table(&mut self) {
        let process = match &self.process {
            Some(p) => p.clone(),
            None => return,
        };
        for entry in &mut self.address_table {
            let size = entry.value_type.byte_size().unwrap_or(entry.display_value.len());
            if let Some(data) = memory::read_memory(&process, entry.address, size) {
                entry.display_value = entry.value_type.format_bytes(&data);
            }
        }
    }

    pub fn sync_frozen(&self) {
        if let Ok(mut frozen) = self.frozen_entries.lock() {
            *frozen = self.address_table.iter().filter(|e| e.frozen).cloned().collect();
        }
    }

    pub fn ensure_freeze_thread(&self) {
        let mut running = self.freeze_running.lock().unwrap();
        if *running {
            return;
        }
        *running = true;

        let frozen = Arc::clone(&self.frozen_entries);
        let flag = Arc::clone(&self.freeze_running);

        if let Some(process) = &self.process {
            let pid = process.pid;
            let name = process.name.clone();

            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(100));
                let entries = frozen.lock().unwrap().clone();
                if entries.is_empty() {
                    continue;
                }
                let handle = match ProcessHandle::open(pid, name.clone()) {
                    Ok(h) => h,
                    Err(_) => {
                        *flag.lock().unwrap() = false;
                        return;
                    }
                };
                for entry in &entries {
                    if let Some(bytes) = entry.value_type.parse_value(&entry.frozen_value) {
                        memory::write_memory(&handle, entry.address, &bytes);
                    }
                }
            });
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        crate::ui::render_app(self, ctx);
    }
}
