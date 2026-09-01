[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [ValidatePattern('^v\d+\.\d+\.\d+$')]
    [string]$Tag,
    [switch]$Apply,
    [switch]$SkipVerify
)

$ErrorActionPreference = 'Stop'

function Invoke-Git {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$GitArgs)
    & git @GitArgs
    if ($LASTEXITCODE -ne 0) {
        throw "git $($GitArgs -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Get-GitOutput {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$GitArgs)
    $result = & git @GitArgs
    if ($LASTEXITCODE -ne 0) {
        throw "git $($GitArgs -join ' ') failed with exit code $LASTEXITCODE"
    }
    return ($result | Out-String).Trim()
}

function Test-GitRef {
    param([string]$Ref)
    & git show-ref --verify --quiet $Ref
    return $LASTEXITCODE -eq 0
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $branch = Get-GitOutput branch --show-current
    if ($branch -ne 'custom/main') {
        throw "Run this script from custom/main; current branch is '$branch'."
    }

    $status = Get-GitOutput status --porcelain
    if ($status) {
        throw "Refusing to upgrade a dirty worktree. Commit or stash your current changes first."
    }

    $upstreamUrl = Get-GitOutput remote get-url upstream
    if (-not $upstreamUrl) {
        throw 'The upstream remote is not configured.'
    }

    Invoke-Git fetch upstream --tags --prune

    if (-not $Tag) {
        $Tag = @(
            (& git tag --list 'v*' --sort=-v:refname) |
                Where-Object { $_ -match '^v\d+\.\d+\.\d+$' }
        ) | Select-Object -First 1
    }
    if (-not $Tag) {
        throw 'Could not find an upstream stable semantic-version tag.'
    }
    if (-not (Test-GitRef "refs/tags/$Tag")) {
        throw "Upstream tag '$Tag' was not fetched."
    }

    $currentVersion = (Get-Content -Raw package.json | ConvertFrom-Json).version
    $currentUpstreamVersion = $currentVersion -replace '-custom\.\d+$', ''
    $targetUpstreamVersion = $Tag.Substring(1)
    if ($currentUpstreamVersion -eq $targetUpstreamVersion) {
        Write-Host "Already based on upstream $Tag ($currentVersion). Nothing to prepare."
        return
    }

    $upgradeBranch = "upgrade/$Tag"
    if (Test-GitRef "refs/heads/$upgradeBranch") {
        throw "Local branch '$upgradeBranch' already exists. Review or delete it manually before retrying."
    }
    if (Test-GitRef "refs/remotes/origin/$upgradeBranch") {
        throw "Remote branch '$upgradeBranch' already exists. Review its pull request before retrying."
    }

    Invoke-Git switch --create $upgradeBranch custom/main
    & git merge --no-ff --no-edit $Tag
    if ($LASTEXITCODE -ne 0) {
        $conflicts = (& git diff --name-only --diff-filter=U) -join ', '
        throw "Merge conflict while importing $Tag. Resolve only after reviewing the conflicted files: $conflicts"
    }

    $customVersion = "$targetUpstreamVersion-custom.1"
    & npm run version:set -- $customVersion
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to set the custom release version.'
    }
    Invoke-Git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
    Invoke-Git commit -m "chore(custom): prepare upstream $Tag"

    if (-not $SkipVerify) {
        & node scripts/verify-custom-boundaries.mjs
        if ($LASTEXITCODE -ne 0) { throw 'Custom boundary verification failed.' }
        & npm run lint
        if ($LASTEXITCODE -ne 0) { throw 'Frontend lint failed.' }
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
    }

    Write-Host "Prepared $upgradeBranch from $Tag. Review the diff before merging it."
    if ($Apply) {
        if ($PSCmdlet.ShouldProcess('custom/main', "merge $upgradeBranch")) {
            Invoke-Git switch custom/main
            Invoke-Git merge --no-ff $upgradeBranch
            Write-Host "Merged $upgradeBranch into custom/main. Push custom/main only after final review."
        }
    }
} finally {
    Pop-Location
}
