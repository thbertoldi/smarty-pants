//! Preserve comments and unrelated settings when the tray edits configuration.

use crate::config::Config;
use std::{io::Write, path::Path};
use toml_edit::{DocumentMut, Item, Value};

pub struct ConfigFile {
    document: DocumentMut,
}

impl ConfigFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let document: DocumentMut = raw.parse()?;
        for section in ["daemon", "inference", "model", "inject", "deepseek", "api"] {
            if let Some(item) = document.get(section) {
                anyhow::ensure!(
                    item.as_table_like().is_some(),
                    "configuration section {section} must be a table"
                );
            }
        }
        Ok(Self { document })
    }

    pub fn set(&mut self, section: &str, key: &str, value: impl Into<Value>) {
        self.document[section][key] = Item::Value(value.into());
    }

    pub fn remove(&mut self, section: &str, key: &str) {
        if let Some(table) = self
            .document
            .get_mut(section)
            .and_then(Item::as_table_like_mut)
        {
            table.remove(key);
        }
    }

    pub fn config(&self) -> anyhow::Result<Config> {
        let mut cfg: Config = toml::from_str(&self.document.to_string())?;
        cfg.add_builtin_modes();
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        self.config()?;
        write_private(path, self.document.to_string().as_bytes())
    }
}

pub fn initialize(path: &Path, cfg: &Config) -> anyhow::Result<()> {
    if !path.exists() {
        cfg.validate()?;
        let text = format!(
            "# smarty-pants configuration. Reload after editing.\n{}",
            toml::to_string_pretty(cfg)?
        );
        write_private(path, text.as_bytes())?;
    }
    Ok(())
}

/// Atomic replacement; also used for the separately stored API key.
pub fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

pub fn new_private_file(dir: &Path, bytes: &[u8]) -> anyhow::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::Builder::new()
        .prefix("api-")
        .suffix(".key")
        .tempfile_in(dir)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    let (_, path) = file.keep()?; // tempfile creates files with mode 0600.
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_preserve_custom_modes_and_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            "# keep me\n[model]\ngpu_layers = -1\n[modes.personal]\nsystem = 'Keep this prompt'\n",
        )
        .unwrap();
        let mut file = ConfigFile::load(&path).unwrap();
        file.set("model", "gpu_layers", 0);
        file.save(&path).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("# keep me"));
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.model.gpu_layers, 0);
        assert_eq!(cfg.modes["personal"].system, "Keep this prompt");
    }

    #[test]
    fn invalid_edit_leaves_original_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "# original\n").unwrap();
        let mut file = ConfigFile::load(&path).unwrap();
        file.set("model", "temperature", -1.0);
        assert!(file.save(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# original\n");
    }
}
