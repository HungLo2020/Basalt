use crate::error::{CoreError, CoreResult};
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
pub struct EmulatorLaunchTarget {
    backend: EmulationBackend,
    system_key: String,
    rom_path: PathBuf,
}

impl EmulatorLaunchTarget {
    pub fn new(
        backend: EmulationBackend,
        system_key: impl Into<String>,
        rom_path: PathBuf,
    ) -> CoreResult<Self> {
        let system_key = normalize_system_key(system_key.into())
            .ok_or_else(|| CoreError::new("System key cannot be empty".to_string()))?;

        if rom_path.as_os_str().is_empty() {
            return Err(CoreError::new("ROM path cannot be empty".to_string()));
        }

        Ok(Self {
            backend,
            system_key,
            rom_path,
        })
    }

    pub fn new_retroarch(system_key: impl Into<String>, rom_path: PathBuf) -> CoreResult<Self> {
        Self::new(EmulationBackend::Retroarch, system_key, rom_path)
    }

    pub fn decode(raw: &str) -> CoreResult<Self> {
        let mut parts = raw.splitn(3, '|');
        let backend_raw = parts.next().unwrap_or_default();
        let system_raw = parts.next().unwrap_or_default();
        let rom_path_raw = parts.next().unwrap_or_default();

        let backend = EmulationBackend::parse(backend_raw).ok_or_else(|| {
            CoreError::new(format!("Unsupported emulator backend '{}'.", backend_raw))
        })?;

        let system_key = normalize_system_key(system_raw).ok_or_else(|| {
            CoreError::new("Malformed emulator launch target: missing system key.".to_string())
        })?;

        if rom_path_raw.trim().is_empty() {
            return Err(CoreError::new(
                "Malformed emulator launch target: missing ROM path.".to_string(),
            ));
        }

        Ok(Self {
            backend,
            system_key,
            rom_path: PathBuf::from(rom_path_raw),
        })
    }

    pub fn encode(&self) -> CoreResult<String> {
        let rom_path = self
            .rom_path
            .to_str()
            .ok_or_else(|| CoreError::new("ROM path contains invalid UTF-8".to_string()))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_target_round_trips_retroarch_system_and_path() {
        let target = EmulatorLaunchTarget::new_retroarch(
            "GBA",
            PathBuf::from(r"C:\Games\Pokemon Radical Red.gba"),
        )
        .unwrap();

        let encoded = target.encode().unwrap();
        let decoded = EmulatorLaunchTarget::decode(&encoded).unwrap();

        assert_eq!(decoded.system_key(), "gba");
        assert_eq!(
            decoded.rom_path(),
            Path::new(r"C:\Games\Pokemon Radical Red.gba")
        );
        assert_eq!(decoded.encode().unwrap(), encoded);
    }

    #[test]
    fn launch_target_rejects_missing_parts() {
        assert!(EmulatorLaunchTarget::decode("retroarch||game.gba").is_err());
        assert!(EmulatorLaunchTarget::decode("retroarch|gba|").is_err());
        assert!(EmulatorLaunchTarget::decode("unknown|gba|game.gba").is_err());
        assert!(EmulatorLaunchTarget::new_retroarch("gba", PathBuf::new()).is_err());
    }
}
