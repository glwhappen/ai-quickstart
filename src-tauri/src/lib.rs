mod commands;

use commands::config::{self, ConfigInfo, WriteResult};
use commands::detect::{self, EnvStatus};
use commands::launch;
use commands::prepare;
use commands::test::{self, TestResult};

#[tauri::command]
async fn check_environment() -> EnvStatus {
    detect::check_environment()
}

/// 下载并解压 Codex 到我们自己的目录（不安装、不写 PATH）。
#[tauri::command]
async fn prepare_codex(app: tauri::AppHandle) -> Result<String, String> {
    prepare::prepare_codex(app).await
}

#[tauri::command]
async fn read_config() -> ConfigInfo {
    config::read_config()
}

/// 写我们自己的（隔离的）config.toml；`also_desktop` 为真时才额外写用户的 `~/.codex`（带备份、外科式）。
#[tauri::command]
async fn write_config(key: String, also_desktop: bool) -> Result<WriteResult, String> {
    let r = config::write_isolated(&key)?;
    if also_desktop {
        let d = config::write_desktop(&key)?;
        return Ok(WriteResult {
            path: r.path,
            backup_path: d.backup_path,
            changed: r.changed || d.changed,
        });
    }
    Ok(r)
}

#[tauri::command]
async fn run_test(key: Option<String>) -> TestResult {
    test::run_test(key).await
}

#[tauri::command]
async fn launch_codex() -> Result<String, String> {
    launch::launch_codex()
}

#[tauri::command]
async fn cleanup() -> Result<String, String> {
    launch::cleanup()
}

#[tauri::command]
async fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_environment,
            prepare_codex,
            read_config,
            write_config,
            run_test,
            launch_codex,
            cleanup,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 无界面模式：与界面共用同一套函数，供自动化验收（假 HOME、真 Key、错 Key）使用。
/// 用法：`codex-quickstart --headless <check|prepare|config|test|launch|cleanup> [--key sk-...] [--also-desktop]`
pub fn headless(args: &[String]) -> i32 {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let key = args
        .iter()
        .position(|a| a == "--key")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let also_desktop = args.iter().any(|a| a == "--also-desktop");
    let out = match cmd {
        "check" => Ok(serde_json::to_string_pretty(&detect::check_environment()).unwrap()),
        "prepare" => prepare::prepare_sync().map(|v| {
            serde_json::json!({ "version": v }).to_string()
        }),
        "config" => match key {
            Some(k) => {
                let r = config::write_isolated(&k).and_then(|r| {
                    if also_desktop {
                        config::write_desktop(&k).map(|d| WriteResult {
                            path: r.path.clone(),
                            backup_path: d.backup_path,
                            changed: r.changed || d.changed,
                        })
                    } else {
                        Ok(r)
                    }
                });
                r.and_then(|r| serde_json::to_string_pretty(&r).map_err(|e| e.to_string()))
            }
            None => serde_json::to_string_pretty(&config::read_config()).map_err(|e| e.to_string()),
        },
        "test" => Ok(serde_json::to_string_pretty(&test::run_test_blocking(key)).unwrap()),
        "launch" => launch::launch_codex(),
        "cleanup" => launch::cleanup(),
        other => Err(format!("未知子命令：{other}")),
    };
    match out {
        Ok(s) => {
            println!("{s}");
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
