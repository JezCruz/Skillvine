$envFile = 'C:\Users\JEJE\Documents\rust_projects\Skillvine\.env.local'
if (Test-Path $envFile) {
  Get-Content $envFile | ForEach-Object {
    $line = $_.Trim()
    if (-not [string]::IsNullOrWhiteSpace($line) -and -not $line.StartsWith('#')) {
      $parts = $line -split '=',2
      if ($parts.Count -ge 2) {
        $name = $parts[0].Trim()
        $value = $parts[1].Trim().Trim("'\"")
        Set-Item -Path "Env:$name" -Value $value
      }
    }
  }
}
Write-Host "Starting server with DATABASE_URL: $env:DATABASE_URL"
cargo run
