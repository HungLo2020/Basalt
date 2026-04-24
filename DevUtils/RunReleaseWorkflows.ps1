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

    gh auth status *> $null
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

    Write-Log "Done - workflow dispatched. Monitor progress at:"
    $repoName = gh repo view --json nameWithOwner -q .nameWithOwner
    $actionsUrl = "https://github.com/$repoName/actions"
    Write-Host "  $actionsUrl"
}

main
