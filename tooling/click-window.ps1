# Clicks a point inside the Glyphi window, given as a fraction of its client area.
#   powershell -File tooling/click-window.ps1 -X 48 -Y 81      (absolute, window-relative)
param(
  [string]$ProcessName = "Glyphi",
  [int]$X = 48,
  [int]$Y = 81
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Clicker {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $process) { Write-Error "No $ProcessName window is open."; exit 1 }

$rect = New-Object Clicker+RECT
[void][Clicker]::GetWindowRect($process.MainWindowHandle, [ref]$rect)
[void][Clicker]::SetForegroundWindow($process.MainWindowHandle)
Start-Sleep -Milliseconds 300
[void][Clicker]::SetCursorPos($rect.Left + $X, $rect.Top + $Y)
Start-Sleep -Milliseconds 150
[Clicker]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)   # left down
Start-Sleep -Milliseconds 60
[Clicker]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)   # left up
Start-Sleep -Milliseconds 500
Write-Output "Clicked $($rect.Left + $X),$($rect.Top + $Y)"
