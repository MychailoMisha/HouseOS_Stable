param(
    [string]$QemuPath = "C:\Program Files\qemu\qemu-system-i386.exe",
    [string]$GrubMkrescuePath = "grub-mkrescue",
    [string]$XorrisoPath = "",
    [string]$TarPath = "tar",
    [string]$IsoPath = "",
    [string]$ImagePath = ""
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$makeIso = Join-Path $root "make-iso.ps1"

if ([string]::IsNullOrWhiteSpace($IsoPath)) {
    $IsoPath = Join-Path $root "build\houseos.iso"
}

if (-not (Test-Path $makeIso)) {
    throw "make-iso.ps1 not found at $makeIso"
}

if (Test-Path $IsoPath) {
    Remove-Item -Force $IsoPath
}

& $makeIso -GrubMkrescuePath $GrubMkrescuePath -XorrisoPath $XorrisoPath -TarPath $TarPath -IsoPath $IsoPath -ImagePath $ImagePath

if (-not (Test-Path $QemuPath)) {
    throw "QEMU not found at $QemuPath. Install QEMU or pass -QemuPath with the full path."
}
if (-not (Test-Path $IsoPath)) {
    throw "ISO not found at $IsoPath"
}

& $QemuPath "-cdrom" $IsoPath "-m" "256M" "-display" "gtk,zoom-to-fit=on,full-screen=on" "-vga" "std"
