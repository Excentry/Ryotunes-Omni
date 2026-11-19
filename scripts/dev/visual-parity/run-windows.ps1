param(
    [Parameter(Mandatory)][string]$LibmpvDir,
    [string]$OutDir = (Join-Path $env:TEMP ("ryotunes-visual-parity\windows-" + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))),
    [int]$Port = 9337,
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..\..')
$identifier = 'dev.excentry.ryotune-omni.visual-parity'
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }

$env:LIB = "$LibmpvDir;$env:LIB"
$env:PATH = "$LibmpvDir;$env:PATH"

if (-not $SkipBuild) {
    Push-Location (Join-Path $root 'ui')
    try { pnpm build; if ($LASTEXITCODE) { throw 'pnpm build failed' } } finally { Pop-Location }
    Push-Location $root
    try {
        cargo tauri build --debug --no-bundle --config scripts/dev/visual-parity/tauri.visual-parity.conf.json -- --locked
        if ($LASTEXITCODE) { throw 'cargo tauri build failed' }
    } finally { Pop-Location }
}

foreach ($base in $env:APPDATA, $env:LOCALAPPDATA) {
    $labProfile = Join-Path $base $identifier
    if (Test-Path $labProfile) { Remove-Item -Recurse -Force $labProfile }
}

$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port --remote-debugging-address=127.0.0.1"
$app = Start-Process -FilePath (Join-Path $target 'debug\ryotune-omni.exe') -PassThru
try {
    Start-Sleep -Seconds 15
    node (Join-Path $PSScriptRoot 'win-capture.mjs') $Port $OutDir
    if ($LASTEXITCODE) { throw 'capture failed' }
} finally {
    Stop-Process -Id $app.Id -ErrorAction SilentlyContinue
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
}
$OutDir
