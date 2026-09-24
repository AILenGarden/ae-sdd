param(
    [string]$OutputRoot = 'D:\al-agent-workspace\prod\DB',
    [string]$BuildRoot = (Join-Path $env:LOCALAPPDATA 'db-operator-build'),
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifest = Join-Path $sourceRoot 'native\Cargo.toml'
$versionMatch = [regex]::Match((Get-Content -LiteralPath $manifest -Raw), '(?m)^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) { throw 'Package version is missing.' }
$version = $versionMatch.Groups[1].Value
$stamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')
$packageName = "db-operator-$version-windows-x86_64-$stamp"
$packageRoot = Join-Path $OutputRoot $packageName
$target = 'x86_64-pc-windows-msvc'
$clientBuild = Join-Path $BuildRoot 'client'
$serviceBuild = Join-Path $BuildRoot 'service'

function Invoke-CheckedCargo([string[]]$CargoArgs) {
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed with exit code $LASTEXITCODE." }
}

if (-not $SkipBuild) {
    Invoke-CheckedCargo @('build', '--locked', '--release', '--manifest-path', $manifest,
        '--target', $target, '--target-dir', $clientBuild,
        '--no-default-features', '--features', 'client', '--bin', 'db-operator')
    Invoke-CheckedCargo @('build', '--locked', '--release', '--manifest-path', $manifest,
        '--target', $target, '--target-dir', $serviceBuild,
        '--no-default-features', '--features', 'admin,daemon',
        '--bin', 'db-operator-admin', '--bin', 'db-operator-daemon')
}

# Explicit payload; never copy a workspace, target directory, or user's state.
$payload = @(
    @((Join-Path $clientBuild "$target\release\db-operator.exe"), 'bin/db-operator.exe'),
    @((Join-Path $clientBuild "$target\release\db-operator.exe"), 'bin/windows-x86_64/db-operator.exe'),
    @((Join-Path $serviceBuild "$target\release\db-operator-admin.exe"), 'bin/service/windows-x86_64/db-operator-admin.exe'),
    @((Join-Path $serviceBuild "$target\release\db-operator-daemon.exe"), 'bin/service/windows-x86_64/db-operator-daemon.exe'),
    @((Join-Path $sourceRoot 'SKILL.md'), 'SKILL.md'),
    @((Join-Path $sourceRoot 'ui/index.html'), 'ui/index.html'),
    @((Join-Path $sourceRoot 'scripts/registry-server.mjs'), 'scripts/registry-server.mjs'),
    @((Join-Path $sourceRoot 'scripts/open-registry.ps1'), 'scripts/open-registry.ps1'),
    @((Join-Path $sourceRoot 'scripts/install.ps1'), 'scripts/install.ps1'),
    @((Join-Path $sourceRoot 'scripts/install-service.ps1'), 'scripts/install-service.ps1'),
    @((Join-Path $sourceRoot 'references/connection-schema.md'), 'references/connection-schema.md'),
    @((Join-Path $sourceRoot 'references/release-readme.md'), 'README.md')
)
foreach ($entry in $payload) {
    if (-not (Test-Path -LiteralPath $entry[0] -PathType Leaf)) { throw "Missing payload: $($entry[0])" }
}
if (Test-Path -LiteralPath $packageRoot) { throw 'Output already exists; refusing to replace it.' }
New-Item -ItemType Directory -Path $packageRoot | Out-Null
$fileRecords = foreach ($entry in $payload) {
    $destination = Join-Path $packageRoot $entry[1]
    $parent = Split-Path -Parent $destination
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent | Out-Null }
    Copy-Item -LiteralPath $entry[0] -Destination $destination
    $sourceHash = (Get-FileHash -LiteralPath $entry[0] -Algorithm SHA256).Hash
    $targetHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    if ($sourceHash -ne $targetHash) { throw "Copy verification failed: $($entry[1])" }
    [ordered]@{path=$entry[1]; sha256=$targetHash.ToLowerInvariant(); bytes=(Get-Item -LiteralPath $destination).Length}
}

$versionChecks = @()
foreach ($relative in @('bin/db-operator.exe', 'bin/service/windows-x86_64/db-operator-admin.exe', 'bin/service/windows-x86_64/db-operator-daemon.exe')) {
    $exe = Join-Path $packageRoot $relative
    $reported = (& $exe --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $reported.EndsWith(" $version")) { throw "Version check failed: $relative" }
    $versionChecks += $reported
}

$release = [ordered]@{
    package=$packageName; version=$version; target=$target; builtAtUtc=$stamp
    registrationDataIncluded=$false; credentialsIncluded=$false
    uiMode='local-admin-http; requires Node.js and administrator privileges'
    versionChecks=$versionChecks; files=@($fileRecords)
}
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $packageRoot 'release-manifest.json'), ($release | ConvertTo-Json -Depth 6) + "`n", $utf8)

$zipPath = "$packageRoot.zip"
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($packageRoot, $zipPath)
$zip = [IO.Compression.ZipFile]::OpenRead($zipPath)
try {
    $expected = @($fileRecords | ForEach-Object { $_.path }) + @('release-manifest.json')
    $names = @($zip.Entries | ForEach-Object { $_.FullName.Replace('\','/') })
    $difference = Compare-Object ($expected | Sort-Object) ($names | Sort-Object)
    if ($difference) { throw 'ZIP inventory does not match the approved payload.' }
} finally { $zip.Dispose() }
$zipHash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$zipPath.sha256", "$zipHash  $([IO.Path]::GetFileName($zipPath))`n", $utf8)
Write-Output "PACKAGE=$packageRoot"
Write-Output "ARCHIVE=$zipPath"
Write-Output "SHA256=$zipHash"
