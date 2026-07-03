param(
  [switch]$NoArchive
)

$ErrorActionPreference = "Stop"

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$packageJsonPath = Join-Path $projectRoot "package.json"
$packageJson = Get-Content -LiteralPath $packageJsonPath -Raw | ConvertFrom-Json
$version = $packageJson.version
$productName = $packageJson.build.productName

$electronDist = Resolve-Path (Join-Path $projectRoot "node_modules/electron/dist")
$releaseRoot = Join-Path $projectRoot "release"
$stagingRoot = Join-Path $releaseRoot "staging"
$appDir = Join-Path $stagingRoot "$productName-win-x64"
$resourcesApp = Join-Path $appDir "resources/app"
$archiveName = "ae-sdd-monitor-$version-windows-x64-installable.zip"
$archivePath = Join-Path $releaseRoot $archiveName
$setupExePath = Join-Path $releaseRoot "ae-sdd-monitor-$version-windows-x64-setup.exe"

if (Test-Path -LiteralPath $stagingRoot) {
  Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $resourcesApp -Force | Out-Null

Copy-Item -Path (Join-Path $electronDist "*") -Destination $appDir -Recurse -Force

$electronExe = Join-Path $appDir "electron.exe"
$productExe = Join-Path $appDir "$productName.exe"
if (Test-Path -LiteralPath $productExe) {
  Remove-Item -LiteralPath $productExe -Force
}
Rename-Item -LiteralPath $electronExe -NewName "$productName.exe"

Copy-Item -LiteralPath (Join-Path $projectRoot "src") -Destination $resourcesApp -Recurse -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "package.json") -Destination $resourcesApp -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "README.md") -Destination $resourcesApp -Force

$installScript = @'
param(
  [switch]$DesktopShortcut
)

$ErrorActionPreference = "Stop"
$productName = "ae-sdd Monitor"
$source = Join-Path $PSScriptRoot "$productName-win-x64"
$target = Join-Path $env:LOCALAPPDATA "Programs\$productName"

if (!(Test-Path -LiteralPath $source)) {
  throw "Package payload not found: $source"
}

if (Test-Path -LiteralPath $target) {
  Remove-Item -LiteralPath $target -Recurse -Force
}

New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $target -Recurse -Force

$exe = Join-Path $target "$productName.exe"
$startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$shortcutPath = Join-Path $startMenu "$productName.lnk"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $target
$shortcut.Description = "ae-sdd workspace monitor"
$shortcut.Save()

if ($DesktopShortcut) {
  $desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "$productName.lnk"
  $shortcut = $shell.CreateShortcut($desktopShortcut)
  $shortcut.TargetPath = $exe
  $shortcut.WorkingDirectory = $target
  $shortcut.Description = "ae-sdd workspace monitor"
  $shortcut.Save()
}

Write-Host "Installed $productName to $target"
Write-Host "Start Menu shortcut: $shortcutPath"
'@

$uninstallScript = @'
$ErrorActionPreference = "Stop"
$productName = "ae-sdd Monitor"
$target = Join-Path $env:LOCALAPPDATA "Programs\$productName"
$startMenuShortcut = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$productName.lnk"
$desktopShortcut = Join-Path ([Environment]::GetFolderPath("Desktop")) "$productName.lnk"

foreach ($path in @($startMenuShortcut, $desktopShortcut)) {
  if (Test-Path -LiteralPath $path) {
    Remove-Item -LiteralPath $path -Force
  }
}

if (Test-Path -LiteralPath $target) {
  Remove-Item -LiteralPath $target -Recurse -Force
}

Write-Host "Uninstalled $productName"
'@

Set-Content -LiteralPath (Join-Path $stagingRoot "install.ps1") -Value $installScript -Encoding UTF8
Set-Content -LiteralPath (Join-Path $stagingRoot "uninstall.ps1") -Value $uninstallScript -Encoding UTF8

if (!$NoArchive) {
  New-Item -ItemType Directory -Path $releaseRoot -Force | Out-Null
  if (Test-Path -LiteralPath $archivePath) {
    Remove-Item -LiteralPath $archivePath -Force
  }
  Compress-Archive -Path (Join-Path $stagingRoot "*") -DestinationPath $archivePath -Force

  $iexpress = Get-Command iexpress.exe -ErrorAction SilentlyContinue
  if ($iexpress) {
    $sfxRoot = Join-Path $releaseRoot "sfx"
    if (Test-Path -LiteralPath $sfxRoot) {
      Remove-Item -LiteralPath $sfxRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $sfxRoot -Force | Out-Null
    Copy-Item -LiteralPath $archivePath -Destination (Join-Path $sfxRoot "payload.zip") -Force

    $sfxCmd = @'
@echo off
setlocal
set "WORK=%TEMP%\ae-sdd-monitor-install-%RANDOM%%RANDOM%"
mkdir "%WORK%" >nul 2>nul
powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -LiteralPath '%~dp0payload.zip' -DestinationPath '%WORK%' -Force; & (Join-Path '%WORK%' 'install.ps1')"
exit /b %ERRORLEVEL%
'@
    Set-Content -LiteralPath (Join-Path $sfxRoot "install-from-sfx.cmd") -Value $sfxCmd -Encoding ASCII

    $sedPath = Join-Path $sfxRoot "setup.sed"
    $sed = @"
[Version]
Class=IEXPRESS
SEDVersion=3

[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=0
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=0
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=
DisplayLicense=
FinishMessage=
TargetName=$setupExePath
FriendlyName=ae-sdd Monitor Setup
AppLaunched=install-from-sfx.cmd
PostInstallCmd=<None>
AdminQuietInstCmd=install-from-sfx.cmd
UserQuietInstCmd=install-from-sfx.cmd
SourceFiles=SourceFiles

[Strings]
FILE0="payload.zip"
FILE1="install-from-sfx.cmd"

[SourceFiles]
SourceFiles0=$sfxRoot

[SourceFiles0]
%FILE0%=
%FILE1%=
"@
    Set-Content -LiteralPath $sedPath -Value $sed -Encoding ASCII

    if (Test-Path -LiteralPath $setupExePath) {
      Remove-Item -LiteralPath $setupExePath -Force
    }
    & $iexpress.Source /N /Q $sedPath | Out-Null
    if (!(Test-Path -LiteralPath $setupExePath)) {
      Write-Warning "IExpress setup executable was not generated; zip package is still available."
    }
  } else {
    Write-Warning "iexpress.exe not found; zip package is still available."
  }
}

Write-Host "Packaged: $appDir"
if (!$NoArchive) {
  Write-Host "Archive: $archivePath"
  if (Test-Path -LiteralPath $setupExePath) {
    Write-Host "Setup: $setupExePath"
  }
}
