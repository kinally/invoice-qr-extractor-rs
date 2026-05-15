@echo off
chcp 65001 >nul
echo 发票PDF二维码提取工具 - Rust 编译脚本
echo ========================================
echo.

:: 检查 Rust 是否安装
where rustc >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ 未检测到 Rust 编译器！
    echo 请访问 https://rustup.rs 安装 Rust
    pause
    exit /b 1
)

echo ✅ Rust 已安装: 
rustc --version
echo.

:: 编译 Release 版本
echo 🔨 正在编译 Release 版本...
cargo build --release
if %errorlevel% neq 0 (
    echo ❌ 编译失败，请检查错误信息
    pause
    exit /b 1
)

echo.
echo ✅ 编译成功！
echo 📦 可执行文件: target\release\InvoiceQRExtractor.exe
echo.

:: 显示文件大小
for %%i in (target\release\InvoiceQRExtractor.exe) do (
    echo 📏 文件大小: %%~zi 字节
)

echo.
echo 💡 提示: 可使用 UPX 进一步压缩体积
echo    upx --best target\release\InvoiceQRExtractor.exe
echo.

pause
