pub mod app;
pub mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let icon_image = image::load_from_memory(include_bytes!("../icon.png"))
        .expect("Failed to load icon")
        .to_rgba8();
    
    let icon_data = std::sync::Arc::new(egui::IconData {
        width: icon_image.width(),
        height: icon_image.height(),
        rgba: icon_image.into_raw(),
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_icon(icon_data),
        ..Default::default()
    };
    eframe::run_native(
        "Argus",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(app::App::default()))
        }),
    )
}
