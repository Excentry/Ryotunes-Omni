[CmdletBinding()]
param(
    [ValidatePattern("^[a-fA-F0-9]{32}$")]
    [string]$ClientId,
    [ValidatePattern("^[a-zA-Z0-9]{22}$")]
    [string[]]$PlaylistId,
    [ValidateRange(1, 100)]
    [int]$MaxPages = 30,
    [switch]$CreateTestPlaylist,
    [switch]$InspectPlaylistOnly,
    [switch]$CompleteTestPlaylist,
    [switch]$ListingOnly,
    [switch]$ShowPlaylistNames,
    [switch]$MatchSample,
    [switch]$ReviewMatches,
    [string]$ExcludeMatchReviewPath,
    [string]$MatcherPath,
    [ValidateRange(1, 10)]
    [int]$SamplePerPlaylist = 5,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function ConvertTo-Base64Url([byte[]]$Bytes) {
    [Convert]::ToBase64String($Bytes).TrimEnd("=").Replace("+", "-").Replace("/", "_")
}

function New-RandomValue {
    ConvertTo-Base64Url ([System.Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
}

function Get-PkceChallenge([string]$Verifier) {
    ConvertTo-Base64Url ([System.Security.Cryptography.SHA256]::HashData(
        [System.Text.Encoding]::ASCII.GetBytes($Verifier)
    ))
}

function Read-Callback([string]$Target, [string]$ExpectedState) {
    $uri = [uri]("http://127.0.0.1:8989" + $Target)
    if ($uri.AbsolutePath -cne "/login") { throw "Unexpected callback path." }
    $parameters = @{}
    foreach ($pair in $uri.Query.TrimStart("?").Split("&")) {
        $parts = $pair.Split("=", 2)
        $key = [uri]::UnescapeDataString($parts[0])
        if ($parameters.ContainsKey($key)) { throw "Duplicate callback parameter." }
        $parameters[$key] = if ($parts.Count -eq 2) {
            [uri]::UnescapeDataString($parts[1].Replace("+", " "))
        } else { "" }
    }
    if ($parameters["state"] -cne $ExpectedState) { throw "OAuth state mismatch." }
    if ($parameters.ContainsKey("error")) { throw "Spotify authorization was denied." }
    if ([string]::IsNullOrWhiteSpace($parameters["code"])) { throw "Missing authorization code." }
    $parameters["code"]
}

function Invoke-SpotifyGet([string]$Url, [string]$Token) {
    $uri = [uri]$Url
    if ($uri.Scheme -cne "https" -or $uri.Host -cne "api.spotify.com" -or
        $uri.Port -ne 443 -or $uri.UserInfo -or -not $uri.AbsolutePath.StartsWith("/v1/")) {
        throw "Rejected API destination."
    }
    try {
        Invoke-RestMethod -Uri $uri -Headers @{ Authorization = "Bearer $Token" } `
            -TimeoutSec 30 -MaximumRedirection 0 -Verbose:$false -Debug:$false
    } catch {
        $status = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
        throw "Spotify read failed (HTTP $status). No automatic retry; check access or quota."
    }
}

function Read-SpotifyPages([string]$Url, [string]$Token) {
    $entries = [System.Collections.Generic.List[object]]::new()
    $visited = [System.Collections.Generic.HashSet[string]]::new()
    $total = $null
    $pages = 0
    while ($Url) {
        if ($pages -ge $MaxPages) { throw "Pagination limit reached; result is incomplete." }
        if (-not $visited.Add($Url)) { throw "Repeated pagination URL." }
        $page = Invoke-SpotifyGet $Url $Token
        if (-not $page.PSObject.Properties["items"] -or
            -not $page.PSObject.Properties["total"] -or
            -not $page.PSObject.Properties["offset"] -or
            -not $page.PSObject.Properties["next"] -or
            $null -eq $page.items -or $page.offset -ne $entries.Count) {
            throw "Unexpected pagination shape or offset."
        }
        if ($null -eq $total) { $total = [int]$page.total }
        if ($page.total -ne $total) { throw "Source count changed during pagination." }
        foreach ($entry in $page.items) { $entries.Add($entry) }
        $Url = [string]$page.next
        $pages++
        if (-not $SelfTest) {
            Write-Host ("Page {0}: offset={1}, received={2}, total={3}, hasNext={4}" -f
                $pages, $page.offset, @($page.items).Count, $total, [bool]$Url)
        }
    }
    if ($entries.Count -ne $total) {
        throw "Source count differs from fetched occurrence count (expected=$total, received=$($entries.Count), pages=$pages)."
    }
    [pscustomobject]@{ Entries = $entries; Total = $total; Pages = $pages }
}

function New-TestTrackSequence($Entries) {
    $tracks = @($Entries | ForEach-Object {
        if ($_.track.type -ceq "track" -and -not $_.track.is_local -and
            $_.track.uri -cmatch "^spotify:track:[a-zA-Z0-9]{22}$" -and
            $_.track.name -and $_.track.artists -and $_.track.duration_ms -gt 0) {
            [string]$_.track.uri
        }
    } | Select-Object -Unique)
    if ($tracks.Count -eq 0) { throw "No suitable saved tracks for the test playlist." }
    for ($index = 0; $index -lt 1251; $index++) { $tracks[$index % $tracks.Count] }
}

function Invoke-TestPlaylistPost([string]$Path, [string]$Token, $Body) {
    $allowedCreation = $CreateTestPlaylist -and $Path -ceq "/me/playlists"
    $allowedAppend = ($CreateTestPlaylist -or $CompleteTestPlaylist) -and
        $script:createdTestPlaylistId -and
        $Path -ceq "/playlists/$script:createdTestPlaylistId/items"
    if (-not ($allowedCreation -or $allowedAppend)) {
        throw "Rejected playlist write destination."
    }
    try {
        Invoke-RestMethod -Method Post -Uri "https://api.spotify.com/v1$Path" `
            -Headers @{ Authorization = "Bearer $Token" } -ContentType "application/json" `
            -Body ($Body | ConvertTo-Json -Depth 5 -Compress) -TimeoutSec 30 `
            -MaximumRedirection 0 -Verbose:$false -Debug:$false
    } catch {
        $status = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
        $exceptionType = $_.Exception.GetType().Name
        throw "Playlist write failed (HTTP $status, exception=$exceptionType). Not retried: remote state may be partial."
    }
}

function New-SpotifyTestPlaylist([string]$Token) {
    $liked = Read-SpotifyPages "https://api.spotify.com/v1/me/tracks?limit=50" $Token
    $sequence = @(New-TestTrackSequence $liked.Entries)
    $created = Invoke-TestPlaylistPost "/me/playlists" $Token @{
        name = "Ryotune-Omni - Prueba de importacion"
        public = $false
        description = "Private import feasibility fixture: 1251 entries from saved tracks, with intentional repeats. No audio downloads."
    }
    if ([string]$created.id -cnotmatch "^[a-zA-Z0-9]{22}$") {
        throw "Creation response missing a valid playlist ID. Check Spotify before retrying creation."
    }
    $script:createdTestPlaylistId = [string]$created.id
    Write-Host "Created test playlist (visibility not yet verified): https://open.spotify.com/playlist/$script:createdTestPlaylistId"
    Write-Host "If interrupted, the playlist remains in Spotify. Do not rerun creation blindly."
    $visibilityCheck = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$script:createdTestPlaylistId" $Token
    if ($visibilityCheck.public -ne $false) {
        throw "Creation requested public=false but readback did not confirm it. No songs added; check visibility in Spotify."
    }
    Write-Host "Profile publication flag public=false verified; this flag alone does not restrict link access."
    Complete-PlaylistContents $Token $sequence 0
}

function Complete-PlaylistContents([string]$Token, [string[]]$Sequence, [int]$StartOffset) {
    for ($offset = $StartOffset; $offset -lt $Sequence.Count; $offset += 100) {
        Start-Sleep -Seconds 2
        $end = [Math]::Min($offset + 99, $sequence.Count - 1)
        $reply = Invoke-TestPlaylistPost "/playlists/$script:createdTestPlaylistId/items" $Token @{
            uris = @($sequence[$offset..$end])
        }
        if (-not $reply.snapshot_id) { throw "Append response missing snapshot. Do not retry the batch blindly." }
        Write-Host "Test playlist: $($end + 1)/1251 entries acknowledged."
    }
    $before = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$script:createdTestPlaylistId" $Token
    if (-not $before.snapshot_id) { throw "Playlist snapshot missing." }
    $readback = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$script:createdTestPlaylistId/items?limit=50" $Token
    $after = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$script:createdTestPlaylistId" $Token
    if ($before.snapshot_id -cne $after.snapshot_id) { throw "Test playlist changed during verification." }
    if ($readback.Total -ne 1251) { throw "Test playlist does not contain 1251 entries." }
    for ($index = 0; $index -lt $sequence.Count; $index++) {
        if ($readback.Entries[$index].item.uri -cne $sequence[$index]) {
            throw "Readback differs from written URI at position $($index + 1)."
        }
    }
    Write-Output "Created fixture: 1251/1251 entries verified in exact written order, including repeats."
    Write-Output ("Fixture summary: " + ((Get-OccurrenceSummary $readback.Entries "item") | ConvertTo-Json -Depth 5 -Compress))
}

function Resume-TestPlaylist([string]$Token) {
    $target = "5nhKdGJM1WKFfbnI03shqd"
    $before = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$target" $Token
    if (-not $before.snapshot_id -or $before.public -ne $false -or
        $before.name -cne "Ryotune-Omni - Prueba de importacion") {
        $nameMatches = $before.name -ceq "Ryotune-Omni - Prueba de importacion"
        $hasSnapshot = [bool]$before.snapshot_id
        $visibility = if ($null -eq $before.public) { "null-or-absent" } else { [string]$before.public }
        throw "Test playlist guard failed (nameMatches=$nameMatches, hasSnapshot=$hasSnapshot, public=$visibility). No writes performed."
    }
    $existing = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$target/items?limit=50" $Token
    if ($existing.Total -lt 500 -or $existing.Total -gt 1251) {
        throw "Unexpected existing fixture size. No writes performed."
    }
    $sequence = @(New-TestTrackSequence @($existing.Entries | ForEach-Object {
        @{ track = $_.item }
    }))
    for ($index = 0; $index -lt $existing.Total; $index++) {
        if ($existing.Entries[$index].item.uri -cne $sequence[$index]) {
            throw "Existing playlist does not match the fixture sequence. No writes performed."
        }
    }
    $after = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$target" $Token
    if ($before.snapshot_id -cne $after.snapshot_id) {
        throw "Playlist changed during resume checks. No writes performed."
    }
    $script:createdTestPlaylistId = $target
    Write-Host "Verified existing prefix: $($existing.Total) entries. Appending only remaining entries."
    Complete-PlaylistContents $Token $sequence $existing.Total
}

function Get-OccurrenceSummary($Entries, [string]$ItemField) {
    $seen = [System.Collections.Generic.HashSet[string]]::new()
    $duplicates = 0
    $missing = 0
    $unsupported = 0
    $incomplete = 0
    $details = [System.Collections.Generic.List[object]]::new()
    $ordinal = 0
    foreach ($entry in $Entries) {
        $ordinal++
        $item = if ($ItemField) { $entry.$ItemField } else { $entry }
        if ($null -eq $item) { $missing++; continue }
        if ($entry.is_local -or $item.is_local -or $item.type -cne "track") {
            $unsupported++
            continue
        }
        $fields = [System.Collections.Generic.List[string]]::new()
        if ([string]::IsNullOrWhiteSpace($item.uri)) { $fields.Add("uri") }
        if ([string]::IsNullOrWhiteSpace($item.name)) { $fields.Add("name") }
        if (-not $item.artists -or @($item.artists | Where-Object {
            [string]::IsNullOrWhiteSpace($_.name)
        }).Count -gt 0) { $fields.Add("artists") }
        if ($null -eq $item.duration_ms -or $item.duration_ms -le 0) { $fields.Add("duration_ms") }
        if ($fields.Count -gt 0) {
            $incomplete++
            $details.Add([pscustomobject]@{ Position = $ordinal; Fields = $fields.ToArray() })
        }
        if ($item.uri -and -not $seen.Add([string]$item.uri)) { $duplicates++ }
    }
    [pscustomobject]@{
        Occurrences = $Entries.Count
        RepeatedOccurrences = $duplicates
        MissingItems = $missing
        UnsupportedItems = $unsupported
        IncompleteMetadata = $incomplete
        IncompleteDetails = $details.ToArray()
    }
}

function Assert-SameOccurrenceOrder($First, $Second, [string]$ItemField) {
    if ($First.Count -ne $Second.Count) { throw "Occurrence count changed between reads." }
    for ($index = 0; $index -lt $First.Count; $index++) {
        $left = if ($ItemField) { $First[$index].$ItemField } else { $First[$index] }
        $right = if ($ItemField) { $Second[$index].$ItemField } else { $Second[$index] }
        $leftKey = @(($null -eq $left), $left.uri, $left.id, $left.type, $left.name,
            $left.duration_ms, $First[$index].is_local) | ConvertTo-Json -Compress
        $rightKey = @(($null -eq $right), $right.uri, $right.id, $right.type, $right.name,
            $right.duration_ms, $Second[$index].is_local) | ConvertTo-Json -Compress
        if ($leftKey -cne $rightKey) {
            throw "Occurrence identity or metadata changed at position $($index + 1)."
        }
    }
}

function Test-PlaylistListing([string]$Token, [string[]]$SelectedIds) {
    $first = Invoke-SpotifyGet "https://api.spotify.com/v1/me/playlists?limit=50" $Token
    Write-Output "Listing diagnostic: declared=$($first.total), returned=$(@($first.items).Count), hasNext=$([bool]$first.next)."
    for ($offset = 0; $offset -lt [Math]::Min([int]$first.total, 10); $offset++) {
        $page = Invoke-SpotifyGet "https://api.spotify.com/v1/me/playlists?limit=1&offset=$offset" $Token
        $selectedMatches = @($page.items | Where-Object { $_.id -cin $SelectedIds }).Count
        Write-Output "Listing offset $offset`: returned=$(@($page.items).Count), declared=$($page.total), hasNext=$([bool]$page.next), selectedMatches=$selectedMatches."
    }
    Write-Output "Diagnostic samples only; not a complete listing or a replacement for the failed check."
}

function Read-ListingSnapshot([string]$Token, [int]$PageSize) {
    $ids = [System.Collections.Generic.List[string]]::new()
    $total = $null
    $url = "https://api.spotify.com/v1/me/playlists?limit=$PageSize"
    $visited = [System.Collections.Generic.HashSet[string]]::new()
    $pages = 0
    $references = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    $showNames = $ShowPlaylistNames -and $PageSize -eq 1 -and -not $script:listingNamesPrinted
    while ($url) {
        if ($pages -ge $MaxPages -or -not $visited.Add($url)) {
            throw "Listing diagnostic exceeded page limit or repeated a continuation."
        }
        $page = Invoke-SpotifyGet $url $Token
        if ($null -eq $page.items -or $null -eq $page.total -or
            $page.offset -ne $pages * $PageSize -or
            -not $page.PSObject.Properties["next"]) {
            throw "Unexpected diagnostic listing page."
        }
        if ($null -eq $total) { $total = [int]$page.total }
        if ($page.total -ne $total) { throw "Listing total changed within a traversal." }
        foreach ($entry in $page.items) {
            if ([string]$entry.id -cnotmatch "^[a-zA-Z0-9]{22}$") {
                throw "Listing contains a missing or invalid playlist ID."
            }
            $ids.Add([string]$entry.id)
            if ($showNames) {
                if (-not $references.ContainsKey([string]$entry.id)) {
                    $references.Add([string]$entry.id, "P$($references.Count + 1)")
                }
                $title = [string]$entry.name | ConvertTo-Json -Compress
                $kind = [string]$entry.type | ConvertTo-Json -Compress
                Write-Host "Playlist position=$($ids.Count), reference=$($references[[string]$entry.id]), type=$kind, title=$title."
            }
        }
        Write-Host "Listing page: size=$PageSize, offset=$($page.offset), returned=$(@($page.items).Count), total=$total, hasNext=$([bool]$page.next)."
        $url = [string]$page.next
        $pages++
    }
    $unique = [System.Collections.Generic.HashSet[string]]::new([string[]]$ids, [StringComparer]::Ordinal)
    if ($showNames) { $script:listingNamesPrinted = $true }
    [pscustomobject]@{
        Ids = $ids
        Total = $total
        Received = $ids.Count
        Distinct = $unique.Count
        Complete = $ids.Count -eq $total -and $unique.Count -eq $total
    }
}

function Compare-ListingSnapshots($First, $Second) {
    $left = [System.Collections.Generic.HashSet[string]]::new([string[]]$First.Ids, [StringComparer]::Ordinal)
    $right = [System.Collections.Generic.HashSet[string]]::new([string[]]$Second.Ids, [StringComparer]::Ordinal)
    $leftOnly = @($left | Where-Object { -not $right.Contains($_) }).Count
    $rightOnly = @($right | Where-Object { -not $left.Contains($_) }).Count
    [pscustomobject]@{
        SameDeclaredTotal = $First.Total -eq $Second.Total
        SameOrder = ($First.Ids -join ",") -ceq ($Second.Ids -join ",")
        FirstReceived = $First.Received
        SecondReceived = $Second.Received
        FirstUnique = $First.Distinct
        SecondUnique = $Second.Distinct
        OnlyInFirst = $leftOnly
        OnlyInSecond = $rightOnly
        BothComplete = $First.Complete -and $Second.Complete
    }
}

function Test-ListingConsistency([string]$Token) {
    $script:listingNamesPrinted = $false
    $snapshots = @{}
    foreach ($size in @(50, 2, 1)) {
        $snapshots[$size] = Read-ListingSnapshot $Token $size
    }
    $allConsistent = $true
    foreach ($size in @(50, 2, 1)) {
        $second = Read-ListingSnapshot $Token $size
        $repeat = Compare-ListingSnapshots $snapshots[$size] $second
        Write-Output ("Repeated traversal size=$size`: " + ($repeat | ConvertTo-Json -Compress))
        $cross = Compare-ListingSnapshots $snapshots[1] $second
        Write-Output ("Compared with initial size=1, size=$size`: " + ($cross | ConvertTo-Json -Compress))
        if (-not $repeat.SameDeclaredTotal -or -not $repeat.SameOrder -or
            -not $repeat.BothComplete -or -not $cross.SameDeclaredTotal -or
            -not $cross.SameOrder -or -not $cross.BothComplete) { $allConsistent = $false }
    }
    if (-not $allConsistent) {
        throw "Listing inconsistency confirmed. Diagnostic comparisons are not a complete discovery pass."
    }
    Write-Output "All listing traversals complete, unique and equal in order across page sizes."
}

function Invoke-ReadCheck([string]$Label, [scriptblock]$Read) {
    try {
        & $Read | ForEach-Object { Write-Host $_ }
        Write-Host "$Label`: PASS (API read and count checks only)."
        return $true
    } catch {
        Write-Host "$Label`: FAIL. $($_.Exception.Message)"
        return $false
    }
}

function Get-MatchSample($Entries, [int]$SourceNumber, [int]$Limit) {
    $count = [Math]::Min($Limit, $Entries.Count)
    for ($index = 0; $index -lt $count; $index++) {
        $position = if ($count -eq 1) { 0 } else {
            [int][Math]::Floor($index * ($Entries.Count - 1) / ($count - 1))
        }
        $entry = $Entries[$position]
        $item = $entry.item
        $status = if ($null -eq $item) { "source_error" } elseif (
            $entry.is_local -or $item.is_local -or $item.type -cne "track"
        ) { "unsupported" } else { "ready" }
        [pscustomobject]@{
            source = $SourceNumber
            position = $position + 1
            title = [string]$item.name
            artists = @($item.artists | ForEach-Object { [string]$_.name })
            duration_ms = if ($null -eq $item.duration_ms) { $null } else { [long]$item.duration_ms }
            status = $status
        }
    }
}

function Get-MatchIdentity($Sample) {
    if ($Sample.status -cne "ready" -or -not $Sample.title -or -not $Sample.artists) {
        return "unavailable:$($Sample.source):$($Sample.position)"
    }
    $title = [regex]::Replace(
        ([string]$Sample.title).Normalize().ToLowerInvariant(), '[^\p{L}\p{N}]+', ' '
    ).Trim()
    $artists = @($Sample.artists | ForEach-Object {
        [regex]::Replace(([string]$_).Normalize().ToLowerInvariant(), '[^\p{L}\p{N}]+', ' ').Trim()
    } | Sort-Object)
    ConvertTo-Json -InputObject @($title, $artists) -Compress -Depth 4
}

function Read-ExcludedMatchIdentities([string]$Path) {
    $rows = @(Get-Content -LiteralPath $Path -Encoding utf8 | Where-Object {
        $_.StartsWith('{"source":')
    } | ForEach-Object { $_ | ConvertFrom-Json })
    if ($rows.Count -ne 15 -or @($rows | Where-Object { -not $_.review.original }).Count -gt 0) {
        throw "Expected the baseline log with exactly 15 reviewed source occurrences."
    }
    $identities = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($row in $rows) {
        $null = $identities.Add((Get-MatchIdentity $row.review.original))
    }
    return ,$identities
}

function Get-HoldoutMatchSample(
    $Entries, [int]$SourceNumber, [int]$Limit,
    [System.Collections.Generic.HashSet[string]]$Excluded
) {
    $pool = [System.Collections.Generic.List[object]]::new()
    $unique = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($sample in @(Get-MatchSample $Entries $SourceNumber $Entries.Count)) {
        $identity = Get-MatchIdentity $sample
        if (-not $Excluded.Contains($identity) -and $unique.Add($identity)) { $pool.Add($sample) }
    }
    if ($pool.Count -lt $Limit) { throw "Not enough distinct unseen source metadata for the holdout sample." }
    for ($index = 0; $index -lt $Limit; $index++) {
        $selected = $pool[[int][Math]::Floor(($index + 0.5) * $pool.Count / $Limit)]
        $null = $Excluded.Add((Get-MatchIdentity $selected))
        $selected
    }
}

function Invoke-MatchingLab([string]$Executable, $Samples) {
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.ArgumentList.Add("--live")
    if ($ReviewMatches) { $start.ArgumentList.Add("--review") }
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardInputEncoding = [System.Text.UTF8Encoding]::new($false)
    $start.StandardOutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    $started = $false
    try {
        $null = $process.Start()
        $started = $true
        $outputTask = $process.StandardOutput.ReadToEndAsync()
        $errorTask = $process.StandardError.ReadToEndAsync()
        $process.StandardInput.Write((ConvertTo-Json -InputObject @($Samples) -Depth 6 -Compress))
        $process.StandardInput.Close()
        if (-not $process.WaitForExit($Samples.Count * 32000 + 10000)) {
            $process.Kill($true)
            throw "Matching process exceeded its deadline."
        }
        if ($process.ExitCode -ne 0) { throw "Matching process failed; no full matching result." }
        $rows = @($outputTask.Result.Split("`n", [System.StringSplitOptions]::RemoveEmptyEntries) |
            ForEach-Object { $_ | ConvertFrom-Json })
        if ($rows.Count -ne $Samples.Count) { throw "Matching results do not account for every sample." }
        $allowed = @("matched", "low_confidence", "not_found", "unsupported", "source_error", "search_error")
        for ($index = 0; $index -lt $rows.Count; $index++) {
            if ($rows[$index].source -ne $Samples[$index].source -or
                $rows[$index].position -ne $Samples[$index].position -or
                $rows[$index].outcome -cnotin $allowed) {
                throw "Unexpected matching result identity or outcome."
            }
            Write-Output ($rows[$index] | ConvertTo-Json -Depth 10 -Compress)
        }
        Write-Output "Heuristic outcomes only: no recording accuracy measured without human review. No playlist import or downloads."
        if (@($rows | Where-Object { $_.outcome -ceq "search_error" }).Count -gt 0) {
            throw "One or more searches failed; do not count them as missing songs."
        }
    } finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true) }
        $process.Dispose()
    }
}

if ($SelfTest) {
    $sampleEntries = @(
        @{ item = @{ name = "first"; type = "track"; artists = @(@{ name = "artist" }); duration_ms = 100 } },
        @{ item = $null },
        @{ item = @{ name = "last"; type = "episode" } }
    )
    $samples = @(Get-MatchSample $sampleEntries 2 3)
    if ($samples.Count -ne 3 -or $samples[0].position -ne 1 -or
        $samples[1].status -cne "source_error" -or $samples[2].status -cne "unsupported") {
        throw "Match sample occurrence accounting failed."
    }
    $samples = @(Get-MatchSample $sampleEntries 2 2)
    if ($samples[0].position -ne 1 -or $samples[1].position -ne 3) {
        throw "Match sample spacing failed."
    }
    $holdoutEntries = @(for ($index = 0; $index -lt 12; $index++) {
        @{ item = @{
            name = "Synthetic track $index"; type = "track"
            artists = @(@{ name = "Artist" }); duration_ms = 1000
        } }
    })
    $baseline = @(Get-MatchSample $holdoutEntries 1 5)
    $excluded = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($sample in $baseline) { $null = $excluded.Add((Get-MatchIdentity $sample)) }
    $initial = $excluded.Count
    $heldout = @(Get-HoldoutMatchSample $holdoutEntries 1 5 $excluded)
    if ($heldout.Count -ne 5 -or $excluded.Count -ne $initial + 5) {
        throw "Holdout identities were not distinct from baseline."
    }
    foreach ($sample in $heldout) {
        if ($sample.position -cin $baseline.position) { throw "Baseline occurrence reused in holdout." }
    }
    $equivalent = $heldout[0].PSObject.Copy()
    $equivalent.title = $equivalent.title.ToUpperInvariant()
    if ((Get-MatchIdentity $equivalent) -cne (Get-MatchIdentity $heldout[0])) {
        throw "Identity normalization test failed."
    }
    $listingA = [pscustomobject]@{
        Ids = @("a", "b", "c"); Total = 3; Received = 3; Distinct = 3; Complete = $true
    }
    $listingB = [pscustomobject]@{
        Ids = @("a", "c"); Total = 3; Received = 2; Distinct = 2; Complete = $false
    }
    $comparison = Compare-ListingSnapshots $listingA $listingB
    if ($comparison.SameOrder -or $comparison.BothComplete -or
        $comparison.OnlyInFirst -ne 1 -or $comparison.OnlyInSecond -ne 0) {
        throw "Listing omission comparison failed."
    }
    $comparison = Compare-ListingSnapshots $listingA $listingA
    if (-not $comparison.SameOrder -or -not $comparison.BothComplete) {
        throw "Equal listing comparison failed."
    }
    $listingB.Ids = @("a", "a", "c")
    $listingB.Received = 3
    $comparison = Compare-ListingSnapshots $listingA $listingB
    if ($comparison.SameOrder -or $comparison.OnlyInFirst -ne 1 -or
        $comparison.SecondUnique -ne 2) { throw "Listing duplicate comparison failed." }
    $fixtureTracks = @(
        @{ track = @{ type = "track"; uri = "spotify:track:0000000000000000000001"; name = "one"; artists = @("test"); duration_ms = 100 } },
        @{ track = @{ type = "track"; uri = "spotify:track:0000000000000000000002"; name = "two"; artists = @("test"); duration_ms = 100 } },
        @{ track = $null }
    )
    $sequence = @(New-TestTrackSequence $fixtureTracks)
    if ($sequence.Count -ne 1251 -or $sequence[1250] -cne $fixtureTracks[0].track.uri -or
        $sequence[1] -cne $fixtureTracks[1].track.uri) { throw "Test track sequence failed." }
    $rejected = $false
    try { $null = New-TestTrackSequence @(@{ track = $null }) } catch { $rejected = $true }
    if (-not $rejected) { throw "Empty test track source accepted." }
    $rejected = $false
    try { $null = Invoke-TestPlaylistPost "/playlists/existing/items" "fake" @{} } catch {
        $rejected = $_.Exception.Message -ceq "Rejected playlist write destination."
    }
    if (-not $rejected) { throw "Unrelated playlist write accepted." }
    $expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    if ((Get-PkceChallenge "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk") -cne $expected) {
        throw "PKCE test failed."
    }
    if ((Read-Callback "/login?code=test&state=expected" "expected") -cne "test") {
        throw "Callback test failed."
    }
    foreach ($target in @(
        "/login?code=test&state=wrong",
        "/login?code=test&state=expected&state=expected",
        "/login?error=access_denied&state=expected",
        "/other?code=test&state=expected",
        "/login?state=expected"
    )) {
        $rejected = $false
        try { $null = Read-Callback $target "expected" } catch { $rejected = $true }
        if (-not $rejected) { throw "Invalid callback accepted." }
    }
    $track = [pscustomobject]@{
        uri = "spotify:track:test"; name = "test"; artists = @(@{ name = "test" }); duration_ms = 1000; type = "track"
    }
    $summary = Get-OccurrenceSummary @(
        @{ item = $track }, @{ item = $track }, @{ item = $null },
        @{ item = @{ type = "episode" } }, @{ item = @{ type = "track" } }
    ) "item"
    if ($summary.Occurrences -ne 5 -or $summary.RepeatedOccurrences -ne 1 -or
        $summary.MissingItems -ne 1 -or $summary.UnsupportedItems -ne 1 -or
        $summary.IncompleteMetadata -ne 1) { throw "Occurrence summary test failed." }
    if ($summary.IncompleteDetails[0].Position -ne 5 -or
        ($summary.IncompleteDetails[0].Fields -join ",") -cne "uri,name,artists,duration_ms") {
        throw "Incomplete field diagnostics test failed."
    }
    $firstOrder = @(@{ item = $track }, @{ item = $null }, @{ item = $track })
    Assert-SameOccurrenceOrder $firstOrder $firstOrder "item"
    $rejected = $false
    try {
        Assert-SameOccurrenceOrder $firstOrder @(@{ item = $null }, @{ item = $track }, @{ item = $track }) "item"
    } catch { $rejected = $true }
    if (-not $rejected) { throw "Changed occurrence order accepted." }
    foreach ($destination in @(
        "https://example.com/v1/me", "http://api.spotify.com/v1/me",
        "https://api.spotify.com:444/v1/me", "https://api.spotify.com/not-api"
    )) {
        $rejected = $false
        try { $null = Invoke-SpotifyGet $destination "fake" } catch {
            $rejected = $_.Exception.Message -ceq "Rejected API destination."
        }
        if (-not $rejected) { throw "Unsafe API destination accepted." }
    }
    function Invoke-SpotifyGet([string]$Url, [string]$Token) {
        if ($Url -match "^https://api\.spotify\.com/v1/me/playlists\?limit=(\d+)(?:&offset=(\d+))?$") {
            $limit = [int]$Matches[1]
            $offset = if ($Matches[2]) { [int]$Matches[2] } else { 0 }
            $all = @("0000000000000000000001", "0000000000000000000002", "0000000000000000000003")
            $ids = if ($limit -eq 50) { @($all[0], $all[2]) } else {
                @($all | Select-Object -Skip $offset -First $limit)
            }
            return [pscustomobject]@{
                items = @($ids | ForEach-Object { [pscustomobject]@{ id = $_ } })
                total = 3
                offset = $offset
                next = if ($offset + $limit -lt 3) {
                    "https://api.spotify.com/v1/me/playlists?limit=$limit&offset=$($offset + $limit)"
                } else { $null }
            }
        }
        if ($Url.StartsWith("large:")) {
            $offset = [int]$Url.Substring(6)
            $count = [Math]::Min(50, 1251 - $offset)
            return [pscustomobject]@{
                items = @(for ($n = 0; $n -lt $count; $n++) { ($offset + $n) % 97 })
                total = 1251
                offset = $offset
                next = if ($offset + $count -lt 1251) { "large:$($offset + $count)" } else { $null }
            }
        }
        switch ($Url) {
            "first" { [pscustomobject]@{ items = @("one", "two"); total = 3; offset = 0; next = "last" } }
            "last" { [pscustomobject]@{ items = @("one"); total = 3; offset = 2; next = $null } }
            "bad-count" { [pscustomobject]@{ items = @("one"); total = 2; offset = 0; next = $null } }
            "bad-offset" { [pscustomobject]@{ items = @("one"); total = 1; offset = 1; next = $null } }
            "loop" { [pscustomobject]@{ items = @("one"); total = 2; offset = 0; next = "loop" } }
            "empty" { [pscustomobject]@{ items = @(); total = 0; offset = 0; next = $null } }
            default { throw "Unexpected test request." }
        }
    }
    $result = Read-SpotifyPages "first" "fake"
    if ($result.Pages -ne 2 -or ($result.Entries -join ",") -cne "one,two,one") {
        throw "Pagination did not preserve occurrence order."
    }
    $wide = Read-ListingSnapshot "fake" 50
    $single = Read-ListingSnapshot "fake" 1
    $pair = Read-ListingSnapshot "fake" 2
    if ($wide.Complete -or -not $single.Complete -or -not $pair.Complete -or
        $single.Distinct -ne 3 -or ($single.Ids -join ",") -cne ($pair.Ids -join ",")) {
        throw "Page-size-sensitive listing diagnostics failed."
    }
    $empty = Read-SpotifyPages "empty" "fake"
    if ($empty.Total -ne 0 -or $empty.Entries.Count -ne 0 -or $empty.Pages -ne 1) {
        throw "Empty library test failed."
    }
    foreach ($start in @("bad-count", "bad-offset", "loop")) {
        $rejected = $false
        try { $null = Read-SpotifyPages $start "fake" } catch { $rejected = $true }
        if (-not $rejected) { throw "Invalid pagination accepted." }
    }
    $large = Read-SpotifyPages "large:0" "fake"
    if ($large.Total -ne 1251 -or $large.Pages -ne 26) { throw "Large pagination test failed." }
    for ($index = 0; $index -lt 1251; $index++) {
        if ($large.Entries[$index] -ne $index % 97) { throw "Large pagination changed source order." }
    }
    Write-Output "PASS: PKCE, callbacks, API destinations, field diagnostics, occurrence order and pagination (1,251 entries). No network requests."
    if (-not (Invoke-ReadCheck "Successful check fixture" { "Aggregate result" })) {
        throw "Successful read check failed."
    }
    if (Invoke-ReadCheck "Failed check fixture" { throw "Expected fixture failure." }) {
        throw "Failed read check was accepted."
    }
    if ($MatcherPath) {
        Invoke-MatchingLab (Resolve-Path -LiteralPath $MatcherPath).Path @(
            [pscustomobject]@{
                source = 1; position = 7; title = ""; artists = @()
                duration_ms = $null; status = "source_error"
            }
        )
    }
    return
}

if (-not $ClientId) { throw "Provide -ClientId; no Client Secret is used." }
if ($ReviewMatches -and -not $MatchSample) {
    throw "-ReviewMatches requires -MatchSample and prints source/candidate metadata for review."
}
if ($ExcludeMatchReviewPath -and -not $MatchSample) {
    throw "-ExcludeMatchReviewPath requires -MatchSample."
}
if ($MatchSample) {
    if (-not $PlaylistId -or $PlaylistId.Count -gt 3 -or $CreateTestPlaylist -or
        $CompleteTestPlaylist -or $InspectPlaylistOnly -or $ListingOnly) {
        throw "-MatchSample requires one to three playlist IDs and cannot be combined with other modes."
    }
    if (-not $MatcherPath) {
        $MatcherPath = Join-Path $PSScriptRoot "../../target/debug/examples/catalog_match_lab.exe"
    }
    if (-not (Test-Path -LiteralPath $MatcherPath -PathType Leaf)) {
        throw "Build catalog_match_lab first or provide -MatcherPath."
    }
    $MatcherPath = (Resolve-Path -LiteralPath $MatcherPath).Path
    if ($ExcludeMatchReviewPath) {
        $excludedMatchIdentities = Read-ExcludedMatchIdentities $ExcludeMatchReviewPath
        Write-Host "Holdout selection enabled: baseline metadata identities excluded; matcher rules unchanged."
    }
}
if ($ShowPlaylistNames -and -not $ListingOnly) {
    throw "-ShowPlaylistNames requires -ListingOnly."
}
if ($ListingOnly -and ($CreateTestPlaylist -or $CompleteTestPlaylist -or $InspectPlaylistOnly)) {
    throw "-ListingOnly cannot be combined with other modes."
}
if ($InspectPlaylistOnly -and (-not $PlaylistId -or $CreateTestPlaylist)) {
    throw "-InspectPlaylistOnly requires -PlaylistId and cannot be combined with creation."
}
if ($CompleteTestPlaylist -and ($CreateTestPlaylist -or $InspectPlaylistOnly -or $PlaylistId)) {
    throw "-CompleteTestPlaylist is restricted to the previously created fixture and cannot be combined with other modes."
}
$redirectUri = "http://127.0.0.1:8989/login"
$scopes = "playlist-read-private playlist-read-collaborative user-library-read"
if ($MatchSample) { $scopes = "playlist-read-private playlist-read-collaborative" }
if ($CreateTestPlaylist -or $CompleteTestPlaylist) { $scopes += " playlist-modify-private" }
$verifier = New-RandomValue
$state = New-RandomValue
$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 8989)
$token = $null
$tokenResponse = $null
$code = $null

try {
    $listener.Start()
    $parameters = [ordered]@{
        client_id = $ClientId
        response_type = "code"
        redirect_uri = $redirectUri
        scope = $scopes
        state = $state
        code_challenge_method = "S256"
        code_challenge = Get-PkceChallenge $verifier
    }
    $query = ($parameters.GetEnumerator() | ForEach-Object {
        "{0}={1}" -f $_.Key, [uri]::EscapeDataString($_.Value)
    }) -join "&"
    if ($CreateTestPlaylist -or $CompleteTestPlaylist) {
        Write-Host "Opening Spotify authorization: library read and private playlist write permissions."
    } else {
        Write-Host "Opening Spotify authorization. Only library read permissions are requested."
    }
    Start-Process ("https://accounts.spotify.com/authorize?" + $query)
    $deadline = [DateTime]::UtcNow.AddMinutes(5)
    while (-not $code) {
        $remaining = [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds
        if ($remaining -le 0) { throw "Authorization timed out." }
        $accept = $listener.AcceptTcpClientAsync()
        if (-not $accept.Wait($remaining)) { throw "Authorization timed out." }
        $connection = $accept.Result
        try {
            $stream = $connection.GetStream()
            $requestBytes = [System.Collections.Generic.List[byte]]::new()
            while ($requestBytes.Count -lt 8192) {
                $buffer = [byte[]]::new(1)
                $read = $stream.ReadAsync($buffer, 0, 1)
                $remaining = [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds
                if ($remaining -le 0 -or -not $read.Wait([Math]::Min(5000, $remaining))) {
                    throw "Callback request timed out."
                }
                if ($read.Result -eq 0) { break }
                $requestBytes.Add($buffer[0])
                if ($buffer[0] -eq 10) { break }
            }
            $line = [System.Text.Encoding]::ASCII.GetString($requestBytes.ToArray())
            $parts = $line.Trim().Split(" ")
            $valid = $false
            if ($parts.Count -eq 3 -and $parts[0] -ceq "GET" -and
                $parts[1].StartsWith("/login?") -and $line.EndsWith("`r`n")) {
                try { $code = Read-Callback $parts[1] $state; $valid = $true } catch {
                    $code = $null
                }
            }
            $message = if ($valid) { "Authorization received. You can close this tab." } else {
                "Callback rejected. Return to the Spotify authorization tab."
            }
            $status = if ($valid) { "200 OK" } else { "400 Bad Request" }
            $response = [System.Text.Encoding]::ASCII.GetBytes(
                "HTTP/1.1 $status`r`nContent-Type: text/plain`r`nContent-Length: $($message.Length)`r`nConnection: close`r`nCache-Control: no-store`r`n`r`n$message"
            )
            $stream.WriteTimeout = 5000
            $stream.Write($response, 0, $response.Length)
        } finally { $connection.Dispose() }
    }
    $listener.Stop()
    try {
        $tokenResponse = Invoke-RestMethod -Method Post -Uri "https://accounts.spotify.com/api/token" `
            -ContentType "application/x-www-form-urlencoded" -TimeoutSec 30 -MaximumRedirection 0 `
            -Verbose:$false -Debug:$false -Body @{
                grant_type = "authorization_code"; code = $code; redirect_uri = $redirectUri
                client_id = $ClientId; code_verifier = $verifier
            }
    } catch { throw "Spotify token exchange failed. Response details suppressed." }
    $token = [string]$tokenResponse.access_token
    if (-not $token) { throw "No access token returned." }
    $granted = @(([string]$tokenResponse.scope).Split(" "))
    foreach ($scope in $scopes.Split(" ")) {
        if ($scope -cnotin $granted) { throw "A required read scope was not granted." }
    }
    $tokenResponse = $null
    if ($MatchSample) {
        $samples = [System.Collections.Generic.List[object]]::new()
        $sourceNumber = 0
        foreach ($selectedId in $PlaylistId) {
            $sourceNumber++
            $before = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
            if (-not $before.snapshot_id) { throw "Playlist snapshot missing." }
            $items = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$selectedId/items?limit=50&additional_types=track,episode" $token
            $after = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
            if ($before.snapshot_id -cne $after.snapshot_id) { throw "Playlist changed during sample collection." }
            $selected = if ($ExcludeMatchReviewPath) {
                @(Get-HoldoutMatchSample $items.Entries $sourceNumber $SamplePerPlaylist $excludedMatchIdentities)
            } else {
                @(Get-MatchSample $items.Entries $sourceNumber $SamplePerPlaylist)
            }
            foreach ($sample in $selected) {
                $samples.Add($sample)
            }
        }
        if ($samples.Count -eq 0) { throw "No source occurrences available for matching." }
        $token = $null
        Write-Host "Searching YouTube Music for $($samples.Count) sampled occurrences. Metadata only; no audio or writes."
        Invoke-MatchingLab $MatcherPath $samples.ToArray()
        return
    }
    if ($ListingOnly) {
        Test-ListingConsistency $token
        return
    }
    if ($CompleteTestPlaylist) {
        Resume-TestPlaylist $token
        return
    }
    if ($CreateTestPlaylist) {
        New-SpotifyTestPlaylist $token
        return
    }
    if ($InspectPlaylistOnly) {
        foreach ($selectedId in $PlaylistId) {
            $before = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
            if (-not $before.snapshot_id) { throw "Playlist snapshot missing." }
            $nameMatches = $before.name -ceq "Ryotune-Omni - Prueba de importacion"
            $visibility = if ($null -eq $before.public) { "null-or-absent" } else { [string]$before.public }
            Write-Output "Playlist identity diagnostic: testNameMatches=$nameMatches, public=$visibility."
            $items = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$selectedId/items?limit=50" $token
            $after = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
            if ($before.snapshot_id -cne $after.snapshot_id) { throw "Playlist changed during inspection." }
            Write-Output ("Existing playlist state: " + ((Get-OccurrenceSummary $items.Entries "item") | ConvertTo-Json -Depth 5 -Compress))
            if ($selectedId -ceq "5nhKdGJM1WKFfbnI03shqd" -and $items.Total -eq 1251) {
                $prefix = @($items.Entries | Select-Object -First 500 | ForEach-Object {
                    @{ track = $_.item }
                })
                $expectedCycle = @(New-TestTrackSequence $prefix)
                for ($index = 0; $index -lt $items.Total; $index++) {
                    if ($items.Entries[$index].item.uri -cne $expectedCycle[$index]) {
                        throw "Fixture cycle differs at position $($index + 1)."
                    }
                }
                $second = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$selectedId/items?limit=50" $token
                $final = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
                if ($before.snapshot_id -cne $final.snapshot_id) {
                    throw "Fixture snapshot changed during second read."
                }
                Assert-SameOccurrenceOrder $items.Entries $second.Entries "item"
                Write-Output "Fixture: 1251 entries match the cycle reconstructed from the first 500; order stable across two reads."
            }
            Write-Output "Read-only inspection completed with stable snapshot; no writes or retries performed."
        }
        return
    }
    Write-Host "Authorization succeeded. Reading metadata; names and identifiers will not be printed."
    $checks = [System.Collections.Generic.List[bool]]::new()
    $checks.Add((Invoke-ReadCheck "Playlist listing" {
        $playlists = Read-SpotifyPages "https://api.spotify.com/v1/me/playlists?limit=50" $token
        Write-Output "Playlists: $($playlists.Total) entries, $($playlists.Pages) pages."
    }))
    if (-not $checks[0]) {
        $checks.Add((Invoke-ReadCheck "Playlist listing diagnostics" {
            Test-PlaylistListing $token $PlaylistId
        }))
    }
    if ($PlaylistId) {
        $playlistNumber = 0
        foreach ($selectedId in $PlaylistId) {
            $playlistNumber++
            $checks.Add((Invoke-ReadCheck "Playlist $playlistNumber contents" {
                $before = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
                if (-not $before.snapshot_id) { throw "Playlist snapshot missing." }
                $items = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$selectedId/items?limit=50&additional_types=track,episode" $token
                $after = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
                if ($before.snapshot_id -cne $after.snapshot_id) { throw "Playlist changed during the probe." }
                $summary = Get-OccurrenceSummary $items.Entries "item"
                Write-Output ("Playlist contents: " + ($summary | ConvertTo-Json -Depth 5 -Compress))
                $secondItems = Read-SpotifyPages "https://api.spotify.com/v1/playlists/$selectedId/items?limit=50&additional_types=track,episode" $token
                $final = Invoke-SpotifyGet "https://api.spotify.com/v1/playlists/$selectedId" $token
                if ($before.snapshot_id -cne $final.snapshot_id) { throw "Playlist changed during order comparison." }
                Assert-SameOccurrenceOrder $items.Entries $secondItems.Entries "item"
                Write-Output "Occurrence order and identity stable across two API reads; UI comparison still pending."
            }))
        }
    } else {
        Write-Output "Playlist contents: NOT TESTED (provide -PlaylistId for an owned/collaborative playlist)."
    }
    $checks.Add((Invoke-ReadCheck "Liked Songs" {
        $liked = Read-SpotifyPages "https://api.spotify.com/v1/me/tracks?limit=50" $token
        Write-Output ("Liked Songs: " + ((Get-OccurrenceSummary $liked.Entries "track") | ConvertTo-Json -Depth 5 -Compress))
    }))
    $checks.Add((Invoke-ReadCheck "Saved albums" {
        $albums = Read-SpotifyPages "https://api.spotify.com/v1/me/albums?limit=50" $token
        Write-Output "Saved albums: $($albums.Total) entries, $($albums.Pages) pages."
        if ($albums.Total -gt 0) {
            $albumId = [string]$albums.Entries[0].album.id
            if ($albumId -cnotmatch "^[a-zA-Z0-9]{22}$") { throw "Unexpected album identifier." }
            $albumTracks = Read-SpotifyPages "https://api.spotify.com/v1/albums/$albumId/tracks?limit=50" $token
            Write-Output ("First saved album contents: " + ((Get-OccurrenceSummary $albumTracks.Entries "") | ConvertTo-Json -Depth 5 -Compress))
        } else {
            Write-Output "Album contents: NOT TESTED (no saved album)."
        }
    }))
    if ($checks.Contains($false)) { throw "One or more independent read checks failed; see aggregate results above." }
    Write-Output "Read checks finished. Full feasibility is not established; source order still needs manual comparison. No audio or writes."
} catch {
    Write-Output "Probe stopped. No full feasibility pass."
    Write-Output $_.Exception.Message
    exit 1
} finally {
    $listener.Stop()
    $token = $null
    $tokenResponse = $null
    $code = $null
    $verifier = $null
    $state = $null
}
