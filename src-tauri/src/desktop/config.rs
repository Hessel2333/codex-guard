use crate::error::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::Read, path::Path};
use toml_edit::{DocumentMut, Item};

pub const MAX_CONFIG: u64 = 2 * 1024 * 1024;
pub const KEYS: [&str; 4] = [
    "developer_instructions",
    "model_reasoning_effort",
    "approval_policy",
    "sandbox_mode",
];

pub fn err(code: &str, message: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::new(code, message, detail.to_string())
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn revision(bytes: Option<&[u8]>) -> String {
    bytes.map(digest).unwrap_or_else(|| "missing".into())
}

// Refuse reparse points, including junctions, at every existing path component.
// A user may select the real directory instead; writes must not traverse links.
pub fn plain_path(path: &Path) -> Result<(), AppError> {
    for part in path.ancestors() {
        match fs::symlink_metadata(part) {
            Ok(meta) => {
                #[cfg(windows)]
                let reparse = {
                    use std::os::windows::fs::MetadataExt;
                    meta.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let reparse = false;
                if meta.file_type().is_symlink() || reparse {
                    return Err(err(
                        "DESKTOP_LINK",
                        "请选择不含链接的真实目录",
                        part.display(),
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(err("DESKTOP_PATH", "无法检查配置路径", e)),
        }
    }
    Ok(())
}
pub fn read(path: &Path) -> Result<Option<Vec<u8>>, AppError> {
    plain_path(path)?;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(err("DESKTOP_READ", "无法读取配置文件", e)),
    };
    let meta = file
        .metadata()
        .map_err(|e| err("DESKTOP_READ", "无法检查配置文件", e))?;
    if !meta.is_file() || meta.len() > MAX_CONFIG {
        return Err(err(
            "DESKTOP_SIZE",
            "配置文件不是普通文件或超过 2 MiB",
            path.display(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| err("DESKTOP_READ", "无法读取配置文件", e))?;
    if bytes.len() as u64 > MAX_CONFIG {
        return Err(err("DESKTOP_SIZE", "配置文件超过 2 MiB", path.display()));
    }
    Ok(Some(bytes))
}
pub fn parse(bytes: Option<&[u8]>) -> Result<DocumentMut, AppError> {
    let text = std::str::from_utf8(bytes.unwrap_or_default()).map_err(|_| {
        err(
            "DESKTOP_ENCODING",
            "配置文件不是 UTF-8 文本",
            "请先修复文件编码，原文件未修改。",
        )
    })?;
    text.trim_start_matches('\u{feff}')
        .parse::<DocumentMut>()
        // Parser errors may echo provider tokens from an unrelated line.
        .map_err(|_| {
            err(
                "DESKTOP_TOML",
                "配置文件格式有误",
                "请先修复 config.toml 的语法；原文件保持不变。",
            )
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub key: String,
    // null means remove this user-level setting, not write an empty string.
    pub value: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Setting {
    pub key: String,
    pub value: Option<String>,
    pub advanced: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub key: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
pub fn settings(doc: &DocumentMut) -> Vec<Setting> {
    KEYS.iter()
        .map(|key| Setting {
            key: (*key).into(),
            value: doc.get(key).and_then(Item::as_str).map(str::to_owned),
            advanced: doc
                .get(key)
                .is_some_and(|v| !v.is_none() && v.as_str().is_none()),
        })
        .collect()
}
pub fn validate(edits: &[Edit]) -> Result<(), AppError> {
    if edits.is_empty() || edits.len() > KEYS.len() {
        return Err(err(
            "DESKTOP_EDITS",
            "请选择需要修改的配置",
            "每次最多修改四个配置项。",
        ));
    }
    let mut seen = HashSet::new();
    for edit in edits {
        if !KEYS.contains(&edit.key.as_str()) || !seen.insert(&edit.key) {
            return Err(err("DESKTOP_EDITS", "配置项不受支持或重复", &edit.key));
        }
        if let Some(value) = &edit.value {
            let valid = match edit.key.as_str() {
                "developer_instructions" => value.len() <= 128 * 1024 && !value.contains('\0'),
                "model_reasoning_effort" => {
                    ["none", "minimal", "low", "medium", "high", "xhigh"].contains(&value.as_str())
                }
                "approval_policy" => ["on-request", "never"].contains(&value.as_str()),
                "sandbox_mode" => {
                    ["read-only", "workspace-write", "danger-full-access"].contains(&value.as_str())
                }
                _ => false,
            };
            if !valid {
                return Err(err("DESKTOP_VALUE", "配置值无效", &edit.key));
            }
        }
    }
    Ok(())
}
fn display(doc: &DocumentMut, key: &str) -> Option<String> {
    doc.get(key).filter(|v| !v.is_none()).map(|v| {
        v.as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| "高级配置".into())
    })
}
pub fn patch(bytes: Option<&[u8]>, edits: &[Edit]) -> Result<(Vec<u8>, Vec<Change>), AppError> {
    validate(edits)?;
    let mut doc = parse(bytes)?;
    let mut changes = Vec::new();
    for edit in edits {
        let before = display(&doc, &edit.key);
        let exists = doc.get(&edit.key).is_some_and(|v| !v.is_none());
        if (edit.value.is_none() && !exists)
            || (edit.value.is_some()
                && doc.get(&edit.key).and_then(Item::as_str) == edit.value.as_deref())
        {
            continue;
        }
        if let Some(value) = &edit.value {
            let decor = doc
                .get(&edit.key)
                .and_then(Item::as_value)
                .map(|v| v.decor().clone());
            let mut replacement = toml_edit::value(value);
            if let (Some(decor), Some(value)) = (decor, replacement.as_value_mut()) {
                *value.decor_mut() = decor;
            }
            doc[&edit.key] = replacement;
        } else {
            doc.remove(&edit.key);
        }
        changes.push(Change {
            key: edit.key.clone(),
            before,
            after: edit.value.clone(),
        });
    }
    if changes.is_empty() {
        return Ok((bytes.unwrap_or_default().to_vec(), changes));
    }
    let mut result = doc.to_string().into_bytes();
    if bytes.is_some_and(|b| b.starts_with(&[0xef, 0xbb, 0xbf])) {
        result.splice(0..0, [0xef, 0xbb, 0xbf]);
    }
    if result.len() as u64 > MAX_CONFIG {
        return Err(err(
            "DESKTOP_SIZE",
            "修改后的配置超过 2 MiB",
            "请减少自定义指令长度。",
        ));
    }
    Ok((result, changes))
}

// Compare only owned values, excluding decoration/comments. Never infer that an
// unchanged unrelated key is owned merely because a whole-file backup exists.
fn semantic(doc: &DocumentMut, key: &str) -> Option<String> {
    doc.get(key).filter(|v| !v.is_none()).map(|v| {
        v.as_str()
            .map(|s| format!("string:{s}"))
            .unwrap_or_else(|| v.to_string())
    })
}
pub fn restore(
    current: Option<&[u8]>,
    before: Option<&[u8]>,
    after: Option<&[u8]>,
    keys: &[String],
) -> Result<Option<Vec<u8>>, AppError> {
    if current == after {
        return Ok(before.map(<[u8]>::to_vec));
    }
    let mut doc = parse(current)?;
    let old = parse(before)?;
    let expected = parse(after)?;
    for key in keys {
        if !KEYS.contains(&key.as_str()) || semantic(&doc, key) != semantic(&expected, key) {
            return Err(err(
                "DESKTOP_CONFLICT",
                "配置已被其他操作修改，未覆盖",
                format!("{key} 与快照不一致，请刷新后核对。"),
            ));
        }
    }
    for key in keys {
        if let Some(item) = old.get(key) {
            doc[key] = item.clone();
        } else {
            doc.remove(key);
        }
    }
    let mut result = doc.to_string().into_bytes();
    if current.is_some_and(|b| b.starts_with(&[0xef, 0xbb, 0xbf])) {
        result.splice(0..0, [0xef, 0xbb, 0xbf]);
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edit(value: Option<&str>) -> Edit {
        Edit {
            key: KEYS[0].into(),
            value: value.map(str::to_owned),
        }
    }
    #[test]
    fn preserves_unrelated_tables_comments_and_multiline_values() {
        let original = b"# retain\r\nmodel = 'fixture'\r\n[model_providers.test]\r\nname = 'untouched' # keep\r\n";
        let text = "Chinese 中文\nquoted \"\"\" and ' and \\";
        let (next, _) = patch(Some(original), &[edit(Some(text))]).unwrap();
        let doc = parse(Some(&next)).unwrap();
        assert_eq!(doc[KEYS[0]].as_str(), Some(text));
        assert!(String::from_utf8(next.clone()).unwrap().contains("# keep"));
        assert_eq!(
            doc["model_providers"]["test"]["name"].as_str(),
            Some("untouched")
        );
        assert_eq!(
            restore(Some(&next), Some(original), Some(&next), &[KEYS[0].into()]).unwrap(),
            Some(original.to_vec())
        );
    }
    #[test]
    fn noop_retains_exact_bytes_and_missing_is_not_empty() {
        let src = b"\xef\xbb\xbfdeveloper_instructions = 'same'\r\n";
        assert_eq!(patch(Some(src), &[edit(Some("same"))]).unwrap().0, src);
        assert_ne!(revision(None), revision(Some(b"")));
    }
    #[test]
    fn changing_a_setting_preserves_its_existing_comments() {
        let src = b"# my notes\ndeveloper_instructions = 'old' # preference\nmodel='fixture'\n";
        let (next, _) = patch(Some(src), &[edit(Some("new"))]).unwrap();
        let text = String::from_utf8(next).unwrap();
        assert!(text.contains("# my notes"));
        assert!(text.contains("# preference"));
        assert_eq!(
            parse(Some(text.as_bytes())).unwrap()[KEYS[0]].as_str(),
            Some("new")
        );
    }
    #[test]
    fn restore_preserves_unrelated_user_edits_but_refuses_owned_conflicts() {
        let (next, _) = patch(Some(b"model='x'\n"), &[edit(Some("fixture"))]).unwrap();
        let mut drifted = next.clone();
        drifted.extend_from_slice(b"\n[projects.'D:/new']\ntrust_level='trusted'\n");
        let restored = restore(
            Some(&drifted),
            Some(b"model='x'\n"),
            Some(&next),
            &[KEYS[0].into()],
        )
        .unwrap()
        .unwrap();
        assert!(String::from_utf8(restored).unwrap().contains("D:/new"));
        let conflict = patch(Some(&drifted), &[edit(Some("user edit"))]).unwrap().0;
        assert_eq!(
            restore(
                Some(&conflict),
                Some(b"model='x'\n"),
                Some(&next),
                &[KEYS[0].into()]
            )
            .unwrap_err()
            .code,
            "DESKTOP_CONFLICT"
        );
    }
    #[test]
    fn rejects_unknown_duplicate_invalid_and_malformed() {
        assert!(validate(&[edit(Some("a")), edit(Some("b"))]).is_err());
        assert!(validate(&[Edit {
            key: "model_provider".into(),
            value: Some("x".into())
        }])
        .is_err());
        assert!(validate(&[Edit {
            key: KEYS[1].into(),
            value: Some("invalid".into())
        }])
        .is_err());
        assert!(patch(Some(b"[broken"), &[edit(Some("a"))]).is_err());
        assert!(validate(&[edit(Some(&"x".repeat(128 * 1024 + 1)))]).is_err());
    }
}
