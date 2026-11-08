# Points the mouse at the middle of the Glyphi window and turns the wheel.
#   powershell -File tooling/scroll-window.ps1 -Clicks 40   (positive = up)
param(
  [string]$ProcessName = "Glyphi",
  [int]$Clicks = 30,
  [int]$XOffsetRatio = 2,
  [int]$YOffsetRatio = 3
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Mouse {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, UIntPtr extra);
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $process) { Write-Error "No $ProcessName window is open."; exit 1 }

$rect = New-Object Mouse+RECT
[void][Mouse]::GetWindowRect($process.MainWindowHandle, [ref]$rect)
[void][Mouse]::SetForegroundWindow($process.MainWindowHandle)
$targetX = $rect.Left + [int](($rect.Right - $rect.Left) * $XOffsetRatio / 5)
$targetY = $rect.Top + [int](($rect.Bottom - $rect.Top) * $YOffsetRatio / 6)
[void][Mouse]::SetCursorPos($targetX, $targetY)
Start-Sleep -Milliseconds 200

$wheel = 0x0800
$distance = 120 * [Math]::Sign($Clicks)
for ($i = 0; $i -lt [Math]::Abs($Clicks); $i++) {
  [Mouse]::mouse_event($wheel, 0, 0, $distance, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 35
}
Start-Sleep -Milliseconds 400
Write-Output "Wheeled $Clicks clicks at $targetX,$targetY"
