use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default, Clone)]
pub struct FormatterConfig {
    pub enable: Option<bool>,
    // Map of file extension (without dot, lowercase) to command template string
    // Example: rs = "rustfmt {file}", ts = "prettier --write {file}"
    pub commands: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct AppConfig {
    pub formatter: Option<FormatterConfig>,
}

fn global_config_path() -> PathBuf {
    let (cfg, _data, _state, _cache) = crate::xdg::ensure_app_dirs();
    cfg.join("config.toml")
}

fn local_config_path() -> PathBuf {
    Path::new(".afkaracode.toml").to_path_buf()
}

fn read_config(p: &Path) -> AppConfig {
    match fs::read_to_string(p) {
        Ok(s) => toml::from_str::<AppConfig>(&s).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

fn merge_maps(
    base: Option<HashMap<String, String>>,
    overlay: Option<HashMap<String, String>>,
) -> Option<HashMap<String, String>> {
    match (base, overlay) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(o)) => Some(o),
        (Some(mut b), Some(o)) => {
            for (k, v) in o.into_iter() {
                b.insert(k, v); // overlay overrides
            }
            Some(b)
        }
    }
}

fn merge_formatter(base: Option<FormatterConfig>, overlay: Option<FormatterConfig>) -> Option<FormatterConfig> {
    match (base, overlay) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(o)) => Some(o),
        (Some(b), Some(o)) => Some(FormatterConfig {
            enable: o.enable.or(b.enable),
            commands: merge_maps(b.commands, o.commands),
        }),
    }
}

fn merge_configs(global: AppConfig, local: AppConfig) -> AppConfig {
    AppConfig {
        formatter: merge_formatter(global.formatter, local.formatter),
    }
}

pub fn load_config() -> AppConfig {
    let global = read_config(&global_config_path());
    let local = read_config(&local_config_path());
    merge_configs(global, local)
}

impl AppConfig {
    pub fn formatter_enabled(&self) -> bool {
        self.formatter
            .as_ref()
            .and_then(|f| f.enable)
            .unwrap_or(true)
    }

    pub fn command_for_ext(&self, ext: &str) -> Option<String> {
        self.formatter
            .as_ref()
            .and_then(|f| f.commands.as_ref())
            .and_then(|m| m.get(&ext.to_ascii_lowercase()).cloned())
    }
}
