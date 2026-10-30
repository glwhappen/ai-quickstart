#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--headless") {
        // 绿色版是 GUI 子系统程序（双击不弹黑窗），代价是没有 stdout。
        // `--headless` 被脚本/主页调用时要能读到输出，所以先挂到父进程的控制台。
        #[cfg(target_os = "windows")]
        attach_parent_console();
        std::process::exit(codex_quickstart_lib::headless(&args[pos + 1..]));
    }
    codex_quickstart_lib::run()
}

/// 把标准输出/错误接到父进程的控制台（仅 Windows，仅 --headless 时调用）。
#[cfg(target_os = "windows")]
fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    unsafe extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
        fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
    }
    unsafe {
        // 已经在控制台里时 AttachConsole 会失败，这时什么都不用做。
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            SetConsoleOutputCP(65001); // UTF-8，免得中文乱码
        }
    }
}
