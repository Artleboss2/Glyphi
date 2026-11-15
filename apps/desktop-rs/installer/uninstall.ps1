<#
.SYNOPSIS
    Removes Glyphi.

.DESCRIPTION
    Deletes the installed build and its shortcuts. The usage database and the
    preferences in %APPDATA%\Glyphi are kept unless -RemoveData is given: they hold
    the history the user spent months accumulating.
#>
[CmdletBinding()]
param(
    [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA "Programs\Glyphi"),
    [switch]$RemoveData
)

$ErrorActionPreference = "Stop"

Get-Process glyphi -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$InstallDirectory*" } |
    Stop-Process -Force -ErrorAction SilentlyContinue
Get-Process glyphi-proxy -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 600

foreach ($shortcut in @(
    (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Glyphi.lnk"),
    (Join-Path ([Environment]::GetFolderPath("Desktop")) "Glyphi.lnk")
)) {
    Remove-Item $shortcut -Force -ErrorAction SilentlyContinue
}

Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Glyphi" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $InstallDirectory -Recurse -Force -ErrorAction SilentlyContinue

if ($RemoveData) {
    Remove-Item (Join-Path $env:APPDATA "Glyphi") -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Glyphi and its local history were removed."
} else {
    Write-Host "Glyphi was removed. Your usage history is still in $env:APPDATA\Glyphi."
}
