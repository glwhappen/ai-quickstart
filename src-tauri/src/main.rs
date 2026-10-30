// Windows 上必须是 console 子系统，`--headless`（主页/排障/脚本）才能被 cmd 等待并读到 stdout。
// 代价是双击运行会闪一下黑窗，所以进 GUI 前立刻把控制台窗口隐藏并释放。
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--headless") {
        std::process::exit(aiquick_lib::headless(&args[pos + 1..]));
    }
    #[cfg(target_os = "windows")]
    hide_console_for_gui();
    aiquick_lib::run()
}

/// 双击启动时把附带的控制台窗口藏起来（等价于 GUI 子系统的观感）。
#[cfg(target_os = "windows")]
fn hide_console_for_gui() {
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
        fn ShowWindow(hwnd: *mut core::ffi::c_void, cmd: i32) -> i32;
        fn FreeConsole() -> i32;
        fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
    }
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, 0); // SW_HIDE
            FreeConsole();
        }
        // 万一控制台还在（从终端里手动启动），把输出设成 UTF-8，中文不打码
        SetConsoleOutputCP(65001);
    }
}
