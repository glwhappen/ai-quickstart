#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--headless") {
        std::process::exit(codex_quickstart_lib::headless(&args[pos + 1..]));
    }
    codex_quickstart_lib::run()
}
