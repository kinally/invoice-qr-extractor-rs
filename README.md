# 发票PDF二维码提取工具 🧾

**Rust 重写版** — 从 PDF 发票文件中批量提取二维码内容，按顺序输出到 CSV 文件。

> 使用 Windows 内置 PDF 渲染引擎 (WinRT `Windows.Data.Pdf`)，无需额外安装 PDF 库。
> 二维码识别使用纯 Rust 库 `rqrr`，无外部依赖。

## 功能特性

- ✅ 支持拖拽 PDF 文件到窗口
- ✅ 支持选择文件夹，自动扫描所有 PDF
- ✅ 支持添加单个或多个 PDF 文件
- ✅ 支持列表排序（上移/下移/删除）
- ✅ 按列表顺序输出到 CSV（UTF-8 BOM 编码，Excel 可直接打开）
- ✅ 图形界面，操作直观
- ✅ 单文件 .exe，无需安装任何运行时

## 系统要求

- **操作系统**：Windows 10 或 Windows 11（需要 WinRT `Windows.Data.Pdf` API）
- 无需安装 .NET 运行时、无需 Python、无需任何第三方运行时

## 快速开始

### 方式一：下载预编译的 exe

从 [Releases](https://github.com/kinally/invoice-qr-extractor-rs/releases) 页面下载最新版本，
解压后直接运行 `InvoiceQRExtractor.exe`。

### 方式二：自行编译

```bash
# 安装 Rust（如果未安装）
# 访问 https://rustup.rs 下载安装

# 编译
cd invoice-qr-extractor-rs
cargo build --release

# 编译产物在 target/release/InvoiceQRExtractor.exe
```

### 编译为最小体积

```bash
# 使用 LTO 和 strip 优化体积
cargo build --release

# 可选：使用 upx 进一步压缩
upx --best target/release/InvoiceQRExtractor.exe
```

## 使用说明

1. **添加文件**：点击「添加文件」选择 PDF，或直接拖入窗口
2. **添加文件夹**：自动扫描文件夹内所有 PDF
3. **调整顺序**：选中文件后点「↑」/「↓」
4. **开始提取**：点击「开始提取」，等待处理完成
5. **查看结果**：自动打开输出文件夹，CSV 可直接用 Excel 打开

## 输出格式

| 序号 | 文件名 | 二维码内容 | 状态 |
|------|--------|------------|------|
| 1 | 发票001.pdf | 发票二维码数据... | 成功 |
| 2 | 发票002.pdf | | 未识别到二维码 |

## 技术栈

| 组件 | 库 | 说明 |
|------|-----|------|
| GUI 框架 | egui / eframe | 轻量级即时模式 GUI |
| PDF 渲染 | Windows.Data.Pdf (WinRT) | Windows 10+ 内置 PDF 引擎 |
| 二维码识别 | rqrr | 纯 Rust QR 码解码器 |
| 图片处理 | image | 灰度转换、对比度增强 |
| CSV 输出 | csv | 写入标准 CSV 格式 |
| 文件对话框 | rfd | 原生 Windows 文件对话框 |

## 构建体积对比

| 方式 | 体积 | 说明 |
|------|------|------|
| Debug 编译 | ~20 MB | 未优化 |
| Release 编译 | ~5 MB | LTO 优化 |
| Release + UPX | ~2 MB | 压缩后最小体积 |

## 许可证

MIT

---

Made with ❤️ by [Kinally](https://github.com/kinally)
