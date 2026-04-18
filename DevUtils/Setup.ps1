Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n[setup-windows] $Message"
}

function Test-CommandAvailable {
    param([Parameter(Mandatory = $true)][string]$CommandName)
    return $null -ne (Get-Command $CommandName -ErrorAction SilentlyContinue)
}

function Require-Command {
    param([Parameter(Mandatory = $true)][string]$CommandName)
    if (-not (Test-CommandAvailable -CommandName $CommandName)) {
        throw "[setup-windows] Missing required command: $CommandName"
    }
}

function Install-WingetPackageIfMissing {
    param(
        [Parameter(Mandatory = $true)][string]$PackageId,
        [Parameter(Mandatory = $true)][string]$FriendlyName,
        [string]$AdditionalInstallArgs = ''
    )

    Write-Log "Checking $FriendlyName ($PackageId)"
    $listOutput = & winget list --id $PackageId --exact --accept-source-agreements 2>$null | Out-String
    if ($LASTEXITCODE -eq 0 -and $listOutput -match [Regex]::Escape($PackageId)) {
        Write-Host "[setup-windows] $FriendlyName already installed"
        return
    }

    Write-Log "Installing $FriendlyName"
    $installArgs = @(
        'install',
        '--id', $PackageId,
        '--exact',
        '--silent',
        '--accept-package-agreements',
        '--accept-source-agreements'
    )

    if ($AdditionalInstallArgs.Trim().Length -gt 0) {
        $installArgs += @('--override', $AdditionalInstallArgs)
    }

    & winget @installArgs
    if ($LASTEXITCODE -ne 0) {
        throw "[setup-windows] Failed to install $FriendlyName ($PackageId)."
    }
}

function Ensure-PathContainsCargoBin {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (-not (Test-Path $cargoBin)) {
        return
    }

    $pathEntries = $env:Path -split ';'
    if ($pathEntries -contains $cargoBin) {
        return
    }

    $env:Path = "$cargoBin;$env:Path"
}

function Ensure-PathContainsInnoSetup {
    $candidateDirs = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6')
    )

    $pathEntries = $env:Path -split ';'
    foreach ($candidateDir in $candidateDirs) {
        if ([string]::IsNullOrWhiteSpace($candidateDir)) {
            continue
        }

        $compilerPath = Join-Path $candidateDir 'ISCC.exe'
        if (-not (Test-Path $compilerPath)) {
            continue
        }

        if ($pathEntries -contains $candidateDir) {
            return
        }

        $env:Path = "$candidateDir;$env:Path"
        return
    }
}

function Ensure-PathContainsGitHubCli {
    $candidateDirs = @(
        (Join-Path $env:ProgramFiles 'GitHub CLI'),
        (Join-Path ${env:ProgramFiles(x86)} 'GitHub CLI'),
        (Join-Path $env:LOCALAPPDATA 'Programs\GitHub CLI')
    )

    $pathEntries = $env:Path -split ';'
    foreach ($candidateDir in $candidateDirs) {
        if ([string]::IsNullOrWhiteSpace($candidateDir)) {
            continue
        }

        $ghPath = Join-Path $candidateDir 'gh.exe'
        if (-not (Test-Path $ghPath)) {
            continue
        }

        if ($pathEntries -contains $candidateDir) {
            return
        }

        $env:Path = "$candidateDir;$env:Path"
        return
    }
}

function Setup-WindowsDependencies {
    Require-Command -CommandName winget

    Install-WingetPackageIfMissing -PackageId 'Git.Git' -FriendlyName 'Git'
    Install-WingetPackageIfMissing -PackageId 'GitHub.cli' -FriendlyName 'GitHub CLI'
    Install-WingetPackageIfMissing -PackageId 'Kitware.CMake' -FriendlyName 'CMake'
    Install-WingetPackageIfMissing -PackageId 'LLVM.LLVM' -FriendlyName 'LLVM'
    Install-WingetPackageIfMissing -PackageId 'JRSoftware.InnoSetup' -FriendlyName 'Inno Setup'

    $vsOverride = '--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    Install-WingetPackageIfMissing -PackageId 'Microsoft.VisualStudio.2022.BuildTools' -FriendlyName 'Visual Studio 2022 Build Tools' -AdditionalInstallArgs $vsOverride

    Ensure-PathContainsInnoSetup
    Ensure-PathContainsGitHubCli
}

function Setup-RustToolchain {
    if (-not (Test-CommandAvailable -CommandName rustup)) {
        Install-WingetPackageIfMissing -PackageId 'Rustlang.Rustup' -FriendlyName 'Rustup'
    }
    else {
        Write-Log 'rustup already installed; updating'
        & rustup self update
        if ($LASTEXITCODE -ne 0) {
            throw '[setup-windows] rustup self update failed.'
        }
    }

    Ensure-PathContainsCargoBin
    Require-Command -CommandName rustup

    Write-Log 'Installing/Updating Rust stable toolchain'
    & rustup toolchain install stable
    if ($LASTEXITCODE -ne 0) {
        throw '[setup-windows] rustup toolchain install stable failed.'
    }

    & rustup default stable
    if ($LASTEXITCODE -ne 0) {
        throw '[setup-windows] rustup default stable failed.'
    }

    & rustup component add rustfmt clippy
    if ($LASTEXITCODE -ne 0) {
        throw '[setup-windows] rustup component add rustfmt clippy failed.'
    }
}

function Verify-Toolchain {
    Ensure-PathContainsCargoBin
    Ensure-PathContainsInnoSetup
    Ensure-PathContainsGitHubCli
    Require-Command -CommandName rustc
    Require-Command -CommandName cargo
    Require-Command -CommandName iscc
    Require-Command -CommandName gh

    Write-Log 'Verifying toolchain'
    & rustc --version
    if ($LASTEXITCODE -ne 0) {
        throw '[setup-windows] rustc --version failed.'
    }

    & cargo --version
    if ($LASTEXITCODE -ne 0) {
        throw '[setup-windows] cargo --version failed.'
    }

    Write-Host "[setup-windows] Inno Setup compiler found: $((Get-Command iscc).Source)"

    & gh --version
    if ($LASTEXITCODE -ne 0) {
        throw '[setup-windows] gh --version failed.'
    }

    Write-Host "[setup-windows] GitHub CLI found: $((Get-Command gh).Source)"
}

function Test-IsWindowsHost {
    return [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
}

function Main {
    if (-not (Test-IsWindowsHost)) {
        throw '[setup-windows] This script is intended for Windows only.'
    }

    Setup-WindowsDependencies
    Setup-RustToolchain
    Verify-Toolchain

    Write-Log 'Setup complete'
}

Main
