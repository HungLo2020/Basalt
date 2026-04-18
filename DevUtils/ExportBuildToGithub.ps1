Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n[export-github] $Message"
}

function Require-Command {
    param([Parameter(Mandatory = $true)][string]$CommandName)
    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "[export-github] Missing required command: $CommandName"
    }
}

function Read-BuildMetadata {
    param([Parameter(Mandatory = $true)][string]$FilePath)

    $metadata = @{}
    foreach ($line in Get-Content -Path $FilePath) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith('#')) {
            continue
        }

        $separatorIndex = $trimmed.IndexOf('=')
        if ($separatorIndex -lt 0) {
            continue
        }

        $key = $trimmed.Substring(0, $separatorIndex)
        $value = $trimmed.Substring($separatorIndex + 1)
        $metadata[$key] = $value
    }

    return $metadata
}

function Invoke-Gh {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & gh @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "[export-github] gh command failed: gh $($Arguments -join ' ')"
    }
}

function Main {
    $scriptDir = Split-Path -Parent $PSCommandPath
    $repoRoot = Resolve-Path (Join-Path $scriptDir '..')
    $buildsDir = Join-Path $repoRoot 'builds'
    $buildMeta = Join-Path $buildsDir 'latest-build.env'
    $buildScript = Join-Path $repoRoot 'DevUtils\Build.ps1'

    $tagName = 'latest'

    Require-Command -CommandName git
    Require-Command -CommandName gh

    & gh auth status | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw '[export-github] GitHub CLI is not authenticated. Run: gh auth login'
    }

    Write-Log 'Building local platform artifact'
    & $buildScript
    if ($LASTEXITCODE -ne 0) {
        throw "[export-github] Build script failed: $buildScript"
    }

    if (-not (Test-Path $buildMeta)) {
        throw "[export-github] Build metadata file not found: $buildMeta"
    }

    $metadata = Read-BuildMetadata -FilePath $buildMeta
    $artifactPath = if ($metadata.ContainsKey('BUILD_ARTIFACT_PATH')) { $metadata['BUILD_ARTIFACT_PATH'] } else { '' }
    $artifactName = if ($metadata.ContainsKey('BUILD_ARTIFACT_NAME')) { $metadata['BUILD_ARTIFACT_NAME'] } else { '' }
    $buildPlatform = if ($metadata.ContainsKey('BUILD_PLATFORM')) { $metadata['BUILD_PLATFORM'] } else { '' }

    if ([string]::IsNullOrWhiteSpace($artifactPath) -or -not (Test-Path $artifactPath)) {
        throw "[export-github] Local build artifact path is invalid: $artifactPath"
    }

    if ([string]::IsNullOrWhiteSpace($buildPlatform)) {
        throw '[export-github] Build metadata missing BUILD_PLATFORM'
    }

    if ([string]::IsNullOrWhiteSpace($artifactName)) {
        $artifactName = Split-Path -Leaf $artifactPath
    }

    $timestamp = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd HH:mm:ss "UTC"')
    $commitSha = (& git -C $repoRoot rev-parse --short HEAD).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw '[export-github] Failed to determine git commit SHA'
    }

    $releaseTitle = "Basalt Latest Build ($commitSha)"

    & gh release view $tagName | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Log "Deleting previous '$tagName' release"
        Invoke-Gh -Arguments @('release', 'delete', $tagName, '--yes', '--cleanup-tag')
    }

    Write-Log "Creating new '$tagName' release"
    $notes = "Automated Basalt build exported on $timestamp.`nCommit: $commitSha`nLocal platform: $buildPlatform"
    Invoke-Gh -Arguments @(
        'release', 'create', $tagName,
        "$artifactPath#$artifactName",
        '--title', $releaseTitle,
        '--notes', $notes
    )

    switch ($buildPlatform) {
        'linux-amd64' {
            Write-Log 'Triggering CI workflow for macOS arm64 DMG'
            Invoke-Gh -Arguments @('workflow', 'run', 'release-macos-dmg.yml', '-f', "release_tag=$tagName")

            Write-Log 'Triggering CI workflow for Windows amd64 installer'
            Invoke-Gh -Arguments @('workflow', 'run', 'release-windows-installer.yml', '-f', "release_tag=$tagName")
        }
        'macos-arm64' {
            Write-Log 'Triggering CI workflow for Linux amd64 DEB'
            Invoke-Gh -Arguments @('workflow', 'run', 'release-linux-deb.yml', '-f', "release_tag=$tagName")

            Write-Log 'Triggering CI workflow for Windows amd64 installer'
            Invoke-Gh -Arguments @('workflow', 'run', 'release-windows-installer.yml', '-f', "release_tag=$tagName")
        }
        'windows-amd64' {
            Write-Log 'Triggering CI workflow for Linux amd64 DEB'
            Invoke-Gh -Arguments @('workflow', 'run', 'release-linux-deb.yml', '-f', "release_tag=$tagName")

            Write-Log 'Triggering CI workflow for macOS arm64 DMG'
            Invoke-Gh -Arguments @('workflow', 'run', 'release-macos-dmg.yml', '-f', "release_tag=$tagName")
        }
        default {
            throw "[export-github] Unsupported BUILD_PLATFORM in metadata: $buildPlatform"
        }
    }

    Write-Log 'Done'
    Write-Host "Release published and replaced at tag: $tagName"
    Write-Host "Uploaded local artifact: $artifactName"
    Write-Host "Local platform: $buildPlatform"
}

Main
