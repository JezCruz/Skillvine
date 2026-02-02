$lines = Get-Content -Raw '.env.local'
$m = [regex]::Match($lines, '(?m)^DATABASE_URL=(.*)$')
if ($m.Success) {
  $line = $m.Groups[1].Value.Trim()
  Write-Output "DATABASE_URL value (raw):"
  Write-Output $line
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($line)
  Write-Output "Hex bytes:"
  $bytes -join ' '
} else {
  Write-Output 'DATABASE_URL not found'
}
