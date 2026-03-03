use std::fs;
use std::path::Path;
use std::process::Command;

pub fn normalize_bash_script_path(raw_script_path: &str) -> Result<String, String> {
    let script_path = Path::new(raw_script_path);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!(
            "Script does not exist or is not a file: {}",
            raw_script_path
        ));
    }

    let has_sh_extension = script_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("sh"))
        .unwrap_or(false);

    if !has_sh_extension {
        return Err("Only bash scripts are supported right now (expected .sh file)".to_string());
    }

    let canonical_script_path = fs::canonicalize(script_path)
        .map_err(|err| format!("Failed to resolve script path: {}", err))?;

    canonical_script_path
        .to_str()
        .ok_or_else(|| "Script path contains invalid UTF-8".to_string())
        .map(|value| value.to_string())
}

pub fn launch(script_path: &str) -> Result<(), String> {
    let path = Path::new(script_path);
    if !path.exists() || !path.is_file() {
        return Err(format!(
            "Saved script path does not exist or is not a file: {}",
            script_path
        ));
    }

    let status = Command::new("bash")
        .arg(path)
        .status()
        .map_err(|err| format!("Failed to launch script: {}", err))?;

    if !status.success() {
        return Err(format!(
            "Script exited with non-zero status: {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string())
        ));
    }

    Ok(())
}