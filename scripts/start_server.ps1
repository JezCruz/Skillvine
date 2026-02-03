$envFile = "C:\Users\JEJE\Documents\rust_projects\Skillvine\.env.local"

if (-Not (Test-Path $envFile)) {
    Write-Error ".env.local file not found"
    exit 1
}

Get-Content $envFile -Encoding UTF8 | ForEach-Object {
    $line = $_.Trim()

    # Skip empty lines and comments
    if ($line -and -not $line.StartsWith("#")) {
        # Split only on first '='
        $name, $value = $line -split "=", 2

        if ($name -and $value) {
            $name = $name.Trim()
            $value = $value.Trim()

            # Remove surrounding quotes
            if (
                ($value.StartsWith("'") -and $value.EndsWith("'")) -or
                ($value.StartsWith('"') -and $value.EndsWith('"'))
            ) {
                $value = $value.Substring(1, $value.Length - 2)
            }

            Set-Item -Path "Env:$name" -Value $value
        }
    }
}

Write-Host "Starting server with DATABASE_URL loaded"
cargo run
