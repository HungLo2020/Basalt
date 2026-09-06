#!/usr/bin/env python3
"""Build Basalt's Debian package and hand it to the repository manager."""

from __future__ import annotations

import argparse
import ast
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Callable, Mapping
from urllib.request import Request, urlopen


REPOSITORY_SCRIPT_URL = (
    "https://raw.githubusercontent.com/HungLo2020/LinuxScripts/master/"
    "GenericScripts/ManageMattOSRepository.py"
)
SCRIPT_RELATIVE_PATH = Path("DevUtils/.downloaded/ManageMattOSRepository.py")
BUILD_METADATA_RELATIVE_PATH = Path("builds/latest-build.env")


def repository_root() -> Path:
    return Path(__file__).resolve().parents[1]


def validate_script_content(content: bytes) -> str:
    """Return decoded script text, or raise if the download is not Python."""
    if not content.strip():
        raise ValueError("downloaded repository-management script is empty")
    try:
        text = content.decode("utf-8")
        tree = ast.parse(text, filename="ManageMattOSRepository.py")
    except (UnicodeDecodeError, SyntaxError) as error:
        raise ValueError("downloaded repository-management script is not valid UTF-8 Python") from error
    if not tree.body:
        raise ValueError("downloaded repository-management script has no Python statements")
    return text


def download_latest_script(target: Path, opener: Callable[..., object] = urlopen) -> None:
    """Download the authoritative script and atomically replace *target*."""
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        request = Request(
            REPOSITORY_SCRIPT_URL,
            headers={
                "Cache-Control": "no-cache",
                "Pragma": "no-cache",
                "User-Agent": "BasaltPublishMattOSPackage/1.0",
            },
        )
        with opener(request, timeout=30) as response:  # type: ignore[union-attr]
            text = validate_script_content(response.read())  # type: ignore[union-attr]
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{target.name}.", suffix=".tmp", dir=target.parent
        )
        temporary_path = Path(temporary_name)
        with os.fdopen(descriptor, "w", encoding="utf-8") as temporary_file:
            temporary_file.write(text)
            temporary_file.flush()
            os.fsync(temporary_file.fileno())
        os.replace(temporary_path, target)
        temporary_path = None
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def parse_build_metadata(metadata_text: str) -> Mapping[str, str]:
    """Parse KEY=value build metadata without evaluating it as shell."""
    values: dict[str, str] = {}
    for line_number, raw_line in enumerate(metadata_text.splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise ValueError(f"invalid build metadata on line {line_number}: {raw_line!r}")
        key, value = line.split("=", 1)
        key = key.strip()
        if not key or any(character.isspace() for character in key):
            raise ValueError(f"invalid build metadata key on line {line_number}")
        values[key] = value.strip()
    return values


def resolve_deb_artifact(metadata: Mapping[str, str], root: Path) -> Path:
    if metadata.get("BUILD_ARTIFACT_TYPE") != "deb":
        raise ValueError("build did not produce a Debian artifact (BUILD_ARTIFACT_TYPE=deb required)")
    artifact = Path(metadata.get("BUILD_ARTIFACT_PATH", ""))
    if not artifact.is_absolute():
        artifact = root / artifact
    artifact = artifact.resolve()
    if not artifact.is_file():
        raise ValueError(f"package path does not exist: {artifact}")
    if artifact.suffix != ".deb":
        raise ValueError(f"package path is not a .deb file: {artifact}")
    return artifact


def run(command: list[str], root: Path) -> int:
    try:
        return subprocess.run(command, cwd=root, check=False).returncode
    except OSError as error:
        print(f"[publish] ERROR: unable to run {' '.join(command)}: {error}", file=sys.stderr)
        return 127


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("doctor", "publish"))
    args = parser.parse_args(argv)

    root = repository_root()
    downloaded_script = root / SCRIPT_RELATIVE_PATH
    try:
        print(f"[publish] Downloading latest repository-management script from {REPOSITORY_SCRIPT_URL}")
        download_latest_script(downloaded_script)
    except (OSError, ValueError) as error:
        print(f"[publish] ERROR: download failed; cached scripts are not used: {error}", file=sys.stderr)
        return 1

    manager = [sys.executable, str(downloaded_script), "--repo", "mattpackages"]
    if args.command == "doctor":
        print("[publish] Running repository doctor")
        return run(manager + ["doctor"], root)

    print("[publish] Building Debian package")
    status = run(["bash", str(root / "DevUtils/Build.sh")], root)
    if status != 0:
        return status

    metadata_path = root / BUILD_METADATA_RELATIVE_PATH
    try:
        metadata = parse_build_metadata(metadata_path.read_text(encoding="utf-8"))
        artifact = resolve_deb_artifact(metadata, root)
    except (OSError, ValueError) as error:
        print(f"[publish] ERROR: {error}", file=sys.stderr)
        return 1

    print(f"[publish] Handing package to authoritative repository manager: {artifact}")
    return run(manager + ["upload", str(artifact)], root)


if __name__ == "__main__":
    raise SystemExit(main())
