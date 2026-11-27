use crate::local_files::Directory;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use traceveil::{
    Error, Result,
    diagnostics::{RULES, Settings},
};
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub session_dir: Option<PathBuf>,
    #[serde(default)]
    pub rule_thresholds: BTreeMap<String, usize>,
    #[serde(default)]
    pub suppressed_rules: Vec<String>,
}
pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| {
            Error::new(
                "CONFIGURATION_INVALID",
                "Set HOME or USERPROFILE, or pass an explicit path.",
            )
        })
}
pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("TRACEVEIL_CONFIG") {
        let path = std::path::absolute(p).map_err(traceveil::io_error)?;
        traceveil::adapters::validate_paths(std::slice::from_ref(&path))?;
        return Ok(path);
    }
    let root = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(p) => PathBuf::from(p),
        None => home()?.join(".config"),
    };
    let path =
        std::path::absolute(root.join("traceveil/config.json")).map_err(traceveil::io_error)?;
    traceveil::adapters::validate_paths(std::slice::from_ref(&path))?;
    Ok(path)
}
pub fn load() -> Result<Config> {
    let path = match path() {
        Ok(p) => p,
        Err(_)
            if std::env::var_os("HOME").is_none()
                && std::env::var_os("USERPROFILE").is_none()
                && std::env::var_os("TRACEVEIL_CONFIG").is_none()
                && std::env::var_os("XDG_CONFIG_HOME").is_none() =>
        {
            return Ok(Config::default());
        }
        Err(e) => return Err(e),
    };
    let parent = path.parent().ok_or_else(Error::input)?;
    let Some(dir) = Directory::open(parent, false)? else {
        return Ok(Config::default());
    };
    let Some(bytes) = dir.read(path.file_name().ok_or_else(Error::input)?, 65536)? else {
        return Ok(Config::default());
    };
    let c: Config = serde_json::from_slice(&bytes).map_err(|_| {
        Error::new(
            "CONFIGURATION_INVALID",
            "Configuration must be valid JSON with documented keys. Run traceveil config path.",
        )
    })?;
    c.settings(3, 600)?;
    Ok(c)
}
impl Config {
    pub fn settings(&self, repeat: usize, gap: u32) -> Result<Settings> {
        if self.session_dir.as_ref().is_some_and(|p| !p.is_absolute()) {
            return Err(Error::new(
                "CONFIGURATION_INVALID",
                "Use an absolute session_dir in Traceveil user configuration.",
            ));
        }
        let mut s = Settings {
            repeat_threshold: repeat,
            max_gap_seconds: gap,
            ..Settings::default()
        };
        for (key, value) in &self.rule_thresholds {
            s.rule_thresholds[rule_index(key)?] = Some(*value);
        }
        for key in &self.suppressed_rules {
            s.disabled_rules[rule_index(key)?] = true;
        }
        s.validate()
    }
}
pub fn rule_index(id: &str) -> Result<usize> {
    RULES.iter().position(|r| r.0 == id).ok_or_else(|| {
        Error::new(
            "CONFIGURATION_INVALID",
            "Unknown rule. Use RF001, LOOP001 or READ001.",
        )
    })
}
pub fn save(c: &Config) -> Result<PathBuf> {
    c.settings(3, 600)?;
    let bytes = serde_json::to_vec_pretty(c).map_err(|_| Error::input())?;
    if bytes.len() > 65536 {
        return Err(Error::new(
            "CONFIGURATION_INVALID",
            "Traceveil configuration exceeds 64 KiB.",
        ));
    }
    let path = path()?;
    let dir =
        Directory::open(path.parent().ok_or_else(Error::input)?, true)?.ok_or_else(Error::input)?;
    dir.write(path.file_name().ok_or_else(Error::input)?, &bytes)?;
    Ok(path)
}
