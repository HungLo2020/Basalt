Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n[build-windows-amd64] $Message"
}

function Require-Command {
    param([Parameter(Mandatory = $true)][string]$CommandName)
    if (-not (Get-Command $CommandName -ErrorAction SilentlyContinue)) {
        throw "[build-windows-amd64] Missing required command: $CommandName"
    }
}

function Resolve-IsccPath {
    if (Get-Command iscc -ErrorAction SilentlyContinue) {
        return 'iscc'
    }

    $candidatePaths = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    )

    foreach ($candidatePath in $candidatePaths) {
        if (-not [string]::IsNullOrWhiteSpace($candidatePath) -and (Test-Path $candidatePath)) {
            return $candidatePath
        }
    }

    throw '[build-windows-amd64] Missing required command: iscc'
}

function Read-CargoPackageVersion {
    param([Parameter(Mandatory = $true)][string]$CargoTomlPath)

    $lines = Get-Content -Path $CargoTomlPath
    $inPackageSection = $false

    foreach ($line in $lines) {
        if ($line -match '^\[package\]\s*$') {
            $inPackageSection = $true
            continue
        }

        if ($line -match '^\[') {
            $inPackageSection = $false
        }

        if ($inPackageSection -and $line -match '^\s*version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }

    return ''
}

function Write-Utf8NoBomFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Content
    )

    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
}

function Main {
    $scriptDir = Split-Path -Parent $PSCommandPath
    $repoRoot = Resolve-Path (Join-Path $scriptDir '..\..')
    $buildsDir = Join-Path $repoRoot 'builds'
    $cargoToml = Join-Path $repoRoot 'Cargo.toml'
    $buildMeta = if ([string]::IsNullOrWhiteSpace($env:BASALT_BUILD_META)) {
        Join-Path $buildsDir 'latest-build.env'
    }
    else {
        $env:BASALT_BUILD_META
    }

    $targetTriple = 'x86_64-pc-windows-msvc'

    Require-Command -CommandName cargo
    $isccCommand = Resolve-IsccPath

    $version = Read-CargoPackageVersion -CargoTomlPath $cargoToml
    if ([string]::IsNullOrWhiteSpace($version)) {
        throw "[build-windows-amd64] Unable to determine version from $cargoToml"
    }

    Write-Log 'Clearing builds directory'
    if (Test-Path $buildsDir) {
        Remove-Item -Path $buildsDir -Recurse -Force
    }
    New-Item -Path $buildsDir -ItemType Directory | Out-Null

    Write-Log 'Building Rust release binary'
    & cargo build --manifest-path $cargoToml --release --locked --target $targetTriple
    if ($LASTEXITCODE -ne 0) {
        throw '[build-windows-amd64] cargo build failed.'
    }

    $stagingDir = Join-Path $buildsDir 'installer-files'
    New-Item -Path $stagingDir -ItemType Directory | Out-Null

    $sourceExe = Join-Path $repoRoot "target\$targetTriple\release\basalt.exe"
    if (-not (Test-Path $sourceExe)) {
        throw "[build-windows-amd64] Built executable not found: $sourceExe"
    }

    $stagedExe = Join-Path $stagingDir 'basalt.exe'
    Copy-Item -Path $sourceExe -Destination $stagedExe -Force

    $outputBase = "Basalt-Setup-$version-windows-amd64"
    $artifactPath = Join-Path $buildsDir "$outputBase.exe"
    $issPath = Join-Path $buildsDir 'basalt-windows-installer.iss'

    $issContent = @"
[Setup]
AppId={{22A952AB-0A6D-4B42-9080-89A32D987343}
AppName=Basalt
AppVersion=$version
AppPublisher=Basalt Maintainers
DefaultDirName={autopf}\Basalt
DefaultGroupName=Basalt
DisableProgramGroupPage=yes
ChangesEnvironment=yes
OutputDir=$buildsDir
OutputBaseFilename=$outputBase
Compression=lzma
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
UninstallDisplayIcon={app}\basalt.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "$stagedExe"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
Root: HKLM; Subkey: "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Icons]
Name: "{autoprograms}\Basalt"; Filename: "{app}\basalt.exe"
Name: "{autodesktop}\Basalt"; Filename: "{app}\basalt.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; GroupDescription: "Additional icons:"; Flags: unchecked

[Run]
Filename: "{app}\basalt.exe"; Description: "Launch Basalt"; Flags: nowait postinstall skipifsilent

[Code]
const
    EnvironmentKey = 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment';

function NeedsAddPath(PathEntry: string): Boolean;
var
    ExistingPath: string;
begin
    if not RegQueryStringValue(HKLM, EnvironmentKey, 'Path', ExistingPath) then
    begin
        Result := True;
        exit;
    end;

    Result := Pos(';' + UpperCase(PathEntry) + ';', ';' + UpperCase(ExistingPath) + ';') = 0;
end;

procedure RemovePath(PathEntry: string);
var
    ExistingPath: string;
    NormalizedPath: string;
begin
    if not RegQueryStringValue(HKLM, EnvironmentKey, 'Path', ExistingPath) then
        exit;

    NormalizedPath := ';' + ExistingPath + ';';
    StringChangeEx(NormalizedPath, ';' + PathEntry + ';', ';', True);
    StringChangeEx(NormalizedPath, ';;', ';', True);

    if (Length(NormalizedPath) > 0) and (NormalizedPath[1] = ';') then
        Delete(NormalizedPath, 1, 1);

    if (Length(NormalizedPath) > 0) and (NormalizedPath[Length(NormalizedPath)] = ';') then
        Delete(NormalizedPath, Length(NormalizedPath), 1);

    RegWriteExpandStringValue(HKLM, EnvironmentKey, 'Path', NormalizedPath);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
    if CurUninstallStep = usUninstall then
        RemovePath(ExpandConstant('{app}'));
end;
"@

    Write-Utf8NoBomFile -Path $issPath -Content $issContent

    Write-Log 'Building Windows installer'
    & $isccCommand $issPath
    if ($LASTEXITCODE -ne 0) {
        throw '[build-windows-amd64] iscc failed.'
    }

    if (-not (Test-Path $artifactPath)) {
        throw "[build-windows-amd64] Installer output not found: $artifactPath"
    }

    $metaContent = @"
BUILD_PLATFORM=windows-amd64
BUILD_ARTIFACT_TYPE=exe-installer
BUILD_ARTIFACT_PATH=$artifactPath
BUILD_ARTIFACT_NAME=$(Split-Path -Leaf $artifactPath)
"@
    Write-Utf8NoBomFile -Path $buildMeta -Content $metaContent

    Write-Log 'Done'
    Write-Host "Built artifact: $artifactPath"
    Write-Host "Metadata file: $buildMeta"
}

Main
