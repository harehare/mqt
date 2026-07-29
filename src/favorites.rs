//! Persistence for named "favorite" queries, saved under the user's config
//! directory so they can be reused across sessions.

use crate::app::SavedQuery;
use std::path::PathBuf;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct FavoritesFile {
    #[serde(default)]
    query: Vec<SavedQuery>,
}

/// `<config dir>/mq-tui/queries.toml`, honoring `XDG_CONFIG_HOME`/`APPDATA`.
fn config_path() -> Option<PathBuf> {
    let dir = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else if cfg!(windows) {
        PathBuf::from(std::env::var("APPDATA").ok()?)
    } else {
        PathBuf::from(std::env::var("HOME").ok()?).join(".config")
    };
    Some(dir.join("mq-tui").join("queries.toml"))
}

/// Load saved queries from disk, or an empty list if missing/unparseable.
pub fn load() -> Vec<SavedQuery> {
    let Some(path) = config_path() else {
        return Vec::new();
    };
    load_from(&path)
}

/// Persist saved queries to disk, creating the config directory if needed.
pub fn save(queries: &[SavedQuery]) -> std::io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    save_to(&path, queries)
}

fn load_from(path: &PathBuf) -> Vec<SavedQuery> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    toml::from_str::<FavoritesFile>(&content)
        .map(|f| f.query)
        .unwrap_or_default()
}

fn save_to(path: &PathBuf, queries: &[SavedQuery]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = FavoritesFile {
        query: queries.to_vec(),
    };
    let content = toml::to_string_pretty(&file)
        .map_err(|err| std::io::Error::other(format!("Could not serialize queries: {err}")))?;
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_serialization() {
        let file = FavoritesFile {
            query: vec![
                SavedQuery {
                    name: "headings".to_string(),
                    query: ".h".to_string(),
                },
                SavedQuery {
                    name: "rust code".to_string(),
                    query: r#".code | select(.lang == "rust")"#.to_string(),
                },
            ],
        };
        let content = toml::to_string_pretty(&file).unwrap();
        let parsed: FavoritesFile = toml::from_str(&content).unwrap();
        assert_eq!(parsed.query.len(), 2);
        assert_eq!(parsed.query[0].name, "headings");
        assert_eq!(parsed.query[1].query, r#".code | select(.lang == "rust")"#);
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let _ = load();
    }

    #[test]
    fn test_save_to_and_load_from_temp_file_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "mq-tui-test-{}-{:?}.toml",
            std::process::id(),
            std::thread::current().id()
        ));
        let queries = vec![SavedQuery {
            name: "headings".to_string(),
            query: ".h".to_string(),
        }];

        save_to(&path, &queries).unwrap();
        let loaded = load_from(&path);
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "headings");
        assert_eq!(loaded[0].query, ".h");
    }

    #[test]
    fn test_load_from_nonexistent_path_returns_empty() {
        let path = std::env::temp_dir().join("mq-tui-test-does-not-exist.toml");
        assert!(load_from(&path).is_empty());
    }
}
