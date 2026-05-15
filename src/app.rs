use std::path::Path;
use std::sync::mpsc;
use std::thread;

use crate::core;

// ─── 事件类型 ───

enum UiEvent {
    Progress(usize, usize),
    Log(String),
    Finished(usize, String, Vec<(usize, String, String, String)>),
    Error(String),
}

// ─── 应用结构体 ───

pub struct InvoiceQrApp {
    // 文件列表
    file_list: Vec<String>,

    // CSV输出路径
    csv_path: String,

    // 运行状态
    is_processing: bool,
    progress_current: usize,
    progress_total: usize,
    status_text: String,

    // 日志 (单一字符串缓冲区)
    logs: String,
    logs_scroll: bool,

    // 工作线程的事件接收器
    event_rx: Option<mpsc::Receiver<UiEvent>>,

    // 对话框状态
    show_confirm_clear: bool,
    show_confirm_exit: bool,
    show_error_dialog: Option<String>,

    // 拖拽状态
    drag_hover: bool,

    // 结果展示
    result_rows: Vec<(usize, String, String, String)>,
    show_results: bool,
}

impl Default for InvoiceQrApp {
    fn default() -> Self {
        let csv_default = format!(
            "{}\\二维码提取结果.csv",
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        );

        let logs = "提示：点击\"添加文件\"或\"添加文件夹\"选择PDF发票文件\n提示：也可直接将PDF文件/文件夹拖入窗口\n提示：列表中的文件将按顺序处理\n".to_string();

        Self {
            file_list: Vec::new(),
            csv_path: csv_default,
            is_processing: false,
            progress_current: 0,
            progress_total: 0,
            status_text: "就绪".to_string(),
            logs,
            event_rx: None,
            logs_scroll: false,
            show_confirm_clear: false,
            show_confirm_exit: false,
            show_error_dialog: None,
            drag_hover: false,
            result_rows: Vec::new(),
            show_results: false,
        }
    }
}

impl InvoiceQrApp {
    fn add_files_dialog(&mut self, ctx: &egui::Context) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("PDF文件", &["pdf"])
            .pick_files()
        {
            for f in files {
                let path = f.to_string_lossy().to_string();
                if !self.file_list.contains(&path) {
                    self.file_list.push(path);
                }
            }
            self.update_status();
        }
        ctx.request_repaint();
    }

    fn add_folder_dialog(&mut self, ctx: &egui::Context) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let path = folder.to_string_lossy().to_string();
            self.add_folder_inner(&path);
        }
        ctx.request_repaint();
    }

    fn add_folder_inner(&mut self, folder: &str) {
        if let Ok(entries) = std::fs::read_dir(folder) {
            let mut pdfs: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
                        .unwrap_or(false)
                })
                .map(|e| e.path().to_string_lossy().to_string())
                .collect();
            pdfs.sort();
        let added: Vec<String> = pdfs
                .iter()
                .filter(|f| !self.file_list.contains(*f))
                .cloned()
                .collect();
            let added_count = added.len();
            for f in added {
                self.file_list.push(f);
            }
            if added_count > 0 {
                self.add_log(&format!("从文件夹添加了 {} 个PDF文件", added_count));
                self.update_status();
            }
        }
    }

    fn handle_drop(&mut self, paths: &[String]) {
        if paths.is_empty() {
            return;
        }

        let mut added = 0;
        for p in paths {
            let path = Path::new(p);
            if path.is_dir() {
                // 拖入文件夹
                let old_len = self.file_list.len();
                self.add_folder_inner(p);
                if self.file_list.len() > old_len {
                    added += self.file_list.len() - old_len;
                }
            } else if path.is_file() {
                let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
                if ext.as_deref() == Some("pdf") {
                    let p_str = path.to_string_lossy().to_string();
                    if !self.file_list.contains(&p_str) {
                        self.file_list.push(p_str);
                        added += 1;
                    }
                }
            }
        }

        if added > 0 {
            self.add_log(&format!("拖入添加了 {} 个文件", added));
            self.update_status();
        }
    }

    fn remove_selected(&mut self, selected: &[usize]) {
        let mut to_remove: Vec<usize> = selected.to_vec();
        to_remove.sort_by(|a, b| b.cmp(a));
        to_remove.dedup();
        for &i in &to_remove {
            if i < self.file_list.len() {
                self.file_list.remove(i);
            }
        }
        self.update_status();
    }

    fn move_up(&mut self, index: usize) {
        if index > 0 && index < self.file_list.len() {
            self.file_list.swap(index - 1, index);
        }
    }

    fn move_down(&mut self, index: usize) {
        if index + 1 < self.file_list.len() {
            self.file_list.swap(index, index + 1);
        }
    }

    fn clear_list(&mut self) {
        self.file_list.clear();
        self.update_status();
        self.add_log("已清空文件列表");
    }

    fn update_status(&mut self) {
        self.status_text = format!("共 {} 个文件", self.file_list.len());
    }

    fn select_csv_path(&mut self, ctx: &egui::Context) {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("CSV文件", &["csv"])
            .set_file_name("二维码提取结果.csv")
            .save_file()
        {
            self.csv_path = file.to_string_lossy().to_string();
        }
        ctx.request_repaint();
    }

    fn add_log(&mut self, msg: &str) {
        self.logs.push_str(msg);
        self.logs.push('\n');
        self.logs_scroll = true;
    }

    /// 启动后台处理任务
    fn start_extract(&mut self) {
        if self.file_list.is_empty() {
            self.show_error_dialog = Some("请先添加PDF文件".to_string());
            return;
        }
        if self.csv_path.trim().is_empty() {
            self.show_error_dialog = Some("请选择CSV输出路径".to_string());
            return;
        }

        self.is_processing = true;
        self.progress_current = 0;
        self.progress_total = self.file_list.len();
        self.status_text = "正在处理...".to_string();

        self.add_log(&"\n".repeat(1));
        self.add_log(&"=".repeat(50));
        self.add_log(&format!("开始处理 {} 个文件...", self.file_list.len()));
        self.add_log(&format!("输出路径: {}", self.csv_path));

        let (tx, rx) = mpsc::channel::<UiEvent>();
        self.event_rx = Some(rx);

        let files = self.file_list.clone();
        let csv_out = self.csv_path.clone();
        let log_tx = tx.clone();
        let progress_tx = tx.clone();

        thread::spawn(move || {
            let (success_count, rows) = core::process_pdfs(
                &files,
                &csv_out,
                |current, total| {
                    let _ = progress_tx.send(UiEvent::Progress(current, total));
                },
                |msg| {
                    let _ = log_tx.send(UiEvent::Log(msg.to_string()));
                },
            );

            // 发送完成事件
            let _ = tx.send(UiEvent::Finished(success_count, csv_out.clone(), rows));

            // 自动打开输出文件夹
            if let Some(parent) = Path::new(&csv_out).parent() {
                if parent.exists() {
                    let _ = std::process::Command::new("explorer")
                        .arg(parent.to_string_lossy().as_ref())
                        .spawn();
                }
            }
        });
    }

    /// 处理UI事件
    fn process_events(&mut self) {
        // 先取出 receiver，避免 self 的借用冲突
        if self.event_rx.is_none() {
            return;
        }
        let rx = self.event_rx.take().unwrap();

        let mut is_finished = false;
        let mut finished_success = 0;
        let mut finished_csv = String::new();
        let mut error_msg = String::new();
        let mut log_entries: Vec<String> = Vec::new();

        while let Ok(event) = rx.try_recv() {
            match event {
                UiEvent::Progress(current, total) => {
                    self.progress_current = current;
                    self.progress_total = total;
                    self.status_text = format!("正在处理 {}/{}", current, total);
                }
                UiEvent::Log(msg) => {
                    log_entries.push(msg);
                }
                UiEvent::Finished(success, csv_path, rows) => {
                    is_finished = true;
                    finished_success = success;
                    finished_csv = csv_path;
                    self.result_rows = rows;
                    self.show_results = true;
                }
                UiEvent::Error(err) => {
                    is_finished = true;
                    error_msg = err;
                }
            }
        }

        // 批量写入日志（避免借用冲突）
        for msg in &log_entries {
            self.add_log(msg);
        }

        if is_finished {
            if error_msg.is_empty() {
                self.is_processing = false;
                self.progress_current = self.progress_total;
                self.status_text = "完成".to_string();
                self.add_log(&format!(
                    "\n✅ 处理完成！成功识别 {} 个文件",
                    finished_success
                ));
                self.add_log(&format!("📄 结果已保存: {}", finished_csv));
            } else {
                self.is_processing = false;
                self.status_text = "出错".to_string();
                self.show_error_dialog = Some(error_msg);
            }
            // 处理完成，不保存 receiver
        } else {
            // 未完成，放回去继续用
            self.event_rx = Some(rx);
        }
    }

    // ─── 从文件拖放到窗口 ───
    fn drag_and_drop(&mut self, ctx: &egui::Context) {

        // 使用 egui 的拖放支持
        if !self.is_processing {
            // 处理已释放的文件
            ctx.input(|i| {
                if !i.raw.dropped_files.is_empty() {
                    let paths: Vec<String> = i
                        .raw
                        .dropped_files
                        .iter()
                        .filter_map(|f| f.path.as_ref())
                        .filter_map(|p| p.to_str())
                        .map(|s| s.to_string())
                        .collect();
                    if !paths.is_empty() {
                        // 需要在闭包外处理
                        self.handle_drop(&paths);
                    }
                }
            });
        }
    }

    // ─── 结果展示界面（点击复制）───
    fn render_results(&mut self, ui: &mut egui::Ui) {
        ui.heading("📊 提取结果");
        ui.add_space(4.0);

        // 顶部工具栏
        ui.horizontal(|ui| {
            if ui.button("⬅ 返回主界面").clicked() {
                self.show_results = false;
            }
            let success_count = self.result_rows.iter().filter(|r| r.3 == "成功").count();
            let fail_count = self.result_rows.len() - success_count;
            ui.label(format!(
                "共 {} 条 | ✅ 成功 {} | ❌ 失败 {}",
                self.result_rows.len(),
                success_count,
                fail_count
            ));
        });
        ui.add_space(4.0);

        // 列标题
        let headers = ["序号", "文件名", "二维码内容", "状态"];
        let col_widths = [40.0, 200.0, ui.available_width() - 40.0 - 200.0 - 60.0 - 16.0, 60.0];

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                // 表头
                ui.horizontal(|ui| {
                    let mut x = 0.0;
                    for (i, h) in headers.iter().enumerate() {
                        ui.add_sized(
                            [col_widths[i], 20.0],
                            egui::Label::new(egui::RichText::new(*h).strong()),
                        );
                        x += col_widths[i];
                    }
                });
                ui.separator();

                // 表格滚动区
                let table_height = ui.available_height().max(100.0);
                egui::ScrollArea::vertical()
                    .id_salt("results_table")
                    .auto_shrink([false, false])
                    .max_height(table_height)
                    .show(ui, |ui| {
                        let text_style = egui::TextStyle::Body;
                        let row_height = ui.text_style_height(&text_style) + 4.0;

                        let mut clicked_value: Option<String> = None;

                        for (i, row) in self.result_rows.iter().enumerate() {
                            let (idx, filename, qr_data, status) = row;

                            let is_success = status == "成功";
                            let bg_color = if i % 2 == 0 {
                                egui::Color32::from_rgb(0xf8, 0xf9, 0xfa)
                            } else {
                                egui::Color32::WHITE
                            };

                            ui.horizontal(|ui| {
                                // 序号
                                let resp = ui.add_sized(
                                    [col_widths[0], row_height],
                                    egui::Label::new(idx.to_string()).sense(egui::Sense::click()),
                                );
                                if resp.clicked() {
                                    clicked_value = Some(idx.to_string());
                                }
                                resp.on_hover_text("点击复制");

                                // 文件名（可点击复制）
                                let resp = ui.add_sized(
                                    [col_widths[1], row_height],
                                    egui::Label::new(
                                        egui::RichText::new(filename.as_str())
                                            .color(egui::Color32::from_rgb(0x2c, 0x3e, 0x50)),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if resp.clicked() {
                                    clicked_value = Some(filename.clone());
                                }
                                resp.on_hover_text("点击复制");

                                // 二维码内容（可点击复制）
                                let resp = ui.add_sized(
                                    [col_widths[2], row_height],
                                    egui::Label::new(
                                        egui::RichText::new(qr_data.as_str())
                                            .color(egui::Color32::from_rgb(0x0d, 0x6e, 0x2d)),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if resp.clicked() {
                                    clicked_value = Some(qr_data.clone());
                                }
                                resp.on_hover_text("点击复制");

                                // 状态
                                let status_color = if is_success {
                                    egui::Color32::from_rgb(0x2e, 0xcc, 0x71)
                                } else {
                                    egui::Color32::from_rgb(0xe7, 0x4c, 0x3c)
                                };
                                let resp = ui.add_sized(
                                    [col_widths[3], row_height],
                                    egui::Label::new(
                                        egui::RichText::new(status.as_str()).color(status_color),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if resp.clicked() {
                                    clicked_value = Some(status.clone());
                                }
                                resp.on_hover_text("点击复制");
                            });

                            // 行间隔色
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    ui.cursor().left_top(),
                                    egui::vec2(ui.available_width(), row_height),
                                ),
                                0.0,
                                bg_color,
                            );
                        }

                        // 处理剪贴板复制（在循环外，避免借用冲突）
                        if let Some(val) = clicked_value {
                            ui.ctx().output_mut(|o| o.copied_text = val);
                        }
                    });
            });
    }
}

// ─── egui UI 实现 ───

impl eframe::App for InvoiceQrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 处理后台事件
        self.process_events();
        self.drag_and_drop(ctx);

        // 处理中需要持续刷新UI
        if self.is_processing {
            ctx.request_repaint();
        }

        // ── 窗口配置 ──
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_results && !self.is_processing {
                // ── 结果展示视图 ──
                self.render_results(ui);
            } else {
                // ── 主操作视图 ──
                // ── 标题 ──
                ui.heading("发票PDF二维码提取工具");
                ui.add_space(6.0);

            // ── 操作按钮区 ──
            ui.horizontal(|ui| {
                let add_file_btn = egui::Button::new("📂 添加文件");
                if ui.add_enabled(!self.is_processing, add_file_btn).clicked() {
                    self.add_files_dialog(ctx);
                }

                let add_folder_btn = egui::Button::new("📁 添加文件夹");
                if ui.add_enabled(!self.is_processing, add_folder_btn).clicked() {
                    self.add_folder_dialog(ctx);
                }

                let clear_btn = egui::Button::new("🗑 清空列表");
                if ui.add_enabled(!self.is_processing && !self.file_list.is_empty(), clear_btn).clicked()
                {
                    self.show_confirm_clear = true;
                }

                ui.separator();

                let up_btn = egui::Button::new("⬆ 上移");
                if ui.add_enabled(!self.is_processing, up_btn).clicked() {
                    // 通过上下文获取选中
                }

                let down_btn = egui::Button::new("⬇ 下移");
                if ui.add_enabled(!self.is_processing, down_btn).clicked() {}

                let remove_btn = egui::Button::new("❌ 移除选中");
                if ui.add_enabled(!self.is_processing, remove_btn).clicked() {}
            });

            ui.add_space(4.0);

            // ── 文件列表 ──
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.label("文件列表（拖拽PDF到此处）");
                    ui.separator();

                    let available_height = ui.available_height() - 8.0;
                    let list_height = available_height.max(50.0) * 0.4;

                    egui::ScrollArea::vertical()
                        .id_salt("file_list")
                        .auto_shrink([false, false])
                        .max_height(list_height)
                        .show(ui, |ui| {
                            if self.file_list.is_empty() {
                                ui.label("（无文件，点击上方按钮添加）");
                            } else {
                                let mut to_remove: Vec<usize> = Vec::new();
                                let mut to_move_up: Vec<usize> = Vec::new();
                                let mut to_move_down: Vec<usize> = Vec::new();

                                for (i, file_path) in self.file_list.iter().enumerate() {
                                    let _filename = Path::new(file_path)
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| file_path.clone());

                                    // 显示文件名（截断过长路径）
                                    let display = if file_path.len() > 80 {
                                        format!("{}...{}", &file_path[..40], &file_path[file_path.len()-37..])
                                    } else {
                                        file_path.clone()
                                    };

                                    ui.horizontal(|ui| {
                                        // 序号
                                        ui.label(format!("{}. ", i + 1));
                                        ui.label(&display);
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.add_enabled(!self.is_processing, egui::Button::new("❌").small()).clicked() {
                                                to_remove.push(i);
                                            }
                                            if ui.add_enabled(!self.is_processing && i > 0, egui::Button::new("↑").small()).clicked() {
                                                to_move_up.push(i);
                                            }
                                            if ui.add_enabled(!self.is_processing && i + 1 < self.file_list.len(), egui::Button::new("↓").small()).clicked() {
                                                to_move_down.push(i);
                                            }
                                        });
                                    });
                                }

                                // 执行操作
                                for &i in &to_move_up {
                                    self.move_up(i);
                                }
                                for &i in &to_move_down {
                                    self.move_down(i);
                                }
                                if !to_remove.is_empty() {
                                    self.remove_selected(&to_remove);
                                }
                            }
                        });
                });

            // ── 输出设置 ──
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("输出CSV:");
                let mut csv = self.csv_path.clone();
                ui.add_sized(
                    [ui.available_width() - 100.0, 0.0],
                    egui::TextEdit::singleline(&mut csv)
                        .desired_width(f32::INFINITY),
                );
                self.csv_path = csv;
                if ui.button("选择路径").clicked() {
                    self.select_csv_path(ctx);
                }
            });

            // ── 操作按钮 + 进度条 ──
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let start_btn = egui::Button::new("🚀 开始提取");
                let start_clicked = ui
                    .add_enabled(!self.is_processing && !self.file_list.is_empty(), start_btn)
                    .clicked();

                if start_clicked {
                    self.start_extract();
                }

                // 进度条
                let progress = if self.progress_total > 0 {
                    self.progress_current as f32 / self.progress_total as f32
                } else {
                    0.0
                };
                let pb = egui::ProgressBar::new(progress)
                    .show_percentage()
                    .desired_width(200.0);
                ui.add(pb);
                ui.label(&self.status_text);
            });

            // ── 日志区 ──
            ui.add_space(4.0);
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.label("运行日志");
                    ui.separator();

                    let log_height = ui.available_height().max(80.0);
                    egui::ScrollArea::vertical()
                        .id_salt("log_area")
                        .auto_shrink([false, false])
                        .max_height(log_height)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.logs)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(8)
                                    .desired_width(f32::INFINITY)
                                    .lock_focus(true)
                                    .interactive(false),
                            );
                        });
                });
            }  // else (main view) end
        });

        // ── 对话框 ──

        // 确认清空
        if self.show_confirm_clear {
            egui::Window::new("确认")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("确定要清空所有文件吗？");
                    ui.horizontal(|ui| {
                        if ui.button("确定").clicked() {
                            self.clear_list();
                            self.show_confirm_clear = false;
                        }
                        if ui.button("取消").clicked() {
                            self.show_confirm_clear = false;
                        }
                    });
                });
        }

        // 错误提示
        if let Some(err) = &self.show_error_dialog.clone() {
            egui::Window::new("提示")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(err);
                    if ui.button("确定").clicked() {
                        self.show_error_dialog = None;
                    }
                });
        }

        // 请求持续重绘（处理动画、进度等）
        if self.is_processing {
            ctx.request_repaint();
        }
    }
}
