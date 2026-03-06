use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmulationBackend {
    Retroarch,
}

impl EmulationBackend {
    fn as_str(self) -> &'static str {
        match self {
            Self::Retroarch => "retroarch",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "retroarch" => Some(Self::Retroarch),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmulationLaunchTarget {
    backend: EmulationBackend,
    system_key: String,
    rom_path: PathBuf,
}

impl EmulationLaunchTarget {
    pub fn new(backend: EmulationBackend, system_key: impl Into<String>, rom_path: PathBuf) -> Result<Self, String> {
        let system_key = normalize_system_key(system_key.into())
            .ok_or_else(|| "System key cannot be empty".to_string())?;

        if rom_path.as_os_str().is_empty() {
            return Err("ROM path cannot be empty".to_string());
        }

        Ok(Self {
            backend,
            system_key,
            rom_path,
        })
    }

    pub fn new_retroarch(system_key: impl Into<String>, rom_path: PathBuf) -> Result<Self, String> {
        Self::new(EmulationBackend::Retroarch, system_key, rom_path)
    }

    pub fn decode(raw: &str) -> Result<Self, String> {
        let mut parts = raw.splitn(3, '|');
        let backend_raw = parts.next().unwrap_or_default();
        let system_raw = parts.next().unwrap_or_default();
        let rom_path_raw = parts.next().unwrap_or_default();

        let backend = EmulationBackend::parse(backend_raw)
            .ok_or_else(|| format!("Unsupported emulator backend '{}'.", backend_raw))?;

        let system_key = normalize_system_key(system_raw)
            .ok_or_else(|| "Malformed emulator launch target: missing system key.".to_string())?;

        if rom_path_raw.trim().is_empty() {
            return Err("Malformed emulator launch target: missing ROM path.".to_string());
        }

        Ok(Self {
            backend,
            system_key,
            rom_path: PathBuf::from(rom_path_raw),
        })
    }

    pub fn encode(&self) -> Result<String, String> {
        let rom_path = self
            .rom_path
            .to_str()
            .ok_or_else(|| "ROM path contains invalid UTF-8".to_string())?;

        Ok(format!(
            "{}|{}|{}",
            self.backend.as_str(),
            self.system_key,
            rom_path
        ))
    }

    pub fn system_key(&self) -> &str {
        &self.system_key
    }

    pub fn rom_path(&self) -> &Path {
        &self.rom_path
    }
}

fn normalize_system_key(system: impl AsRef<str>) -> Option<String> {
    let normalized = system.as_ref().trim().to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}