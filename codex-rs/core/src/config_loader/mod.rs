mod macos;

use crate::config::CONFIG_TOML_FILE;
use crate::git_info::get_git_repo_root;
use macos::load_managed_admin_config_layer;
use std::env;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use toml::Value as TomlValue;

#[cfg(unix)]
const CODEX_MANAGED_CONFIG_SYSTEM_PATH: &str = "/etc/codex/managed_config.toml";

#[derive(Debug)]
pub(crate) struct LoadedConfigLayers {
    pub base: TomlValue,
    pub managed_config: Option<TomlValue>,
    pub managed_preferences: Option<TomlValue>,
}

#[derive(Debug, Default)]
pub(crate) struct LoaderOverrides {
    pub managed_config_path: Option<PathBuf>,
    #[cfg(target_os = "macos")]
    pub managed_preferences_base64: Option<String>,
}

// Configuration layering pipeline (top overrides bottom):
//
//        +-------------------------+
//        | Managed preferences (*) |
//        +-------------------------+
//                    ^
//                    |
//        +-------------------------+
//        |  managed_config.toml   |
//        +-------------------------+
//                    ^
//                    |
//        +-------------------------+
//        |    config.toml (base)   |
//        +-------------------------+
//
// (*) Only available on macOS via managed device profiles.

pub async fn load_config_as_toml(codex_home: &Path) -> io::Result<TomlValue> {
    load_config_as_toml_with_overrides(codex_home, LoaderOverrides::default(), None).await
}

fn default_empty_table() -> TomlValue {
    TomlValue::Table(Default::default())
}

pub(crate) async fn load_config_layers_with_overrides(
    codex_home: &Path,
    overrides: LoaderOverrides,
    repo_cwd_override: Option<&Path>,
) -> io::Result<LoadedConfigLayers> {
    load_config_layers_internal(codex_home, overrides, repo_cwd_override).await
}

async fn load_config_as_toml_with_overrides(
    codex_home: &Path,
    overrides: LoaderOverrides,
    repo_cwd_override: Option<&Path>,
) -> io::Result<TomlValue> {
    let layers = load_config_layers_internal(codex_home, overrides, repo_cwd_override).await?;
    Ok(apply_managed_layers(layers))
}

async fn load_config_layers_internal(
    codex_home: &Path,
    overrides: LoaderOverrides,
    repo_cwd_override: Option<&Path>,
) -> io::Result<LoadedConfigLayers> {
    #[cfg(target_os = "macos")]
    let LoaderOverrides {
        managed_config_path,
        managed_preferences_base64,
    } = overrides;

    #[cfg(not(target_os = "macos"))]
    let LoaderOverrides {
        managed_config_path,
    } = overrides;

    let managed_config_path =
        managed_config_path.unwrap_or_else(|| managed_config_default_path(codex_home));

    let user_config_path = codex_home.join(CONFIG_TOML_FILE);
    let user_config = read_config_from_path(&user_config_path, true).await?;
    let repo_config_path = determine_repo_config_path(repo_cwd_override)?;
    let repo_config = if let Some(path) = repo_config_path.as_deref() {
        let value = read_config_from_path(path, false).await?;
        if value.is_some() {
            tracing::info!("Loading repository config from {}", path.display());
        }
        value
    } else {
        None
    };
    let managed_config = read_config_from_path(&managed_config_path, false).await?;

    #[cfg(target_os = "macos")]
    let managed_preferences =
        load_managed_admin_config_layer(managed_preferences_base64.as_deref()).await?;

    #[cfg(not(target_os = "macos"))]
    let managed_preferences = load_managed_admin_config_layer(None).await?;

    let mut base = user_config.unwrap_or_else(default_empty_table);
    if let Some(repo_overlay) = repo_config {
        merge_toml_values(&mut base, &repo_overlay);
    }

    Ok(LoadedConfigLayers {
        base,
        managed_config,
        managed_preferences,
    })
}

async fn read_config_from_path(
    path: &Path,
    log_missing_as_info: bool,
) -> io::Result<Option<TomlValue>> {
    match fs::read_to_string(path).await {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                tracing::error!("Failed to parse {}: {err}", path.display());
                Err(io::Error::new(io::ErrorKind::InvalidData, err))
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if log_missing_as_info {
                tracing::info!("{} not found, using defaults", path.display());
            } else {
                tracing::debug!("{} not found", path.display());
            }
            Ok(None)
        }
        Err(err) => {
            tracing::error!("Failed to read {}: {err}", path.display());
            Err(err)
        }
    }
}

/// Merge config `overlay` into `base`, giving `overlay` precedence.
pub(crate) fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        for (key, value) in overlay_table {
            if let Some(existing) = base_table.get_mut(key) {
                merge_toml_values(existing, value);
            } else {
                base_table.insert(key.clone(), value.clone());
            }
        }
    } else {
        *base = overlay.clone();
    }
}

fn managed_config_default_path(codex_home: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        let _ = codex_home;
        PathBuf::from(CODEX_MANAGED_CONFIG_SYSTEM_PATH)
    }

    #[cfg(not(unix))]
    {
        codex_home.join("managed_config.toml")
    }
}

fn determine_repo_config_path(repo_cwd_override: Option<&Path>) -> io::Result<Option<PathBuf>> {
    let search_dir = resolve_repo_search_dir(repo_cwd_override)?;
    let repo_root = get_git_repo_root(&search_dir).unwrap_or(search_dir);
    let candidate = repo_root.join(".codex").join(CONFIG_TOML_FILE);

    match std::fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.file_type().is_file() || metadata.file_type().is_symlink() => {
            Ok(Some(candidate))
        }
        Ok(_) => Ok(None),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn resolve_repo_search_dir(repo_cwd_override: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(dir) = repo_cwd_override {
        if dir.is_absolute() {
            return Ok(dir.to_path_buf());
        }
        let mut cwd = env::current_dir()?;
        cwd.push(dir);
        return Ok(cwd);
    }

    env::current_dir()
}

fn apply_managed_layers(layers: LoadedConfigLayers) -> TomlValue {
    let LoadedConfigLayers {
        mut base,
        managed_config,
        managed_preferences,
    } = layers;

    for overlay in [managed_config, managed_preferences].into_iter().flatten() {
        merge_toml_values(&mut base, &overlay);
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn merges_managed_config_layer_on_top() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

        std::fs::write(
            tmp.path().join(CONFIG_TOML_FILE),
            r#"foo = 1

[nested]
value = "base"
"#,
        )
        .expect("write base");
        std::fs::write(
            &managed_path,
            r#"foo = 2

[nested]
value = "managed_config"
extra = true
"#,
        )
        .expect("write managed config");

        let overrides = LoaderOverrides {
            managed_config_path: Some(managed_path),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides, None)
            .await
            .expect("load config");
        let table = loaded.as_table().expect("top-level table expected");

        assert_eq!(table.get("foo"), Some(&TomlValue::Integer(2)));
        let nested = table
            .get("nested")
            .and_then(|v| v.as_table())
            .expect("nested");
        assert_eq!(
            nested.get("value"),
            Some(&TomlValue::String("managed_config".to_string()))
        );
        assert_eq!(nested.get("extra"), Some(&TomlValue::Boolean(true)));
    }

    #[tokio::test]
    async fn returns_empty_when_all_layers_missing() {
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");
        let overrides = LoaderOverrides {
            managed_config_path: Some(managed_path),
            #[cfg(target_os = "macos")]
            managed_preferences_base64: None,
        };

        let layers = load_config_layers_with_overrides(tmp.path(), overrides, Some(tmp.path()))
            .await
            .expect("load layers");
        let base_table = layers.base.as_table().expect("base table expected");
        assert!(
            base_table.is_empty(),
            "expected empty base layer when configs missing"
        );
        assert!(
            layers.managed_config.is_none(),
            "managed config layer should be absent when file missing"
        );

        #[cfg(not(target_os = "macos"))]
        {
            let loaded = load_config_as_toml(tmp.path()).await.expect("load config");
            let table = loaded.as_table().expect("top-level table expected");
            assert!(
                table.is_empty(),
                "expected empty table when configs missing"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn managed_preferences_take_highest_precedence() {
        use base64::Engine;

        let managed_payload = r#"
[nested]
value = "managed"
flag = false
"#;
        let encoded = base64::prelude::BASE64_STANDARD.encode(managed_payload.as_bytes());
        let tmp = tempdir().expect("tempdir");
        let managed_path = tmp.path().join("managed_config.toml");

        std::fs::write(
            tmp.path().join(CONFIG_TOML_FILE),
            r#"[nested]
value = "base"
"#,
        )
        .expect("write base");
        std::fs::write(
            &managed_path,
            r#"[nested]
value = "managed_config"
flag = true
"#,
        )
        .expect("write managed config");

        let overrides = LoaderOverrides {
            managed_config_path: Some(managed_path),
            managed_preferences_base64: Some(encoded),
        };

        let loaded = load_config_as_toml_with_overrides(tmp.path(), overrides, None)
            .await
            .expect("load config");
        let nested = loaded
            .get("nested")
            .and_then(|v| v.as_table())
            .expect("nested table");
        assert_eq!(
            nested.get("value"),
            Some(&TomlValue::String("managed".to_string()))
        );
        assert_eq!(nested.get("flag"), Some(&TomlValue::Boolean(false)));
    }

    #[tokio::test]
    async fn repo_config_overrides_user_config() {
        let tmp = tempdir().expect("tempdir");
        let codex_home = tmp.path().join("home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::write(codex_home.join(CONFIG_TOML_FILE), r#"notify = ["global"]"#)
            .expect("write base config");

        let repo_root = tmp.path().join("repo");
        std::fs::create_dir_all(repo_root.join(".git")).expect("create .git dir");
        let repo_codex = repo_root.join(".codex");
        std::fs::create_dir_all(&repo_codex).expect("create repo .codex");
        std::fs::write(repo_codex.join(CONFIG_TOML_FILE), r#"notify = ["repo"]"#)
            .expect("write repo config");
        let nested = repo_root.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested dir");

        let layers = load_config_layers_with_overrides(
            &codex_home,
            LoaderOverrides::default(),
            Some(nested.as_path()),
        )
        .await
        .expect("load layers");
        let notify = layers
            .base
            .get("notify")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str());

        assert_eq!(notify, Some("repo"));
    }

    #[tokio::test]
    async fn repo_config_without_git_root_uses_current_dir() {
        let tmp = tempdir().expect("tempdir");
        let codex_home = tmp.path().join("home");
        std::fs::create_dir_all(&codex_home).expect("create codex home");
        std::fs::write(codex_home.join(CONFIG_TOML_FILE), r#"notify = ["global"]"#)
            .expect("write base config");

        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        let repo_codex = project_dir.join(".codex");
        std::fs::create_dir_all(&repo_codex).expect("create project .codex");
        std::fs::write(repo_codex.join(CONFIG_TOML_FILE), r#"notify = ["local"]"#)
            .expect("write repo config");

        let layers = load_config_layers_with_overrides(
            &codex_home,
            LoaderOverrides::default(),
            Some(project_dir.as_path()),
        )
        .await
        .expect("load layers");
        let notify = layers
            .base
            .get("notify")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str());

        assert_eq!(notify, Some("local"));
    }
}
