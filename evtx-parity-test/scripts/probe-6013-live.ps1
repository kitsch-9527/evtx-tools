$e = Get-WinEvent -FilterHashtable @{LogName='System'; ProviderName='EventLog'; Id=6013} -MaxEvents 1
Write-Output ("Id={0} Provider={1} Level={2}" -f $e.Id, $e.ProviderName, $e.LevelDisplayName)
Write-Output ("Message=[" + $e.Message + "]")
$i = 1
foreach ($p in $e.Properties) { Write-Output ("Param{0}={1}" -f $i, $p.Value); $i++ }
