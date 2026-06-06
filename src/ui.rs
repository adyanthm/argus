use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::{App, MAX_DISPLAY};
use argus::memory::{self, ProcessHandle};
use argus::process;
use argus::scanner::{FilterMode, ScanMode, ValueType};

pub fn render_app(app: &mut App, ctx: &egui::Context) {
    app.poll_scan();

    if app.scanning {
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    // refresh table every 500ms
    if !app.address_table.is_empty() && app.last_refresh.elapsed() > Duration::from_millis(500) {
        app.refresh_table();
        app.last_refresh = Instant::now();
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    // process selector
    if app.process_popup {
        let mut open = true;
        egui::Window::new("Select Process")
            .open(&mut open)
            .collapsible(false)
            .default_size([450.0, 400.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut app.process_search);
                });

                let search = app.process_search.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(350.0)
                    .show(ui, |ui| {
                        egui::Grid::new("proc_grid")
                            .striped(true)
                            .num_columns(3)
                            .show(ui, |ui| {
                                ui.strong("PID");
                                ui.strong("Name");
                                ui.label("");
                                ui.end_row();

                                let mut selected = None;
                                for p in &app.process_list {
                                    if !search.is_empty()
                                        && !p.name.to_lowercase().contains(&search)
                                        && !p.pid.to_string().contains(&search)
                                    {
                                        continue;
                                    }
                                    ui.monospace(p.pid.to_string());
                                    ui.label(&p.name);
                                    if ui.small_button("Attach").clicked() {
                                        selected = Some((p.pid, p.name.clone()));
                                    }
                                    ui.end_row();
                                }

                                if let Some((pid, name)) = selected {
                                    match ProcessHandle::open(pid, name) {
                                        Ok(h) => {
                                            app.process = Some(Arc::new(h));
                                            app.reset_scan();
                                            app.address_table.clear();
                                            app.process_popup = false;
                                        }
                                        Err(e) => app.scan_error = Some(e),
                                    }
                                }
                            });
                    });
            });
        if !open {
            app.process_popup = false;
        }
    }

    // top bar
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.strong("Argus");
            ui.separator();
            if let Some(p) = &app.process {
                ui.label(format!("{} (PID: {})", p.name, p.pid));
            } else {
                ui.label("No process");
            }
            if ui.button("Select Process").clicked() {
                app.process_popup = true;
                app.process_list = process::list_processes();
                app.process_search.clear();
            }
        });
    });

    // address table
    egui::TopBottomPanel::bottom("addr_panel")
        .min_height(100.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Address Table");
            });

            if app.address_table.is_empty() {
                ui.label("Add addresses from scan results");
                return;
            }

            let mut remove = None;
            let mut toggle = None;
            let mut write: Option<(usize, String)> = None;

            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    egui::Grid::new("addr_grid")
                        .striped(true)
                        .num_columns(7)
                        .show(ui, |ui| {
                            ui.strong("Address");
                            ui.strong("Description");
                            ui.strong("Type");
                            ui.strong("Value");
                            ui.strong("Set Value");
                            ui.strong("Freeze");
                            ui.label("");
                            ui.end_row();

                            for (i, entry) in app.address_table.iter_mut().enumerate() {
                                ui.monospace(format!("0x{:X}", entry.address));
                                ui.add(
                                    egui::TextEdit::singleline(&mut entry.description)
                                        .desired_width(80.0),
                                );
                                ui.label(entry.value_type.label());
                                ui.monospace(&entry.display_value);
                                ui.add(
                                    egui::TextEdit::singleline(&mut entry.frozen_value)
                                        .desired_width(70.0),
                                );

                                let freeze_text = if entry.frozen { "🔒" } else { "🔓" };
                                if ui.button(freeze_text).clicked() {
                                    toggle = Some(i);
                                }
                                ui.horizontal(|ui| {
                                    if ui.small_button("Write").clicked() {
                                        write = Some((i, entry.frozen_value.clone()));
                                    }
                                    if ui.small_button("✕").clicked() {
                                        remove = Some(i);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });

            if let Some(i) = toggle {
                app.address_table[i].frozen = !app.address_table[i].frozen;
                app.sync_frozen();
                if app.address_table[i].frozen {
                    app.ensure_freeze_thread();
                }
            }
            if let Some(i) = remove {
                app.address_table.remove(i);
                app.sync_frozen();
            }
            if let Some((i, val)) = write
                && let Some(process) = &app.process
            {
                let entry = &app.address_table[i];
                if let Some(bytes) = entry.value_type.parse_value(&val) {
                    memory::write_memory(process, entry.address, &bytes);
                    app.refresh_table();
                }
            }
        });

    // scan controls
    egui::SidePanel::left("scan_panel")
        .default_width(200.0)
        .show(ctx, |ui| {
            ui.strong("Scan");
            ui.add_space(4.0);

            ui.label("Value Type");
            egui::ComboBox::from_id_salt("vtype")
                .width(180.0)
                .selected_text(app.value_type.label())
                .show_ui(ui, |ui| {
                    for vt in ValueType::ALL {
                        ui.selectable_value(&mut app.value_type, *vt, vt.label());
                    }
                });

            if !app.has_scanned {
                ui.label("Scan Type");
                egui::ComboBox::from_id_salt("smode")
                    .width(180.0)
                    .selected_text(app.scan_mode.label())
                    .show_ui(ui, |ui| {
                        for s in ScanMode::ALL {
                            ui.selectable_value(&mut app.scan_mode, *s, s.label());
                        }
                    });
            } else {
                ui.label("Filter Type");
                egui::ComboBox::from_id_salt("fmode")
                    .width(180.0)
                    .selected_text(app.filter_mode.label())
                    .show_ui(ui, |ui| {
                        for f in FilterMode::ALL {
                            ui.selectable_value(&mut app.filter_mode, *f, f.label());
                        }
                    });
            }

            let needs_val = if app.has_scanned {
                app.filter_mode.needs_value()
            } else {
                app.scan_mode.needs_value()
            };

            if needs_val {
                ui.label("Value");
                ui.text_edit_singleline(&mut app.scan_value);
            }

            ui.add_space(8.0);

            if app.scanning {
                ui.add(egui::ProgressBar::new(app.scan_progress).animate(true));
            }

            let enabled = app.process.is_some() && !app.scanning;
            ui.add_enabled_ui(enabled, |ui| {
                if !app.has_scanned {
                    if ui.button("First Scan").clicked() {
                        app.start_first_scan();
                    }
                } else {
                    if ui.button("Next Scan").clicked() {
                        app.start_next_scan();
                    }
                    if ui.button("New Scan").clicked() {
                        app.reset_scan();
                    }
                }
            });

            ui.add_space(8.0);

            if let Some(results) = &app.scan_results {
                if results.is_snapshot() {
                    ui.label("Snapshot stored — use Next Scan");
                } else {
                    ui.label(format!("Found: {}", app.result_count));
                    if app.result_count > MAX_DISPLAY {
                        ui.small(format!("(showing first {})", MAX_DISPLAY));
                    }
                }
            }

            if let Some(err) = &app.scan_error {
                ui.colored_label(egui::Color32::RED, err);
            }
        });

    // results
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.strong("Results");

        if app.display_results.is_empty() {
            ui.label(if app.has_scanned {
                "No results"
            } else {
                "Scan to see results"
            });
            return;
        }

        let vtype = app.value_type;
        let mut add = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("res_grid")
                .striped(true)
                .num_columns(4)
                .show(ui, |ui| {
                    ui.strong("Address");
                    ui.strong("Value");
                    ui.strong("Previous");
                    ui.label("");
                    ui.end_row();

                    for result in &app.display_results {
                        ui.monospace(format!("0x{:X}", result.address));
                        ui.monospace(vtype.format_bytes(&result.value[..result.size as usize]));
                        ui.monospace(vtype.format_bytes(&result.previous[..result.size as usize]));
                        if ui.small_button("Add").clicked() {
                            add = Some(result.clone());
                        }
                        ui.end_row();
                    }
                });
        });

        if let Some(r) = add {
            app.add_to_table(&r);
        }
    });
}
