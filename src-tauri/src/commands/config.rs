use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{DocumentMut, Item, Table, Value};

use super::tools::{self, codex as codex_tool};
use super::{config_path, user_codex_home};

pub const BASE_URL: &str = codex_tool::BASE_URL;
pub const DEFAULT_MODEL: &str = codex_tool::DEFAULT_MODEL;
pub const PROVIDER_ID: &str = codex_tool::PROVIDER_ID;
pub const PROVIDER_NAME: &str = codex_tool::PROVIDER_NAME;

#[derive(Serialize, Clone, Default)]
pub struct ConfigInfo {
    /// 我们自己的（隔离的）配置文件
    pub path: String,
    pub exists: bool,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub has_key: bool,
    pub key_masked: Option<String>,
    /// 用户自己的 ~/.codex/config.toml 里是否也有我们写的段（可选功能）
    pub desktop_configured: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct WriteResult {
    pub path: String,
    pub backup_path: Option<String>,
    pub changed: bool,
}

fn mask(key: &str) -> String {
    let k: Vec<char> = key.trim().chars().collect();
    if k.len() <= 12 {
        return "sk-***".to_string();
    }
    let head: String = k[..8].iter().collect();
    let tail: String = k[k.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

fn render(key: &str) -> String {
    // 具体格式由 tool 定义（Codex 用 model / model_provider / [model_providers.hsfp]）。
    tools::primary().render_config(key)
}

fn parse_key(text: &str) -> Option<String> {
    let doc: DocumentMut = text.parse().ok()?;
    let raw = doc
        .get("model_providers")?
        .as_table_like()?
        .get(PROVIDER_ID)?
        .as_table_like()?
        .get("http_headers")?
        .get("Authorization")?
        .as_str()?
        .trim_start_matches("Bearer ")
        .trim()
        .to_string();
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}

pub fn read_config() -> ConfigInfo {
    let path = config_path();
    let mut info = ConfigInfo {
        path: path.display().to_string(),
        ..Default::default()
    };
    if let Ok(text) = std::fs::read_to_string(&path) {
        info.exists = true;
        if let Ok(doc) = text.parse::<DocumentMut>() {
            info.model = doc
                .get("model")
                .and_then(|i| i.as_str())
                .map(|s| s.to_string());
            info.provider = doc
                .get("model_provider")
                .and_then(|i| i.as_str())
                .map(|s| s.to_string());
            if let Some(p) = doc
                .get("model_providers")
                .and_then(|i| i.as_table_like())
                .and_then(|t| t.get(PROVIDER_ID))
                .and_then(|i| i.as_table_like())
            {
                info.base_url = p
                    .get("base_url")
                    .and_then(|i| i.as_str())
                    .map(|s| s.to_string());
            }
        }
        info.has_key = parse_key(&text).is_some();
        info.key_masked = parse_key(&text).map(|k| mask(&k));
    }
    // 只读检查用户自己的 ~/.codex/config.toml（不写入）
    if let Ok(text) = std::fs::read_to_string(user_codex_home().join("config.toml")) {
        info.desktop_configured = parse_key(&text).is_some();
    }
    info
}

fn validate(key: &str) -> Result<&str, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Key 不能为空".to_string());
    }
    if !key.starts_with("sk-") || key.chars().count() < 12 {
        return Err("Key 格式不对：应以 sk- 开头，长度不少于 12 位".to_string());
    }
    if key.contains('"') || key.contains('\\') || key.contains('\n') {
        return Err("Key 里含有非法字符（引号/反斜杠/换行）".to_string());
    }
    Ok(key)
}

/// 写我们自己的 `CODEX_HOME/config.toml`：这个文件完全归本工具所有，
/// 用户的 `~/.codex/config.toml` 一个字节都不动。
pub fn write_isolated(key: &str) -> Result<WriteResult, String> {
    let key = validate(key)?;
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let existing = std::fs::read_to_string(&path).ok();
    let rendered = render(key);
    let changed = existing.as_deref() != Some(rendered.as_str());
    if changed {
        std::fs::write(&path, &rendered).map_err(|e| format!("写入 config.toml 失败：{e}"))?;
    }
    Ok(WriteResult {
        path: path.display().to_string(),
        backup_path: None,
        changed,
    })
}

/// 可选：把同一把 Key 也写进**桌面版读的** `~/.codex/config.toml`。
/// 必须先备份，且按 TOML 段做外科式修改——用户原有的 `[desktop]`、`[projects.*]` 等段逐字符保留。
pub fn write_desktop(key: &str) -> Result<WriteResult, String> {
    let key = validate(key)?;
    let path = user_codex_home().join("config.toml");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败：{e}"))?;
    }

    let existing = std::fs::read_to_string(&path).ok();
    let (mut doc, backup_path) = match existing.clone() {
        Some(text) => {
            let doc: DocumentMut = text.parse().map_err(|e| {
                format!(
                    "你现有的 {} 不是合法 TOML（{e}），为免破坏已中止，请先手动修好它",
                    path.display()
                )
            })?;
            let bak = backup(&path, &text)?;
            (doc, Some(bak))
        }
        None => (DocumentMut::new(), None),
    };

    doc["model"] = Item::Value(Value::from(DEFAULT_MODEL));
    doc["model_provider"] = Item::Value(Value::from(PROVIDER_ID));

    let providers = doc.entry("model_providers").or_insert_with(|| {
        // 隐式父表：不会额外打印一行空的 `[model_providers]`
        let mut t = Table::new();
        t.set_implicit(true);
        Item::Table(t)
    });
    let providers = providers.as_table_like_mut().ok_or_else(|| {
        "现有 config.toml 里的 model_providers 不是表，已中止以免破坏配置".to_string()
    })?;
    let entry = providers
        .entry(PROVIDER_ID)
        .or_insert(Item::Table(Table::new()));
    let p = entry.as_table_like_mut().ok_or_else(|| {
        "现有 config.toml 里的 model_providers.hsfp 不是表，已中止以免破坏配置".to_string()
    })?;
    p.insert("name", Item::Value(Value::from(PROVIDER_NAME)));
    p.insert("base_url", Item::Value(Value::from(BASE_URL)));
    p.insert("wire_api", Item::Value(Value::from("responses")));
    let mut headers = Table::new();
    headers.insert(
        "Authorization",
        Item::Value(Value::from(format!("Bearer {key}"))),
    );
    p.insert("http_headers", Item::Table(headers));

    let rendered = doc.to_string();
    let changed = existing.as_deref() != Some(rendered.as_str());
    if changed {
        std::fs::write(&path, rendered).map_err(|e| format!("写入 config.toml 失败：{e}"))?;
    }
    Ok(WriteResult {
        path: path.display().to_string(),
        backup_path: backup_path.map(|p| p.display().to_string()),
        changed,
    })
}

fn backup(path: &PathBuf, text: &str) -> Result<PathBuf, String> {
    let bak = path.with_extension(format!("toml.bak.{}", timestamp()));
    std::fs::write(&bak, text).map_err(|e| format!("备份 config.toml 失败：{e}"))?;
    Ok(bak)
}

/// 本地时间戳 `YYYYMMDDHHMMSS`（按东八区，避免引入时间库）。
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 8 * 3600;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant 的 days → (y, m, d)。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: &str = r#"# 我自己的注释
model = "gpt-5-codex"
approval_policy = "on-request"

[desktop]
theme = "dark"
conversation_detail = "STEPS_COMMANDS"

[projects."/home/me/proj"]
trust_level = "trusted"

[model_providers.other]
name = "Other"
base_url = "https://example.com/v1"
"#;

    /// 把 write_desktop 的 TOML 处理部分单独抽出来测（不碰真实文件系统）。
    fn surgically_edit(text: &str, key: &str) -> String {
        let mut doc: DocumentMut = text.parse().unwrap();
        doc["model"] = Item::Value(Value::from(DEFAULT_MODEL));
        doc["model_provider"] = Item::Value(Value::from(PROVIDER_ID));
        let providers = doc
            .entry("model_providers")
            .or_insert(Item::Table(Table::new()));
        let entry = providers
            .as_table_like_mut()
            .unwrap()
            .entry(PROVIDER_ID)
            .or_insert(Item::Table(Table::new()));
        let p = entry.as_table_like_mut().unwrap();
        p.insert("name", Item::Value(Value::from(PROVIDER_NAME)));
        p.insert("base_url", Item::Value(Value::from(BASE_URL)));
        p.insert("wire_api", Item::Value(Value::from("responses")));
        let mut h = Table::new();
        h.insert(
            "Authorization",
            Item::Value(Value::from(format!("Bearer {key}"))),
        );
        p.insert("http_headers", Item::Table(h));
        doc.to_string()
    }

    #[test]
    fn desktop_edit_keeps_other_sections_verbatim() {
        let out = surgically_edit(LEGACY, "sk-test-1234567890");
        assert!(out.contains("# 我自己的注释"));
        assert!(out.contains("[desktop]\ntheme = \"dark\"\nconversation_detail = \"STEPS_COMMANDS\""));
        assert!(out.contains("[projects.\"/home/me/proj\"]\ntrust_level = \"trusted\""));
        assert!(out.contains("[model_providers.other]"));
        assert!(out.contains("approval_policy = \"on-request\""));
        assert!(out.contains("model = \"deepseek-flash\""));
        assert!(out.contains("model_provider = \"hsfp\""));
        assert!(out.contains(
            "[model_providers.hsfp.http_headers]\nAuthorization = \"Bearer sk-test-1234567890\""
        ));
        assert_eq!(out.matches("[model_providers.hsfp]").count(), 1);
        assert!(!out.contains("env_key"));
    }

    #[test]
    fn isolated_config_is_idempotent() {
        let a = render("sk-abcdefgh12345");
        assert_eq!(a, render("sk-abcdefgh12345"));
        assert_eq!(a.matches("[model_providers.hsfp]").count(), 1);
        assert!(a.contains("wire_api = \"responses\""));
        assert_eq!(parse_key(&a).as_deref(), Some("sk-abcdefgh12345"));
    }

    #[test]
    fn rejects_bad_keys() {
        assert!(validate("").is_err());
        assert!(validate("abc").is_err());
        assert!(validate("sk-short").is_err());
        assert!(validate("sk-abcdefgh1\"2").is_err());
        assert!(validate("  sk-abcdefgh12345  ").is_ok());
    }

    #[test]
    fn mask_hides_middle() {
        let m = mask("sk-1234567890abcdef");
        assert!(m.starts_with("sk-12345"));
        assert!(m.ends_with("cdef"));
    }

    #[test]
    fn timestamp_shape() {
        let ts = timestamp();
        assert_eq!(ts.len(), 14, "{ts}");
        assert!(ts.starts_with("20"), "{ts}");
    }
}
