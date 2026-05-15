use anyhow::{Context, Result};
use std::path::Path;
use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
use windows::Graphics::Imaging::{BitmapDecoder, BitmapEncoder, BitmapPixelFormat, SoftwareBitmap};
use windows::Storage::StorageFile;
use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};

/// 从PDF中提取二维码的结果
#[derive(Debug, Clone)]
pub struct QrResult {
    pub page: u32,
    pub data: String,
    pub error: String,
}

// ─── 二维码提取核心逻辑 ───

/// 从PDF文件中提取所有页面的二维码
pub fn extract_qr_from_pdf(pdf_path: &str) -> Vec<QrResult> {
    let mut results = Vec::new();

    // 打开PDF文档
    let doc = match open_pdf_document(pdf_path) {
        Ok(doc) => doc,
        Err(e) => {
            results.push(QrResult {
                page: 1,
                data: String::new(),
                error: format!("无法打开PDF: {}", e),
            });
            return results;
        }
    };

    let page_count = match doc.PageCount() {
        Ok(c) => c,
        Err(e) => {
            results.push(QrResult {
                page: 1,
                data: String::new(),
                error: format!("获取PDF页数失败: {}", e),
            });
            return results;
        }
    };

    for page_idx in 0..page_count {
        let page = match doc.GetPage(page_idx) {
            Ok(p) => p,
            Err(e) => {
                results.push(QrResult {
                    page: page_idx + 1,
                    data: String::new(),
                    error: format!("获取第{}页失败: {}", page_idx + 1, e),
                });
                continue;
            }
        };

        // 渲染页面为图像 (300 DPI)
        let img_bytes = match render_page_to_png_bytes(&page) {
            Ok(bytes) => bytes,
            Err(e) => {
                results.push(QrResult {
                    page: page_idx + 1,
                    data: String::new(),
                    error: format!("渲染第{}页失败: {}", page_idx + 1, e),
                });
                continue;
            }
        };

        // 尝试多种方式识别二维码
        let qr_data = match decode_qr_from_bytes(&img_bytes) {
            Some(data) => data,
            None => {
                results.push(QrResult {
                    page: page_idx + 1,
                    data: String::new(),
                    error: "未识别到二维码".to_string(),
                });
                continue;
            }
        };

        results.push(QrResult {
            page: page_idx + 1,
            data: qr_data,
            error: String::new(),
        });
    }

    results
}

/// 打开PDF文档
fn open_pdf_document(pdf_path: &str) -> Result<PdfDocument> {
    let path = Path::new(pdf_path);
    let path_str = path
        .as_os_str()
        .to_str()
        .context("PDF路径包含非法字符")?;
    let hstr = windows::core::HSTRING::from(path_str);
    let file = StorageFile::GetFileFromPathAsync(&hstr)?.get()?;
    let doc = PdfDocument::LoadFromFileAsync(&file)?.get()?;
    Ok(doc)
}

/// 将PDF页面渲染为PNG字节数据 (300 DPI)
fn render_page_to_png_bytes(page: &windows::Data::Pdf::PdfPage) -> Result<Vec<u8>> {
    // 获取页面尺寸 (单位: 1/96 英寸)
    let size = page.Size()?;
    let src_width = size.Width;
    let src_height = size.Height;

    // 300 DPI的计算: 默认72 DPI, 缩放因子 = 300/72 ≈ 4.17
    let scale = 300.0 / 72.0;
    let dest_width = (src_width * scale) as u32;
    let dest_height = (src_height * scale) as u32;

    // 设置渲染选项 (高DPI)
    let options = PdfPageRenderOptions::new()?;
    options.SetDestinationWidth(dest_width)?;
    options.SetDestinationHeight(dest_height)?;

    // 渲染到内存流 (WinRT BMP格式)
    let stream = InMemoryRandomAccessStream::new()?;
    page.RenderToStreamAsync(&stream)?.get()?;

    // 回到流开头
    stream.Seek(0)?;

    // 使用BitmapDecoder解码BMP数据
    let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
    let frame = decoder.GetFrameAsync(0)?.get()?;
    let software_bitmap = frame.GetSoftwareBitmapAsync()?.get()?;

    // 转换为RGBA8格式以便image crate处理
    let rgba_bitmap = SoftwareBitmap::Convert(&software_bitmap, BitmapPixelFormat::Rgba8)?;

    // 将SoftwareBitmap编码为PNG字节
    let out_stream = InMemoryRandomAccessStream::new()?;
    let encoder = BitmapEncoder::CreateAsync(
        BitmapEncoder::PngEncoderId()?,
        &out_stream,
    )?.get()?;

    encoder.SetSoftwareBitmap(&rgba_bitmap)?;
    encoder.FlushAsync()?.get()?;

    // 读取PNG字节
    out_stream.Seek(0)?;
    let reader = DataReader::CreateDataReader(&out_stream)?;
    let out_size = out_stream.Size()? as u32;
    reader.LoadAsync(out_size)?.get()?;

    let mut buffer = vec![0u8; out_size as usize];
    reader.ReadBytes(&mut buffer)?;

    Ok(buffer)
}

// ─── 二维码解码 ───

/// 从图像字节数据中解码二维码 (尝试多种策略)
fn decode_qr_from_bytes(img_bytes: &[u8]) -> Option<String> {
    // 1. 直接加载并尝试解码
    let img = image::load_from_memory(img_bytes).ok()?;

    // 2. 尝试在原始图像上解码
    if let Some(data) = decode_qr_from_image(&img) {
        return Some(data);
    }

    // 3. 转灰度再试
    let gray = img.grayscale();
    if let Some(data) = decode_qr_from_image(&gray) {
        return Some(data);
    }

    // 4. 增强对比度再试
    let adjusted = img.adjust_contrast(2.0);
    if let Some(data) = decode_qr_from_image(&adjusted) {
        return Some(data);
    }

    // 5. 放大再试 (2x)
    let enlarged = image::imageops::resize(
        &img,
        img.width() * 2,
        img.height() * 2,
        image::imageops::FilterType::Lanczos3,
    );
    let enlarged_img = image::DynamicImage::ImageRgba8(enlarged);
    if let Some(data) = decode_qr_from_image(&enlarged_img) {
        return Some(data);
    }

    None
}

/// 使用rqrr从DynamicImage中解码二维码
fn decode_qr_from_image(img: &image::DynamicImage) -> Option<String> {
    let gray = img.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();

    for grid in &grids {
        if let Ok((_, content)) = grid.decode() {
            return Some(content);
        }
    }

    None
}

// ─── 批量处理 ───

/// 批量处理PDF文件，输出CSV
/// 返回: (成功识别的文件数, 结果行数据)
pub fn process_pdfs(
    pdf_paths: &[String],
    output_csv: &str,
    progress_callback: impl Fn(usize, usize),
    log_callback: impl Fn(&str),
) -> (usize, Vec<(usize, String, String, String)>) {
    let mut rows: Vec<(usize, String, String, String)> = Vec::new();
    let mut success_count = 0;
    let total = pdf_paths.len();

    for (idx, pdf_path) in pdf_paths.iter().enumerate() {
        let filename = Path::new(pdf_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| pdf_path.clone());

        log_callback(&format!("[{}/{}] 处理: {}", idx + 1, total, filename));

        let qr_results = extract_qr_from_pdf(pdf_path);

        if !qr_results.is_empty() {
            let mut found = false;
            for qr in &qr_results {
                if !qr.data.is_empty() {
                    rows.push((
                        idx + 1,
                        filename.clone(),
                        qr.data.clone(),
                        "成功".to_string(),
                    ));
                    success_count += 1;
                    found = true;
                    break;
                }
            }
            if !found {
                let err_msg = if qr_results[0].error.is_empty() {
                    "识别失败".to_string()
                } else {
                    qr_results[0].error.clone()
                };
                rows.push((idx + 1, filename, String::new(), err_msg));
            }
        } else {
            rows.push((idx + 1, filename, String::new(), "无返回结果".to_string()));
        }

        progress_callback(idx + 1, total);
    }

    // 写入CSV
    match write_csv_with_bom(output_csv, &rows) {
        Ok(()) => {
            log_callback(&format!(
                "\n✅ 完成！共处理 {} 个文件，成功识别 {} 个",
                total, success_count
            ));
            log_callback(&format!("📄 结果已保存: {}", output_csv));
        }
        Err(e) => {
            log_callback(&format!("❌ CSV写入失败: {}", e));
        }
    }

    (success_count, rows)
}

// ─── CSV 写入 ───

const CSV_HEADERS: &[&str] = &["序号", "文件名", "二维码内容", "状态"];

/// 写入CSV文件 (UTF-8 BOM)
fn write_csv_with_bom(
    path: &str,
    rows: &[(usize, String, String, String)],
) -> Result<()> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b',')
        .from_writer(Vec::new());

    // 写表头
    wtr.write_record(CSV_HEADERS)
        .context("写入CSV表头失败")?;

    // 写数据行
    for (idx, filename, qr_data, status) in rows {
        wtr.write_record(&[
            idx.to_string(),
            filename.clone(),
            qr_data.clone(),
            status.clone(),
        ])
        .context("写入CSV数据行失败")?;
    }

    wtr.flush()?;
    let csv_content = wtr.into_inner()?;

    // 添加UTF-8 BOM (用于Excel直接打开)
    let mut bom_content = vec![0xEFu8, 0xBB, 0xBF];
    bom_content.extend_from_slice(&csv_content);

    std::fs::write(path, &bom_content).context("写入CSV文件失败")?;

    Ok(())
}
