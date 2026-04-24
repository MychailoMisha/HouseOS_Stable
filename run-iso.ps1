param(
    [string]$QemuBiosPath = "C:\Program Files\qemu\qemu-system-i386.exe",
    [string]$QemuUefiPath = "C:\Program Files\qemu\qemu-system-x86_64.exe",
    [ValidateSet("auto", "bios", "uefi")]
    [string]$Firmware = "auto",
    [string]$OvmfCodePath = "",
    [string]$OvmfVarsPath = "",
    [string]$GrubMkrescuePath = "grub-mkrescue",
    [string]$XorrisoPath = "",
    [string]$TarPath = "tar",
    [string]$IsoPath = "",
    [string]$ImagePath = ""
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$makeIso = Join-Path $root "make-iso.ps1"
$limineDir = Join-Path $root "boot\limine"

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

if (-not (Test-Path $IsoPath)) {
    throw "ISO not found at $IsoPath"
}

function Find-ExistingPath([string[]]$candidates) {
    foreach ($c in $candidates) {
        if (-not [string]::IsNullOrWhiteSpace($c) -and (Test-Path $c)) {
            return $c
        }
    }
    return $null
}

function Has-UefiPayload {
    param([string]$LimineDirPath)
    $uefiCd = Join-Path $LimineDirPath "limine-uefi-cd.bin"
    $bootX64 = Join-Path $LimineDirPath "BOOTX64.EFI"
    $bootIa32 = Join-Path $LimineDirPath "BOOTIA32.EFI"
    return (Test-Path $uefiCd) -and ((Test-Path $bootX64) -or (Test-Path $bootIa32))
}

$hasUefiPayload = Has-UefiPayload -LimineDirPath $limineDir

$ovmfCodeCandidate = Find-ExistingPath @(
    $OvmfCodePath,
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd",
    "C:\Program Files\qemu\share\OVMF\OVMF_CODE.fd",
    "C:\Program Files\qemu\share\OVMF.fd",
    "C:\msys64\usr\share\edk2-ovmf\x64\OVMF_CODE.fd",
    "C:\msys64\usr\share\edk2-ovmf\OVMF_CODE.fd"
)
$ovmfVarsCandidate = Find-ExistingPath @(
    $OvmfVarsPath,
    "C:\Program Files\qemu\share\edk2-x86_64-vars.fd",
    "C:\Program Files\qemu\share\OVMF\OVMF_VARS.fd",
    "C:\msys64\usr\share\edk2-ovmf\x64\OVMF_VARS.fd",
    "C:\msys64\usr\share\edk2-ovmf\OVMF_VARS.fd"
)

$hasQemuUefi = Test-Path $QemuUefiPath
$canUefi = $hasUefiPayload -and ($null -ne $ovmfCodeCandidate) -and $hasQemuUefi

switch ($Firmware) {
    "uefi" {
        if (-not $canUefi) {
            throw "UEFI requested but missing files. Need Limine UEFI payload in boot\\limine and OVMF firmware (OVMF_CODE.fd)."
        }
    }
    "bios" { }
    "auto" {
        if ($canUefi) {
            $Firmware = "uefi"
        } else {
            $Firmware = "bios"
        }
    }
}

$netArgs = @("-netdev", "user,id=net0", "-device", "rtl8139,netdev=net0")

if ($Firmware -eq "uefi") {
    if (-not (Test-Path $QemuUefiPath)) {
        throw "QEMU UEFI binary not found at $QemuUefiPath. Install QEMU x86_64 or pass -QemuUefiPath."
    }

    if ($ovmfVarsCandidate) {
        $varsRuntime = Join-Path $root "build\OVMF_VARS.fd"
        Copy-Item -Force $ovmfVarsCandidate $varsRuntime
        & $QemuUefiPath `
            "-m" "512M" `
            "-display" "gtk,zoom-to-fit=on,full-screen=on" `
            "-drive" "if=pflash,format=raw,readonly=on,file=$ovmfCodeCandidate" `
            "-drive" "if=pflash,format=raw,file=$varsRuntime" `
            "-cdrom" $IsoPath `
            @netArgs
    } else {
        & $QemuUefiPath `
            "-m" "512M" `
            "-display" "gtk,zoom-to-fit=on,full-screen=on" `
            "-bios" $ovmfCodeCandidate `
            "-cdrom" $IsoPath `
            @netArgs
    }
} else {
    if (-not (Test-Path $QemuBiosPath)) {
        throw "QEMU BIOS binary not found at $QemuBiosPath. Install QEMU i386 or pass -QemuBiosPath."
    }
    & $QemuBiosPath `
        "-cdrom" $IsoPath `
        "-m" "384M" `
        "-display" "gtk,zoom-to-fit=on,full-screen=on" `
        "-vga" "std" `
        @netArgs
}
