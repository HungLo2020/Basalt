Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)
    Write-Host "`n[run-release] $Message"
}

function main {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        Write-Error "[run-release] Missing required command: gh"
        exit 1
    }

    $authStatus = gh auth status 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[run-release] GitHub CLI is not authenticated. Run: gh auth login"
        exit 1
    }

    Write-Log "Triggering release-latest workflow"
    gh workflow run release-latest.yml
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[run-release] Failed to dispatch workflow"
        exit 1
    }

    Write-Log "Done — workflow dispatched. Monitor progress at:"
    $repoName = gh repo view --json nameWithOwner -q .nameWithOwner
    Write-Host "  https://github.com/$repoName/actions"
}

main
