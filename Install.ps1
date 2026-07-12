Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n[exe-install] $Message"
}

function Require-Command {
    param([Parameter(Mandatory = $true)][string]$CommandName)
    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "[exe-install] Missing required command: $CommandName"
    }
}

function Get-RepoSlug {
    if (-not [string]::IsNullOrWhiteSpace($env:BASALT_GITHUB_REPO)) {
        return $env:BASALT_GITHUB_REPO
    }

    if (Get-Command git -ErrorAction SilentlyContinue) {
        $originUrl = (& git -C $script:RepoRoot remote get-url origin 2>$null).Trim()
        if ($LASTEXITCODE -eq 0 -and $originUrl -match 'github\.com[:/]([^/]+/[^/.]+)(\.git)?$') {
            return $Matches[1]
        }
    }

    return 'HungLo2020/Basalt'
}

function Get-LatestLocalInstaller {
    param([Parameter(Mandatory = $true)][string]$BuildsDir)

    if (-not (Test-Path $BuildsDir)) {
        return $null
    }

    $windowsSpecific = Get-ChildItem -Path $BuildsDir -Filter '*windows-amd64*.exe' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($windowsSpecific) {
        return $windowsSpecific.FullName
    }

    $anyExe = Get-ChildItem -Path $BuildsDir -Filter '*.exe' -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($anyExe) {
        return $anyExe.FullName
    }

    return $null
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

function Invoke-LocalBuildPipeline {
    $buildScript = Join-Path $script:RepoRoot 'DevUtils\Build.ps1'
    if (-not (Test-Path $buildScript)) {
        throw "[exe-install] Local build script not found: $buildScript"
    }

    Write-Log 'Building local Windows installer'
    $buildProcess = Start-Process `
        -FilePath powershell `
        -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $buildScript) `
        -Wait `
        -PassThru `
        -NoNewWindow
    if ($buildProcess.ExitCode -ne 0) {
        throw "[exe-install] Local build pipeline failed with exit code $($buildProcess.ExitCode)."
    }

    $buildMeta = Join-Path (Join-Path $script:RepoRoot 'builds') 'latest-build.env'
    if (-not (Test-Path $buildMeta)) {
        throw "[exe-install] Local build metadata not found: $buildMeta"
    }

    $metadata = Read-BuildMetadata -FilePath $buildMeta
    $artifactPath = if ($metadata.ContainsKey('BUILD_ARTIFACT_PATH')) { $metadata['BUILD_ARTIFACT_PATH'] } else { '' }
    $artifactType = if ($metadata.ContainsKey('BUILD_ARTIFACT_TYPE')) { $metadata['BUILD_ARTIFACT_TYPE'] } else { '' }

    if ([string]::IsNullOrWhiteSpace($artifactPath) -or -not (Test-Path $artifactPath)) {
        throw '[exe-install] Local build metadata is missing a valid BUILD_ARTIFACT_PATH.'
    }

    if ($artifactType -ne 'exe-installer' -or -not $artifactPath.ToLowerInvariant().EndsWith('.exe')) {
        throw "[exe-install] Local build produced an unsupported artifact for this installer: $artifactPath"
    }

    return $artifactPath
}

function Get-ReleaseInstallerUrl {
    param([Parameter(Mandatory = $true)][string]$RepoSlug)

    $apiUrl = "https://api.github.com/repos/$RepoSlug/releases/latest"
    Write-Log "Fetching latest GitHub release metadata from $RepoSlug"

    $headers = @{
        Accept = 'application/vnd.github+json'
        'User-Agent' = 'Basalt-InstallScript'
    }

    $release = Invoke-RestMethod -Uri $apiUrl -Headers $headers -Method Get
    if (-not $release -or -not $release.assets) {
        throw "[exe-install] No release assets found in latest release for $RepoSlug."
    }

    $windowsAsset = $release.assets |
        Where-Object {
            $_.browser_download_url -match '\.exe$' -and
            ($_.name -match 'windows-amd64' -or $_.name -match 'setup')
        } |
        Select-Object -First 1

    if ($windowsAsset) {
        return $windowsAsset.browser_download_url
    }

    $anyExeAsset = $release.assets |
        Where-Object { $_.browser_download_url -match '\.exe$' } |
        Select-Object -First 1

    if ($anyExeAsset) {
        return $anyExeAsset.browser_download_url
    }

    throw "[exe-install] No .exe asset found in latest release for $RepoSlug."
}

function Install-FromInstaller {
    param([Parameter(Mandatory = $true)][string]$InstallerPath)

    if (-not (Test-Path $InstallerPath)) {
        throw "[exe-install] Installer not found: $InstallerPath"
    }

    Write-Log "Running installer: $InstallerPath"
    $process = Start-Process -FilePath $InstallerPath -ArgumentList '/SP-', '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait -PassThru

    if ($process.ExitCode -ne 0) {
        throw "[exe-install] Installer exited with code $($process.ExitCode)."
    }

    $basaltCommand = Get-Command basalt -ErrorAction SilentlyContinue
    if ($basaltCommand) {
        Write-Log 'Installed successfully'
        Write-Host 'Run with: basalt list'
        return
    }

    $candidatePaths = @(
        (Join-Path $env:ProgramFiles 'Basalt\basalt.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Basalt\basalt.exe')
    )

    $resolvedBinary = $candidatePaths | Where-Object { -not [string]::IsNullOrWhiteSpace($_) -and (Test-Path $_) } | Select-Object -First 1
    if ($resolvedBinary) {
        Write-Log 'Installed successfully'
        Write-Host "Run with: `"$resolvedBinary`" list"
    }
    else {
        Write-Warning '[exe-install] Installer finished, but Basalt executable was not found in common install locations.'
    }
}

function Install-FromLocalInstaller {
    $InstallerPath = Invoke-LocalBuildPipeline
    Install-FromInstaller -InstallerPath $InstallerPath
}

function Install-FromGithubRelease {
    $repoSlug = Get-RepoSlug
    $downloadUrl = Get-ReleaseInstallerUrl -RepoSlug $repoSlug

    $fileName = [System.IO.Path]::GetFileName($downloadUrl)
    if ([string]::IsNullOrWhiteSpace($fileName)) {
        $fileName = 'basalt-setup.exe'
    }

    $tempInstaller = Join-Path ([System.IO.Path]::GetTempPath()) ("basalt-install-$([Guid]::NewGuid())-$fileName")

    Write-Log 'Downloading latest release installer'
    Invoke-WebRequest -Uri $downloadUrl -OutFile $tempInstaller

    try {
        Install-FromInstaller -InstallerPath $tempInstaller
    }
    finally {
        if (Test-Path $tempInstaller) {
            Remove-Item -Path $tempInstaller -Force -ErrorAction SilentlyContinue
        }
    }
}

function Main {
    if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
        throw '[exe-install] This script is intended for Windows only.'
    }

    Require-Command -CommandName powershell

    $scriptDir = Split-Path -Parent $PSCommandPath
    $script:RepoRoot = $scriptDir
    $buildsDir = Join-Path $script:RepoRoot 'builds'
    $localInstaller = Get-LatestLocalInstaller -BuildsDir $buildsDir
    $localBuildScript = Join-Path $script:RepoRoot 'DevUtils\Build.ps1'

    if (Test-Path $localBuildScript) {
        if (-not [string]::IsNullOrWhiteSpace($localInstaller) -and (Test-Path $localInstaller)) {
            Write-Host "A previous local Windows installer was found at: $localInstaller"
            Write-Host 'Choosing local install will rebuild it before installing.'
        }
        else {
            Write-Host 'Local build support was found for this checkout.'
        }
        Write-Host 'Choose an option:'
        Write-Host '  [1] Build and install local installer'
        Write-Host '  [2] Fetch and install latest GitHub release'

        while ($true) {
            $choice = Read-Host 'Enter choice [1/2]'
            switch ($choice) {
                '1' {
                    Install-FromLocalInstaller
                    return
                }
                '2' {
                    Install-FromGithubRelease
                    return
                }
                default {
                    Write-Host 'Please enter 1 or 2.'
                }
            }
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($localInstaller) -and (Test-Path $localInstaller)) {
        Write-Warning "[exe-install] Local build script was not found, so the existing local installer will not be used: $localInstaller"
    }

    Install-FromGithubRelease
}

Main
