use sysinfo::System;

pub struct ProcessInfo {
    pub name: String,
    pub pid: u32,
}

pub fn list_processes() -> Vec<ProcessInfo> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut processes: Vec<ProcessInfo> = system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessInfo {
            name: process.name().to_string_lossy().to_string(),
            pid: pid.as_u32(),
        })
        .collect();

    processes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    processes
}
