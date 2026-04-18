Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n[build-dispatch] $Message"
}

function Require-Command {
    param([Parameter(Mandatory = $true)][string]$CommandName)
    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "[build-dispatch] Missing required command: $CommandName"
    }
}

function Get-BuildPlatform {
    $osName = $null
    if ([System.Runtime.InteropServices.RuntimeInformation].GetProperty('OSDescription')) {
        $osName = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    }
    elseif ($env:OS -eq 'Windows_NT') {
        $osName = 'Windows'
    }

    if ($env:OS -eq 'Windows_NT') {
        if ([Environment]::Is64BitOperatingSystem) {
            return 'windows-amd64'
        }
        return $null
    }

    if ($osName -and $osName -match 'Linux') {
        if ([Environment]::Is64BitOperatingSystem) {
            return 'linux-amd64'
        }
        return $null
    }

    if ($osName -and ($osName -match 'Darwin' -or $osName -match 'macOS')) {
        return 'macos-arm64'
    }

    return $null
}

function Get-PlatformDescription {
    $osPart = if ([System.Runtime.InteropServices.RuntimeInformation].GetProperty('OSDescription')) {
        [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
    }
    elseif ($env:OS -eq 'Windows_NT') {
        'Windows'
    }
    else {
        'Unknown OS'
    }

    $archPart = if ([System.Runtime.InteropServices.RuntimeInformation].GetProperty('OSArchitecture')) {
        [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    elseif ([Environment]::Is64BitOperatingSystem) {
        'x64'
    }
    else {
        'x86'
    }

    return "$osPart/$archPart"
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

function Invoke-BuildScript {
    param(
        [Parameter(Mandatory = $true)][string]$Platform,
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$BuildMeta
    )

    $originalBuildMeta = $env:BASALT_BUILD_META
    $env:BASALT_BUILD_META = $BuildMeta

    try {
        if ($Platform -eq 'windows-amd64') {
            $buildScript = Join-Path $RepoRoot 'DevUtils\BuildScripts\build-windows-amd64.ps1'
            if (-not (Test-Path $buildScript)) {
                throw "[build-dispatch] Missing build script for platform '$Platform': $buildScript"
            }

            Write-Log "Delegating to: $buildScript"
            & $buildScript
            if ($LASTEXITCODE -ne 0) {
                throw "[build-dispatch] Build script failed: $buildScript"
            }

            return
        }

        $buildScript = Join-Path $RepoRoot "DevUtils/BuildScripts/build-$Platform.sh"
        if (-not (Test-Path $buildScript)) {
            throw "[build-dispatch] Missing build script for platform '$Platform': $buildScript"
        }

        Require-Command -CommandName bash
        Write-Log "Delegating to: $buildScript"
        & bash $buildScript
        if ($LASTEXITCODE -ne 0) {
            throw "[build-dispatch] Build script failed: $buildScript"
        }
    }
    finally {
        if ($null -eq $originalBuildMeta) {
            Remove-Item Env:BASALT_BUILD_META -ErrorAction SilentlyContinue
        }
        else {
            $env:BASALT_BUILD_META = $originalBuildMeta
        }
    }
}

function Main {
    $scriptDir = Split-Path -Parent $PSCommandPath
    $repoRoot = Resolve-Path (Join-Path $scriptDir '..')
    $buildsDir = Join-Path $repoRoot 'builds'
    $buildMeta = Join-Path $buildsDir 'latest-build.env'

    $platform = Get-BuildPlatform
    if ([string]::IsNullOrWhiteSpace($platform)) {
        throw "[build-dispatch] Unsupported platform: $(Get-PlatformDescription)"
    }

    Write-Log "Detected platform: $platform"
    Invoke-BuildScript -Platform $platform -RepoRoot $repoRoot -BuildMeta $buildMeta

    if (-not (Test-Path $buildMeta)) {
        throw "[build-dispatch] Build metadata not generated: $buildMeta"
    }

    $metadata = Read-BuildMetadata -FilePath $buildMeta
    $artifactPath = if ($metadata.ContainsKey('BUILD_ARTIFACT_PATH')) { $metadata['BUILD_ARTIFACT_PATH'] } else { '' }
    if ([string]::IsNullOrWhiteSpace($artifactPath) -or -not (Test-Path $artifactPath)) {
        throw '[build-dispatch] Build metadata is missing a valid BUILD_ARTIFACT_PATH'
    }

    $builtPlatform = if ($metadata.ContainsKey('BUILD_PLATFORM')) { $metadata['BUILD_PLATFORM'] } else { 'unknown' }
    $artifactType = if ($metadata.ContainsKey('BUILD_ARTIFACT_TYPE')) { $metadata['BUILD_ARTIFACT_TYPE'] } else { 'unknown' }

    Write-Log 'Done'
    Write-Host "Built platform: $builtPlatform"
    Write-Host "Artifact type: $artifactType"
    Write-Host "Artifact path: $artifactPath"
    Write-Host "Metadata file: $buildMeta"
}

Main
