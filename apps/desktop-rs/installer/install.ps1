<#
.SYNOPSIS
    Installs Glyphi 0.6 and removes the Electron build it replaces.

.DESCRIPTION
    A per-user install, no administrator prompt: the same place the Squirrel
    installer used, so shortcuts, the Start Menu entry and the uninstall record all
    point at one app.

    This is also where the switch from Electron happens. The old app is uninstalled
    first, through its own updater, so its files and its registry entry go the way
    Squirrel knows how to remove them; anything it leaves behind (build folders, the
    downloaded package cache, Chromium caches) is deleted here. The local database
    is kept: it holds the usage history and it is the one thing that must survive.

.PARAMETER InstallDirectory
    Where the app goes. Defaults to %LOCALAPPDATA%\Programs\Glyphi.

.PARAMETER NoShortcuts
    Skips the Start Menu and Desktop shortcuts.
#>
[CmdletBinding()]
param(
    [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA "Programs\Glyphi"),
    [switch]$NoShortcuts
)

$ErrorActionPreference = "Stop"
$source = $PSScriptRoot
$exe = Join-Path $source "glyphi.exe"
if (-not (Test-Path $exe)) { throw "glyphi.exe is missing next to install.ps1" }

$userData = Join-Path $env:APPDATA "Glyphi"
$squirrelRoot = Join-Path $env:LOCALAPPDATA "Glyphi"

# 1. The Electron app, through its own uninstaller.
$updateExe = Join-Path $squirrelRoot "Update.exe"
if (Test-Path $updateExe) {
    Write-Host "Uninstalling the Electron build..."
    # Squirrel removes shortcuts and the Apps & Features entry; it can refuse while
    # the app is running, which is why the process is stopped first.
    Get-Process Glyphi -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Get-Process glyphi-proxy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 800
    try { & $updateExe --uninstall --silent } catch { Write-Warning "The old uninstaller said: $_" }
    Start-Sleep -Milliseconds 800
}

# 2. Whatever the old installer did not own: build folders, package cache, logs.
Get-ChildItem $squirrelRoot -ErrorAction SilentlyContinue | Where-Object {
    $_.Name -like "app-*" -or $_.Name -eq "packages" -or $_.Name -like "Squirrel-*.log" -or $_.Name -eq "Update.exe"
} | ForEach-Object {
    Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
}

# 3. The Chromium profile. The database and preferences live in the same folder.
$caches = @(
    "Cache", "Code Cache", "GPUCache", "DawnGraphiteCache", "DawnWebGPUCache",
    "Local Storage", "Session Storage", "Network", "blob_storage", "shared_proto_db",
    "Local State", "DIPS", "DIPS-wal", "SingletonLock", "SingletonSocket", "SingletonCookie"
)
foreach ($name in $caches) {
    Remove-Item (Join-Path $userData $name) -Recurse -Force -ErrorAction SilentlyContinue
}

# 4. The new build.
Write-Host "Installing into $InstallDirectory..."
New-Item -ItemType Directory -Force -Path $InstallDirectory | Out-Null
# A running build holds its exe and its WebView libraries: stop it before copying.
Get-Process glyphi -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$InstallDirectory*" } |
    Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500
Get-ChildItem $source -Force | Where-Object { $_.Name -notin @("install.ps1", "uninstall.ps1") } | ForEach-Object {
    Copy-Item $_.FullName -Destination $InstallDirectory -Recurse -Force
}

$installedExe = Join-Path $InstallDirectory "glyphi.exe"

# 5. Shortcuts and the Apps & Features record, per user.
if (-not $NoShortcuts) {
    $shell = New-Object -ComObject WScript.Shell
    $startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
    New-Item -ItemType Directory -Force -Path $startMenu | Out-Null
    $link = $shell.CreateShortcut((Join-Path $startMenu "Glyphi.lnk"))
    $link.TargetPath = $installedExe
    $link.WorkingDirectory = $InstallDirectory
    $link.IconLocation = $installedExe
    $link.Save()
}

$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Glyphi"
New-Item -Path $uninstallKey -Force | Out-Null
Set-ItemProperty $uninstallKey -Name "DisplayName" -Value "Glyphi"
Set-ItemProperty $uninstallKey -Name "DisplayVersion" -Value "0.6.0"
Set-ItemProperty $uninstallKey -Name "DisplayIcon" -Value $installedExe
Set-ItemProperty $uninstallKey -Name "Publisher" -Value "Arthur Godart"
Set-ItemProperty $uninstallKey -Name "InstallLocation" -Value $InstallDirectory
$uninstaller = Join-Path $InstallDirectory "uninstall.ps1"
Set-ItemProperty $uninstallKey -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$uninstaller`""

Write-Host "Glyphi 0.6.0 is installed. The Electron build has been removed; your usage history was kept."
Write-Host "Launch it from the Start Menu, or run $installedExe"
