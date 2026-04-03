param(
    [string]$NasmPath = "C:\Program Files\NASM\nasm.exe",
    [string]$QemuPath = "C:\Program Files\qemu\qemu-system-i386.exe",
    [string]$ImagePath = "",
    [string]$CursorPath = "",
    [int]$CursorSize = 32,
    [int]$CursorX = -1,
    [int]$CursorY = -1,
    [ValidateSet("iso", "raw")]
    [string]$Boot = "iso"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$runIso = Join-Path $root "run-iso.ps1"
$bootAsm = Join-Path $root "boot\boot.asm"
$stage2Asm = Join-Path $root "boot\stage2.asm"
$buildDir = Join-Path $root "build"
$bootBin = Join-Path $buildDir "boot.bin"
$stage2Bin = Join-Path $buildDir "stage2.bin"
$diskImg = Join-Path $buildDir "houseos.img"
$imageWidth = 1024
$imageHeight = 768
$stage2Sectors = 64
$stage2Bytes = $stage2Sectors * 512
if ([string]::IsNullOrWhiteSpace($ImagePath)) {
    $ImagePath = Join-Path $root "assets\background.png"
}
$cursorPathResolved = $CursorPath
if ([string]::IsNullOrWhiteSpace($cursorPathResolved)) {
    $cursorPathResolved = Join-Path $root "assets\cursor.png"
}
$imageRaw = Join-Path $buildDir "background.raw"

if ($Boot -eq "iso") {
    if (-not (Test-Path $runIso)) {
        throw "run-iso.ps1 not found at $runIso"
    }
    & $runIso -QemuPath $QemuPath -ImagePath $ImagePath
    return
}

if (-not (Test-Path $bootAsm)) {
    throw "Missing bootloader source: $bootAsm"
}
if (-not (Test-Path $stage2Asm)) {
    throw "Missing stage2 source: $stage2Asm"
}

New-Item -ItemType Directory -Force -Path $buildDir | Out-Null

if (-not (Test-Path $NasmPath)) {
    throw "NASM not found at $NasmPath. Install NASM or pass -NasmPath with the full path to nasm.exe."
}
if (-not (Test-Path $QemuPath)) {
    throw "QEMU not found at $QemuPath. Install QEMU or pass -QemuPath with the full path to qemu-system-i386.exe."
}
if (-not (Test-Path $ImagePath)) {
    throw "Missing image file: $ImagePath. Put a PNG/JPG/BMP there or pass -ImagePath with the full path."
}

& $NasmPath "-f" "bin" $bootAsm "-o" $bootBin
& $NasmPath "-f" "bin" $stage2Asm "-o" $stage2Bin

if (-not (Test-Path $bootBin)) {
    throw "NASM did not produce $bootBin"
}
if (-not (Test-Path $stage2Bin)) {
    throw "NASM did not produce $stage2Bin"
}

$bootBytes = Get-Content -Encoding Byte -Path $bootBin
if ($bootBytes.Length -ne 512) {
    throw "boot.bin must be exactly 512 bytes, got $($bootBytes.Length)"
}
$stage2BytesRaw = [System.IO.File]::ReadAllBytes($stage2Bin)
if ($stage2BytesRaw.Length -gt $stage2Bytes) {
    throw "stage2.bin is too large ($($stage2BytesRaw.Length) bytes). Max is $stage2Bytes bytes."
}
$stage2Padded = New-Object byte[] $stage2Bytes
[System.Buffer]::BlockCopy($stage2BytesRaw, 0, $stage2Padded, 0, $stage2BytesRaw.Length)

Add-Type -AssemblyName System.Drawing

$src = [System.Drawing.Bitmap]::new($ImagePath)
$bmp = [System.Drawing.Bitmap]::new($imageWidth, $imageHeight, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$gfx.DrawImage($src, 0, 0, $imageWidth, $imageHeight)

if (Test-Path $cursorPathResolved) {
    $cursorBmp = [System.Drawing.Bitmap]::new($cursorPathResolved)
    $cursorW = $CursorSize
    $cursorH = $CursorSize
    if ($CursorX -lt 0) {
        $CursorX = [int](($imageWidth - $cursorW) / 2)
    }
    if ($CursorY -lt 0) {
        $CursorY = [int](($imageHeight - $cursorH) / 2)
    }
    $gfx.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
    $gfx.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::None
    $gfx.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
    $gfx.DrawImage($cursorBmp, $CursorX, $CursorY, $cursorW, $cursorH)
    $cursorBmp.Dispose()
}

$gfx.Dispose()
$src.Dispose()

$rect = New-Object System.Drawing.Rectangle(0, 0, $imageWidth, $imageHeight)
$bd = $bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
try {
    $stride = $bd.Stride
    $absStride = [Math]::Abs($stride)
    $rowBytes = $imageWidth * 4
    $raw = New-Object byte[] ($rowBytes * $imageHeight)
    for ($y = 0; $y -lt $imageHeight; $y++) {
        if ($stride -lt 0) {
            $srcRow = $imageHeight - 1 - $y
        } else {
            $srcRow = $y
        }
        $offset = $srcRow * $absStride
        $ptr = [IntPtr]::Add($bd.Scan0, $offset)
        [System.Runtime.InteropServices.Marshal]::Copy($ptr, $raw, $y * $rowBytes, $rowBytes)
    }
}
finally {
    $bmp.UnlockBits($bd)
    $bmp.Dispose()
}

[System.IO.File]::WriteAllBytes($imageRaw, $raw)

$imgBytes = Get-Content -Encoding Byte -Path $imageRaw
$needed = 512 + $stage2Bytes + $imgBytes.Length
$imgSize = [int]([math]::Ceiling($needed / 512.0) * 512)

$fs = [System.IO.File]::Open($diskImg, [System.IO.FileMode]::Create, [System.IO.FileAccess]::ReadWrite)
$fs.SetLength($imgSize)
$fs.Write($bootBytes, 0, $bootBytes.Length)
$fs.Position = 512
$fs.Write($stage2Padded, 0, $stage2Padded.Length)
$fs.Position = 512 + $stage2Bytes
$fs.Write($imgBytes, 0, $imgBytes.Length)
$fs.Close()

& $QemuPath "-drive" "format=raw,file=$diskImg" "-m" "128M" "-display" "gtk,zoom-to-fit=on,full-screen=on" "-vga" "std"
