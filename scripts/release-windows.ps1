param([switch]$InitializeSigningKey)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path -Parent $PSScriptRoot
$keyDirectory = Join-Path $env:USERPROFILE '.tauri\codex-guard'
$backupDirectory = Join-Path $env:LOCALAPPDATA 'CodexGuardSigningBackup'
$keyFile = Join-Path $keyDirectory 'updater-v2.key.dpapi'
$publicFile = Join-Path $keyDirectory 'updater-v2.key.pub'
Add-Type -AssemblyName System.Security

function Protect-Directory([string]$Directory) {
    [void](New-Item -ItemType Directory -Path $Directory -Force)
    $user = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
    & "$env:WINDIR\System32\icacls.exe" $Directory /inheritance:r /grant:r ('*' + $user.Value + ':(OI)(CI)F') '*S-1-5-18:(OI)(CI)F' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to restrict signing-key directory permissions' }
    foreach ($rule in (Get-Acl -LiteralPath $Directory).Access) {
        if ($rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -notin @($user.Value, 'S-1-5-18')) { throw 'Unexpected signing-key directory access rule; review permissions before proceeding' }
    }
}

if ($InitializeSigningKey) {
    if ((Test-Path -LiteralPath $keyFile) -or (Test-Path -LiteralPath $publicFile) -or (Test-Path -LiteralPath (Join-Path $backupDirectory 'updater-v2.key.dpapi'))) { throw 'Signing key or backup already exists. Restore existing keys; existing keys will not be overwritten.' }
    Protect-Directory $keyDirectory
    Protect-Directory $backupDirectory
    $temporary = Join-Path $keyDirectory ('generate-' + [Guid]::NewGuid().ToString('N') + '.key')
    $raw = $null
    try {
        # Never forward signer output: some CLI versions print key material.
        $info = New-Object Diagnostics.ProcessStartInfo
        $info.FileName = (Get-Command node.exe).Source
        $info.Arguments = '"' + (Join-Path $root 'node_modules\@tauri-apps\cli\tauri.js') + '" signer generate --ci --write-keys "' + $temporary + '"'
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        $generator = [Diagnostics.Process]::Start($info)
        $output = $generator.StandardOutput.ReadToEnd()
        $errors = $generator.StandardError.ReadToEnd()
        $generator.WaitForExit()
        if ($generator.ExitCode -ne 0) { throw 'Signing key generation failed; no key material is written to logs.' }
        $raw = [IO.File]::ReadAllBytes($temporary)
        $encrypted = [Security.Cryptography.ProtectedData]::Protect($raw, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
        [IO.File]::WriteAllBytes($keyFile, $encrypted)
        Copy-Item -LiteralPath ($temporary + '.pub') -Destination $publicFile
        Copy-Item -LiteralPath $keyFile -Destination (Join-Path $backupDirectory 'updater-v2.key.dpapi')
        Copy-Item -LiteralPath $publicFile -Destination (Join-Path $backupDirectory 'updater-v2.key.pub')
        foreach ($file in @($keyFile, (Join-Path $backupDirectory 'updater-v2.key.dpapi'))) {
            $roundtrip = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($file), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
            if ([Convert]::ToBase64String($raw) -ne [Convert]::ToBase64String($roundtrip)) { throw 'Signing key backup verification failed' }
            [Array]::Clear($roundtrip, 0, $roundtrip.Length)
        }
        Write-Output "Encrypted signing key and verified backup created. Public key: $publicFile"
    } finally {
        if ($null -ne $raw) { [Array]::Clear($raw, 0, $raw.Length) }
        # Only these two known temporary files, never a recursive directory delete.
        foreach ($file in @($temporary, ($temporary + '.pub'))) { if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force } }
        $output = $null
        $errors = $null
    }
    exit 0
}

if (!(Test-Path -LiteralPath $keyFile)) { throw "Encrypted signing key missing: $keyFile. Restore it; this script never silently creates a replacement key." }
$previousKey = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'Process')
$previousPassword = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'Process')
$plain = $null
Push-Location $root
try {
    $plain = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($keyFile), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    $env:TAURI_SIGNING_PRIVATE_KEY = [Text.Encoding]::UTF8.GetString($plain).Trim()
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
    & node scripts/release.mjs
    if ($LASTEXITCODE -ne 0) { throw "Signed release build failed ($LASTEXITCODE)" }
} finally {
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $previousKey, 'Process')
    [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', $previousPassword, 'Process')
    if ($null -ne $plain) { [Array]::Clear($plain, 0, $plain.Length) }
    Pop-Location
}
