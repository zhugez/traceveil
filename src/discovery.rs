use serde::Serialize;
use std::{
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use traceveil::{Error, MAX_RECORD_BYTES, Result, adapters};

#[derive(Serialize)]
pub struct Session {
    pub path: PathBuf,
    pub format: &'static str,
    pub project_matches: Option<bool>,
    pub modified_unix_seconds: u64,
    pub possibly_active: bool,
    pub incomplete_tail: bool,
    pub bytes: u64,
}
pub struct Location {
    pub path: PathBuf,
    pub from_project_settings: bool,
}
impl Location {
    pub fn authorize(self, interactive: bool) -> Result<PathBuf> {
        adapters::validate_paths(std::slice::from_ref(&self.path))?;
        if self.from_project_settings {
            if !interactive {
                return Err(Error::new(
                    "DISCOVERY_AUTHORIZATION_REQUIRED",
                    "Project-controlled Pi settings redirect discovery. Pass --session-dir explicitly or confirm the proposed directory interactively.",
                ));
            }
            eprintln!(
                "Project Pi settings propose: {}",
                crate::presentation::label(&self.path.display().to_string())
            );
            if !crate::prompt::ask(|| {
                dialoguer::Confirm::new()
                    .with_prompt("Authorize this session directory?")
                    .default(false)
                    .interact()
            })? {
                return Err(Error::new("CANCELLED", "No source was read."));
            }
        }
        Ok(self.path)
    }
}
pub fn directory(project: &Path, configured: Option<&Path>) -> Result<Location> {
    if let Some(p) = configured {
        return Ok(Location {
            path: expand(Path::new(&p), project)?,
            from_project_settings: false,
        });
    }
    if let Some(p) = std::env::var_os("PI_CODING_AGENT_SESSION_DIR") {
        return Ok(Location {
            path: expand(Path::new(&p), project)?,
            from_project_settings: false,
        });
    }
    let agent = agent_dir()?;
    // Only the documented sessionDir field is consumed; no other Pi setting is retained.
    for file in [
        project.join(".pi/settings.json"),
        agent.join("settings.json"),
    ] {
        if file.try_exists().map_err(traceveil::io_error)? {
            let mut bytes = Vec::new();
            adapters::open_source(&file)?
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(traceveil::io_error)?;
            if bytes.len() > 65536 {
                return Err(Error::new(
                    "CONFIGURATION_INVALID",
                    "Pi settings exceed the bounded settings reader.",
                ));
            }
            let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
                Error::new(
                    "CONFIGURATION_INVALID",
                    "Pi settings are not valid JSON. Pass --session-dir explicitly.",
                )
            })?;
            if let Some(value) = v.get("sessionDir") {
                let s = value.as_str().filter(|s| !s.is_empty()).ok_or_else(|| {
                    Error::new(
                        "CONFIGURATION_INVALID",
                        "Pi sessionDir must be a nonempty path.",
                    )
                })?;
                let path = if let Some(t) = s.strip_prefix("~/") {
                    crate::user_config::home()?.join(t)
                } else {
                    let path = PathBuf::from(s);
                    if path.is_absolute() {
                        path
                    } else {
                        project.join(path)
                    }
                };
                return Ok(Location {
                    path,
                    from_project_settings: file == project.join(".pi/settings.json"),
                });
            }
        }
    }
    let project = project.canonicalize().map_err(traceveil::io_error)?;
    let text = project.to_str().ok_or_else(|| {
        Error::new(
            "PATH_ENCODING_UNSUPPORTED",
            "Pass an explicit UTF-8 session directory.",
        )
    })?;
    Ok(Location {
        path: agent.join("sessions").join(format!(
            "--{}--",
            text.trim_start_matches(['/', '\\'])
                .replace(['/', '\\', ':'], "-")
        )),
        from_project_settings: false,
    })
}
fn expand(path: &Path, project: &Path) -> Result<PathBuf> {
    Ok(if let Ok(t) = path.strip_prefix("~") {
        crate::user_config::home()?.join(t)
    } else if path.is_absolute() {
        path.into()
    } else {
        project.join(path)
    })
}
pub fn agent_dir() -> Result<PathBuf> {
    let path = match std::env::var_os("PI_CODING_AGENT_DIR") {
        Some(p) => {
            let p = PathBuf::from(p);
            if let Ok(t) = p.strip_prefix("~") {
                crate::user_config::home()?.join(t)
            } else {
                p
            }
        }
        None => crate::user_config::home()?.join(".pi/agent"),
    };
    adapters::validate_paths(std::slice::from_ref(&path))?;
    Ok(path)
}
pub fn sessions(directory: &Path, project: &Path, filter: bool) -> Result<Vec<Session>> {
    let paths = adapters::discover(directory)?;
    let mut out = Vec::new();
    for path in paths.into_iter().take(100) {
        if path.to_str().is_none() {
            return Err(Error::new(
                "PATH_ENCODING_UNSUPPORTED",
                "A session path cannot be represented as UTF-8.",
            ));
        }
        let mut file = adapters::open_source(&path)?;
        let meta = file.metadata().map_err(traceveil::io_error)?;
        let mut header = Vec::new();
        BufReader::new((&mut file).take((MAX_RECORD_BYTES + 1) as u64))
            .read_until(b'\n', &mut header)
            .map_err(traceveil::io_error)?;
        let value: Option<serde_json::Value> = serde_json::from_slice(&header).ok();
        let format = match value.as_ref() {
            Some(v) if v["type"] == "session" && v["version"] == 3 => "pi-v3",
            Some(v) if v["type"] == "session" => "unsupported-version",
            _ => "malformed-header",
        };
        let matches = value
            .as_ref()
            .and_then(|v| v["cwd"].as_str())
            .map(|cwd| Path::new(cwd) == project);
        if filter && matches == Some(false) {
            continue;
        }
        let incomplete_tail = if meta.len() > 0 {
            file.seek(SeekFrom::End(-1)).map_err(traceveil::io_error)?;
            let mut last = [0];
            file.read_exact(&mut last).map_err(traceveil::io_error)?;
            last[0] != b'\n'
        } else {
            true
        };
        let modified = meta.modified().map_err(traceveil::io_error)?;
        out.push(Session {
            path,
            format,
            project_matches: matches,
            modified_unix_seconds: modified
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            possibly_active: SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|d| d.as_secs() < 3),
            incomplete_tail,
            bytes: meta.len(),
        });
    }
    Ok(out)
}
pub fn select(directory: &Path, project: &Path, last: bool, interactive: bool) -> Result<PathBuf> {
    let items = sessions(directory, project, true)?;
    let items: Vec<_> = items.into_iter().filter(|s| s.format == "pi-v3").collect();
    if items.is_empty() {
        return Err(Error::new(
            "SESSION_NOT_FOUND",
            "No supported Pi v3 session found for this project. Run traceveil demo or traceveil init --session-dir DIR.",
        ));
    }
    if items.len() == 1 || last {
        return Ok(items[0].path.clone());
    }
    if !interactive {
        return Err(Error::new(
            "SESSION_AMBIGUOUS",
            "Several sessions match. Use analyze --last or select a path from traceveil sessions.",
        ));
    }
    let names: Vec<_> = items
        .iter()
        .map(|s| {
            crate::presentation::label(&s.path.file_name().unwrap_or_default().to_string_lossy())
        })
        .collect();
    let selected = crate::prompt::ask(|| {
        dialoguer::Select::new()
            .with_prompt("Select a Pi session (Esc to cancel)")
            .items(&names)
            .default(0)
            .interact_opt()
    })?;
    selected
        .map(|i| items[i].path.clone())
        .ok_or_else(|| Error::new("CANCELLED", "Session selection cancelled."))
}
