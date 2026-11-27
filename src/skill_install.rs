use crate::local_files::Directory;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use traceveil::{Error, Result, digest};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: String,
    sha256: String,
}
pub fn destination(scope: &str) -> Result<PathBuf> {
    let root = if scope == "user" {
        crate::discovery::agent_dir()?
    } else {
        std::env::current_dir()
            .map_err(traceveil::io_error)?
            .join(".pi")
    };
    let path = root.join("skills/traceveil");
    traceveil::adapters::validate_paths(std::slice::from_ref(&path))?;
    Ok(path)
}
fn owned(dir: &Directory) -> Result<Receipt> {
    let raw = dir.read(".traceveil-install.json", 8192)?.ok_or_else(|| {
        Error::new(
            "SKILL_NOT_OWNED",
            "No Traceveil installation receipt. Preserve this skill and select another scope.",
        )
    })?;
    let receipt: Receipt = serde_json::from_slice(&raw).map_err(|_| {
        Error::new(
            "SKILL_NOT_OWNED",
            "The skill installation receipt is invalid.",
        )
    })?;
    if receipt.version.is_empty()
        || receipt.sha256 != digest(include_str!("../skills/traceveil/SKILL.md"))
        || receipt.sha256.len() != 64
        || !receipt.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::new(
            "SKILL_NOT_OWNED",
            "The receipt does not identify this binary's distributed skill. Preserve the directory for manual review.",
        ));
    }
    Ok(receipt)
}
pub fn install(scope: &str, force: bool) -> Result<Value> {
    let path = destination(scope)?;
    let directory = if let Some(dir) = Directory::open(&path, false)? {
        let receipt = owned(&dir)?;
        if let Some(bytes) = dir.read("SKILL.md", 1048576)? {
            if !force && receipt.sha256 != digest(&bytes) {
                return Err(Error::new(
                    "SKILL_MODIFIED",
                    "The skill was modified. Use --force only after reviewing those changes.",
                ));
            }
        } else {
            return Err(Error::new(
                "SKILL_MODIFIED",
                "The recorded skill is missing. Preserve the receipt and review the directory manually.",
            ));
        }
        dir
    } else {
        Directory::open(&path, true)?.ok_or_else(Error::input)?
    };
    let text = include_str!("../skills/traceveil/SKILL.md");
    directory.write("SKILL.md", text.as_bytes())?;
    directory.write(
        ".traceveil-install.json",
        &serde_json::to_vec_pretty(&json!({"version":traceveil::VERSION,"sha256":digest(text)}))
            .map_err(|_| Error::input())?,
    )?;
    Ok(
        json!({"installed":true,"agent":"pi","scope":scope,"path":path,"restart":"Start a new Pi session or use Pi /reload to refresh skills."}),
    )
}
pub fn uninstall(scope: &str) -> Result<Value> {
    let path = destination(scope)?;
    let dir = Directory::open(&path, false)?.ok_or_else(|| {
        Error::new(
            "SKILL_NOT_OWNED",
            "No installed Traceveil skill in this scope.",
        )
    })?;
    let receipt = owned(&dir)?;
    let bytes = dir.read("SKILL.md", 1048576)?.ok_or_else(|| {
        Error::new(
            "SKILL_MODIFIED",
            "The installed skill is missing. Preserve the receipt for manual review.",
        )
    })?;
    if receipt.sha256 != digest(bytes) {
        return Err(Error::new(
            "SKILL_MODIFIED",
            "The installed skill was modified. Preserve it and remove it manually after review.",
        ));
    }
    dir.remove("SKILL.md")?;
    dir.remove(".traceveil-install.json")?;
    let directory_removed = Directory::remove_empty(&path)?;
    Ok(
        json!({"uninstalled":true,"scope":scope,"directory_removed":directory_removed,"unrelated_files":"preserved"}),
    )
}
