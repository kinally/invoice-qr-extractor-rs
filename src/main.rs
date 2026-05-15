mod app;
mod core;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 580.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("发票PDF二维码提取工具"),
        ..Default::default()
    };

    eframe::run_native(
        "发票PDF二维码提取工具",
        native_options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(app::InvoiceQrApp::default()))
        }),
    )
}

/// 加载 Windows 系统字体（微软雅黑）到 egui 中，
/// 解决中文显示为方框的问题。
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_paths = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\SIMHEI.TTF",
    ];

    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            let name = format!("chinese_font_{}", path.rsplit('\\').next().unwrap_or("unknown"));
            fonts.font_data.insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));

            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, name);

            break;
        }
    }

    ctx.set_fonts(fonts);
}
