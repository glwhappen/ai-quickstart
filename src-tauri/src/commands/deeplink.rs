//! deep link：`aiquick://set-key?key=sk-xxx&group=半价`
//!
//! 网页端的 Key 卡片点「填入 AI 快速配置」就跳这个 URL，
//! 我们收到后把 Key 回填到输入框，用户不用「复制 → 粘贴」两趟。
//!
//! 三条硬约束（SPEC §3.4）：
//! 1. 参数名固定：scheme=`aiquick`、host/path=`set-key`、参数 `key` / `group`（官网那边要对齐，不改名）；
//! 2. **Key 绝不写进日志**——所有输出只用掩码；
//! 3. 非法/缺参数的 URL 一律忽略并给中性提示，不 panic。

use serde::Serialize;
use url::Url;

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct DeepLinkKey {
    pub key: String,
    pub group: Option<String>,
}

/// 日志里用的脱敏描述：只留尾 4 位 + 长度，**连 `sk-` 前缀都不出现**，
/// 这样对日志跑 `rg 'sk-'` 一定是空的（SPEC §3.4 验收项）。
pub fn mask(key: &str) -> String {
    let k: Vec<char> = key.trim().chars().collect();
    if k.len() <= 8 {
        return format!("尾号 ****（长度 {}）", k.len());
    }
    let tail: String = k[k.len() - 4..].iter().collect();
    format!("尾号 {tail}（长度 {}）", k.len())
}

fn valid_key(key: &str) -> bool {
    key.starts_with("sk-") && key.chars().count() >= 12 && key.chars().all(|c| !c.is_whitespace())
}

/// 解析 deep link。返回 `Err(中性提示)` 给界面显示，绝不包含 Key。
pub fn parse_set_key(raw: &str) -> Result<DeepLinkKey, String> {
    let url = Url::parse(raw).map_err(|_| "链接格式不对，已忽略".to_string())?;

    if url.scheme() != "aiquick" {
        return Err("不是本程序的链接，已忽略".to_string());
    }

    // `aiquick://set-key?...` 里 set-key 会落在 host；
    // 也兼容 `aiquick:///set-key?...`（path）这种写法。
    let target = format!(
        "{}{}",
        url.host_str().unwrap_or(""),
        url.path().trim_start_matches('/')
    )
    .to_lowercase();
    if target != "set-key" {
        return Err("链接里没有 set-key，已忽略".to_string());
    }

    let mut key: Option<String> = None;
    let mut group: Option<String> = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "key" => key = Some(v.trim().to_string()),
            "group" => {
                let g = v.trim().to_string();
                if !g.is_empty() {
                    group = Some(g);
                }
            }
            _ => {}
        }
    }

    let key = key.ok_or_else(|| "链接里没有 Key，已忽略".to_string())?;
    if !valid_key(&key) {
        return Err("链接里的 Key 格式不对（应以 sk- 开头），已忽略".to_string());
    }
    Ok(DeepLinkKey { key, group })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_and_group() {
        let r = parse_set_key("aiquick://set-key?key=sk-abcdefgh12345678&group=半价").unwrap();
        assert_eq!(r.key, "sk-abcdefgh12345678");
        assert_eq!(r.group.as_deref(), Some("半价"));
    }

    #[test]
    fn parses_percent_encoded_chinese_group() {
        let r = parse_set_key("aiquick://set-key?key=sk-abcdefgh12345678&group=%E5%8D%8A%E4%BB%B7")
            .unwrap();
        assert_eq!(r.group.as_deref(), Some("半价"));
    }

    #[test]
    fn parses_path_style() {
        let r = parse_set_key("aiquick:///set-key?key=sk-abcdefgh12345678").unwrap();
        assert_eq!(r.key, "sk-abcdefgh12345678");
        assert_eq!(r.group, None);
    }

    #[test]
    fn ignores_missing_key() {
        assert!(parse_set_key("aiquick://set-key?group=半价").is_err());
        assert!(parse_set_key("aiquick://set-key").is_err());
    }

    #[test]
    fn ignores_garbage_without_panic() {
        for bad in [
            "",
            "not a url",
            "://",
            "aiquick://",
            "aiquick://other?key=sk-abcdefgh12345678",
            "https://cc.hsfp.cn/#key",
            "aiquick://set-key?key=",
            "aiquick://set-key?key=abc",
            "aiquick://set-key?key=sk-abc%20def",
            "aiquick://set-key?key=%00%01",
            "aiquick://set-key?key=中文乱码",
            "aiquick://set-key?key=sk-abcdefgh12345678&group=",
        ] {
            let r = parse_set_key(bad);
            if let Ok(v) = &r {
                // 只有带合法 key 的才会成功
                assert!(valid_key(&v.key), "不该接受：{bad}");
            }
            // 错误信息里不允许出现 url 原文以外的敏感内容（尤其不能有 key）
            if let Err(e) = &r {
                assert!(!e.contains("sk-abcdefgh"), "{bad} -> {e}");
            }
        }
    }

    #[test]
    fn error_text_never_leaks_key() {
        let e = parse_set_key("aiquick://set-key?key=sk-SECRETSECRET1234&group=x")
            .err()
            .unwrap_or_default();
        assert!(!e.contains("SECRET"));
    }

    #[test]
    fn mask_never_contains_key_prefix() {
        let m = mask("sk-abcdefgh12345678");
        assert!(m.contains("5678"));
        assert!(!m.contains("sk-"), "日志掩码里不能出现 sk- 前缀：{m}");
        assert!(!m.contains("efgh1234"));
        // 短 key 也只给长度
        assert!(!mask("sk-short").contains("sk-"));
    }

    #[test]
    fn idempotent_overwrite_same_key() {
        // 重复点官网按钮：同一个 Key 解析结果一致（界面里只是覆盖输入框）
        let a = parse_set_key("aiquick://set-key?key=sk-abcdefgh12345678&group=半价").unwrap();
        let b = parse_set_key("aiquick://set-key?key=sk-abcdefgh12345678&group=半价").unwrap();
        assert_eq!(a, b);
    }
}
