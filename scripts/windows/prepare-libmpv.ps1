[CmdletBinding()]
param(
    [string]$ArchivePath
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$assetUrl = "https://github.com/Excentry/Ryotune-Omni/releases/download/libmpv-6a6ac973-r2/libmpv-windows-x64-6a6ac973.zip"
$archiveSha256 = "6ea36553ccdeb68ad99b8d76a0cd1c6a141a631d55b20862f4a9e3bcd6d5d6e1"
$dllSha256 = "6a6ac973ecb495b6799b32d221fdf357f212f9cbd65bc01599670ed6b9d13f56"
$importLibrarySha256 = "d311c8c722e8f78b560086527282fe849cf6349e7bbd0e26af68f35816911975"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$cacheDirectory = Join-Path $repositoryRoot ".libmpv\6a6ac973-x64-gpl"
$cachedArchive = Join-Path $cacheDirectory "libmpv.zip"
$dllDestination = Join-Path $repositoryRoot "src-tauri\libmpv-2.dll"
$licenseDestination = Join-Path $repositoryRoot "src-tauri\licenses\mpv\LICENSE.GPL"

New-Item -ItemType Directory -Path $cacheDirectory -Force | Out-Null

if ($ArchivePath) {
    $sourceArchive = (Resolve-Path $ArchivePath).Path
} else {
    $sourceArchive = $cachedArchive
    $cachedHash = if (Test-Path $cachedArchive) {
        (Get-FileHash -Algorithm SHA256 $cachedArchive).Hash.ToLowerInvariant()
    } else {
        ""
    }

    if ($cachedHash -ne $archiveSha256) {
        $downloadPath = "$cachedArchive.download"
        Remove-Item -LiteralPath $downloadPath -Force -ErrorAction SilentlyContinue
        Invoke-WebRequest -Uri $assetUrl -OutFile $downloadPath
        $downloadHash = (Get-FileHash -Algorithm SHA256 $downloadPath).Hash.ToLowerInvariant()
        if ($downloadHash -ne $archiveSha256) {
            Remove-Item -LiteralPath $downloadPath -Force -ErrorAction SilentlyContinue
            throw "libmpv archive SHA-256 mismatch: expected $archiveSha256, got $downloadHash"
        }
        Move-Item -LiteralPath $downloadPath -Destination $cachedArchive -Force
    }
}

$actualArchiveHash = (Get-FileHash -Algorithm SHA256 $sourceArchive).Hash.ToLowerInvariant()
if ($actualArchiveHash -ne $archiveSha256) {
    throw "libmpv archive SHA-256 mismatch: expected $archiveSha256, got $actualArchiveHash"
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($sourceArchive)
try {
    $entries = @{
        "libmpv-2.dll" = Join-Path $cacheDirectory "libmpv-2.dll"
        "libmpv.dll.a" = Join-Path $cacheDirectory "mpv.lib"
        "LICENSE.GPL" = Join-Path $cacheDirectory "LICENSE.GPL"
    }

    foreach ($entryName in $entries.Keys) {
        $entry = $archive.Entries | Where-Object { $_.FullName -ceq $entryName } | Select-Object -First 1
        if (-not $entry) {
            throw "Expected file is missing from the verified libmpv archive: $entryName"
        }

        $destination = $entries[$entryName]
        $entryStream = $entry.Open()
        $destinationStream = [System.IO.File]::Open($destination, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
        try {
            $entryStream.CopyTo($destinationStream)
        } finally {
            $destinationStream.Dispose()
            $entryStream.Dispose()
        }
    }
} finally {
    $archive.Dispose()
}

$actualDllHash = (Get-FileHash -Algorithm SHA256 $entries["libmpv-2.dll"]).Hash.ToLowerInvariant()
if ($actualDllHash -ne $dllSha256) {
    throw "libmpv DLL SHA-256 mismatch: expected $dllSha256, got $actualDllHash"
}

$actualImportLibraryHash = (Get-FileHash -Algorithm SHA256 $entries["libmpv.dll.a"]).Hash.ToLowerInvariant()
if ($actualImportLibraryHash -ne $importLibrarySha256) {
    throw "libmpv import library SHA-256 mismatch: expected $importLibrarySha256, got $actualImportLibraryHash"
}

New-Item -ItemType Directory -Path (Split-Path $dllDestination) -Force | Out-Null
New-Item -ItemType Directory -Path (Split-Path $licenseDestination) -Force | Out-Null
Copy-Item -LiteralPath $entries["libmpv-2.dll"] -Destination $dllDestination -Force
Copy-Item -LiteralPath $entries["LICENSE.GPL"] -Destination $licenseDestination -Force

Write-Output "libmpv archive SHA-256: $actualArchiveHash"
Write-Output "libmpv DLL SHA-256: $actualDllHash"
Write-Output "MSVC import library: $($entries['libmpv.dll.a']) (renamed from upstream .dll.a)"
Write-Output "Bundle resource DLL: $dllDestination"
Write-Output "Upstream license text: $licenseDestination"
