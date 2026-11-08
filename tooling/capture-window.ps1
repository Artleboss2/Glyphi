# Captures the main window of a running Glyphi process to a PNG.
#   powershell -File tooling/capture-window.ps1 -Output docs/screenshots/dashboard.png
param(
  [string]$ProcessName = "Glyphi",
  [string]$Output = "docs/screenshots/dashboard.png"
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } |
  Select-Object -First 1
if (-not $process) { Write-Error "No $ProcessName window is open."; exit 1 }

[void][Win]::ShowWindow($process.MainWindowHandle, 9)   # SW_RESTORE
[void][Win]::SetForegroundWindow($process.MainWindowHandle)
Start-Sleep -Milliseconds 700

$rect = New-Object Win+RECT
[void][Win]::GetWindowRect($process.MainWindowHandle, [ref]$rect)
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) { Write-Error "The window has no size."; exit 1 }

$directory = Split-Path -Parent $Output
if ($directory -and -not (Test-Path $directory)) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
$bitmap.Save((Resolve-Path -LiteralPath (Split-Path -Parent $Output)).Path + "\" + (Split-Path -Leaf $Output), [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
Write-Output "Saved $Output ($width x $height)"
