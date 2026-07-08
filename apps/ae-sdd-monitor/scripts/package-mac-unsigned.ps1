param(
  [string[]]$Arch = @("x64", "arm64")
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$packageJsonPath = Join-Path $projectRoot "package.json"
$packageJson = Get-Content -LiteralPath $packageJsonPath -Raw | ConvertFrom-Json
$version = $packageJson.version
$productName = $packageJson.build.productName
$appId = $packageJson.build.appId
$electronPackagePath = Join-Path $projectRoot "node_modules/electron/package.json"
$electronPackage = Get-Content -LiteralPath $electronPackagePath -Raw | ConvertFrom-Json
$electronVersion = $electronPackage.version

$releaseRoot = Join-Path $projectRoot "release"
$cacheRoot = Join-Path $releaseRoot "electron-cache"
New-Item -ItemType Directory -Path $releaseRoot -Force | Out-Null
New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null
$electronMirror = $env:ELECTRON_MIRROR

function Copy-EntryBytes {
  param(
    [System.IO.Compression.ZipArchiveEntry]$SourceEntry,
    [System.IO.Compression.ZipArchiveEntry]$TargetEntry
  )
  $inputStream = $SourceEntry.Open()
  try {
    $outputStream = $TargetEntry.Open()
    try {
      $inputStream.CopyTo($outputStream)
    } finally {
      $outputStream.Dispose()
    }
  } finally {
    $inputStream.Dispose()
  }
}

function Copy-PatchedInfoPlist {
  param(
    [System.IO.Compression.ZipArchiveEntry]$SourceEntry,
    [System.IO.Compression.ZipArchiveEntry]$TargetEntry
  )
  $reader = [System.IO.StreamReader]::new($SourceEntry.Open(), [System.Text.Encoding]::UTF8)
  try {
    $text = $reader.ReadToEnd()
  } finally {
    $reader.Dispose()
  }

  $escapedName = [System.Security.SecurityElement]::Escape($productName)
  $escapedId = [System.Security.SecurityElement]::Escape($appId)
  $text = [regex]::Replace($text, "(<key>CFBundleName</key>\s*<string>)[^<]*(</string>)", "`${1}$escapedName`${2}")
  $text = [regex]::Replace($text, "(<key>CFBundleDisplayName</key>\s*<string>)[^<]*(</string>)", "`${1}$escapedName`${2}")
  $text = [regex]::Replace($text, "(<key>CFBundleIdentifier</key>\s*<string>)[^<]*(</string>)", "`${1}$escapedId`${2}")
  $text = [regex]::Replace($text, "(<key>CFBundleShortVersionString</key>\s*<string>)[^<]*(</string>)", "`${1}$version`${2}")
  $text = [regex]::Replace($text, "(<key>CFBundleVersion</key>\s*<string>)[^<]*(</string>)", "`${1}$version`${2}")

  $bytes = [System.Text.Encoding]::UTF8.GetBytes($text)
  $outputStream = $TargetEntry.Open()
  try {
    $outputStream.Write($bytes, 0, $bytes.Length)
  } finally {
    $outputStream.Dispose()
  }
}

function Add-FileToZip {
  param(
    [System.IO.Compression.ZipArchive]$Zip,
    [string]$SourcePath,
    [string]$EntryName
  )
  $entry = $Zip.CreateEntry($EntryName.Replace("\", "/"), [System.IO.Compression.CompressionLevel]::Optimal)
  # Unix regular file 0644. ZipArchiveEntry.ExternalAttributes is signed int32.
  $entry.ExternalAttributes = -2119958528
  $inputStream = [System.IO.File]::OpenRead($SourcePath)
  try {
    $outputStream = $entry.Open()
    try {
      $inputStream.CopyTo($outputStream)
    } finally {
      $outputStream.Dispose()
    }
  } finally {
    $inputStream.Dispose()
  }
}

function Test-ZipReadable {
  param([string]$Path)
  if (!(Test-Path -LiteralPath $Path)) {
    return $false
  }
  try {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
      $zip = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Read)
      try {
        [void]$zip.Entries.Count
        return $true
      } finally {
        $zip.Dispose()
      }
    } finally {
      $stream.Dispose()
    }
  } catch {
    return $false
  }
}

foreach ($arch in $Arch) {
  if ($arch -notin @("x64", "arm64")) {
    throw "Unsupported macOS arch: $arch"
  }

  $electronZip = Join-Path $cacheRoot "electron-v$electronVersion-darwin-$arch.zip"
  if ($electronMirror) {
    $baseMirror = $electronMirror.TrimEnd("/")
    $electronUrl = "$baseMirror/v$electronVersion/electron-v$electronVersion-darwin-$arch.zip"
  } else {
    $electronUrl = "https://github.com/electron/electron/releases/download/v$electronVersion/electron-v$electronVersion-darwin-$arch.zip"
  }
  if ((Test-Path -LiteralPath $electronZip) -and ((Get-Item -LiteralPath $electronZip).Length -lt 10000000)) {
    Remove-Item -LiteralPath $electronZip -Force
  }
  if ((Test-Path -LiteralPath $electronZip) -and !(Test-ZipReadable -Path $electronZip)) {
    Remove-Item -LiteralPath $electronZip -Force
  }
  if (!(Test-Path -LiteralPath $electronZip)) {
    Write-Host "Downloading $electronUrl"
    $downloadPath = "$electronZip.download"
    if (Test-Path -LiteralPath $downloadPath) {
      Remove-Item -LiteralPath $downloadPath -Force
    }
    Invoke-WebRequest -Uri $electronUrl -OutFile $downloadPath
    Move-Item -LiteralPath $downloadPath -Destination $electronZip -Force
  }

  $artifactPath = Join-Path $releaseRoot "ae-sdd-monitor-$version-macos-$arch-unsigned.zip"
  if (Test-Path -LiteralPath $artifactPath) {
    Remove-Item -LiteralPath $artifactPath -Force
  }

  $sourceStream = [System.IO.File]::OpenRead($electronZip)
  $targetStream = [System.IO.File]::Open($artifactPath, [System.IO.FileMode]::CreateNew)
  try {
    $sourceZip = [System.IO.Compression.ZipArchive]::new($sourceStream, [System.IO.Compression.ZipArchiveMode]::Read)
    $targetZip = [System.IO.Compression.ZipArchive]::new($targetStream, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
      foreach ($sourceEntry in $sourceZip.Entries) {
        $targetName = $sourceEntry.FullName -replace "^Electron\.app/", "$productName.app/"
        $targetEntry = $targetZip.CreateEntry($targetName, [System.IO.Compression.CompressionLevel]::Optimal)
        $targetEntry.ExternalAttributes = $sourceEntry.ExternalAttributes
        if ($sourceEntry.FullName.EndsWith("/")) {
          continue
        }
        if ($sourceEntry.FullName -eq "Electron.app/Contents/Info.plist") {
          Copy-PatchedInfoPlist -SourceEntry $sourceEntry -TargetEntry $targetEntry
        } else {
          Copy-EntryBytes -SourceEntry $sourceEntry -TargetEntry $targetEntry
        }
      }

      $resourcesAppPrefix = "$productName.app/Contents/Resources/app"
      Get-ChildItem -LiteralPath (Join-Path $projectRoot "src") -Recurse -File | ForEach-Object {
        $relative = $_.FullName.Substring((Join-Path $projectRoot "src").Length).TrimStart("\", "/")
        Add-FileToZip -Zip $targetZip -SourcePath $_.FullName -EntryName "$resourcesAppPrefix/src/$relative"
      }
      Get-ChildItem -LiteralPath (Join-Path $projectRoot "dist") -Recurse -File | ForEach-Object {
        $relative = $_.FullName.Substring((Join-Path $projectRoot "dist").Length).TrimStart("\", "/")
        Add-FileToZip -Zip $targetZip -SourcePath $_.FullName -EntryName "$resourcesAppPrefix/dist/$relative"
      }
      Add-FileToZip -Zip $targetZip -SourcePath (Join-Path $projectRoot "package.json") -EntryName "$resourcesAppPrefix/package.json"
      Add-FileToZip -Zip $targetZip -SourcePath (Join-Path $projectRoot "README.md") -EntryName "$resourcesAppPrefix/README.md"
    } finally {
      $targetZip.Dispose()
      $sourceZip.Dispose()
    }
  } finally {
    $targetStream.Dispose()
    $sourceStream.Dispose()
  }

  Write-Host "macOS unsigned zip: $artifactPath"
}
