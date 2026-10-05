# evict-standby.ps1 — allocate + touch ~28GB to force standby-list eviction
# (32GB RAM box; cold-cache simulation for HDD benchmarks). Frees on exit.
$l = New-Object System.Collections.Generic.List[byte[]]
for ($i = 0; $i -lt 14; $i++) {
    $b = New-Object byte[] 2147483648
    [Array]::Fill($b, [byte]1)
    $l.Add($b)
    Write-Host "touched 2GB ($($i+1)/14)"
}
$l.Clear()
[System.GC]::Collect()
Write-Host "standby eviction done"
