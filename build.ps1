$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust and add cargo to PATH."
}

$objcopy = $null
if (Get-Command llvm-objcopy -ErrorAction SilentlyContinue) {
    $objcopy = "llvm-objcopy"
} elseif (Get-Command rust-objcopy -ErrorAction SilentlyContinue) {
    $objcopy = "rust-objcopy"
}

if (-not $objcopy) {
    throw "llvm-objcopy or rust-objcopy not found. Install cargo-binutils or LLVM tools."
}

Push-Location $root
try {
    cargo +nightly build -Z build-std=core -Z json-target-spec --release

    $elf = Join-Path $root "target\i686-houseos\release\houseos-kernel"
    $bin = Join-Path $root "target\i686-houseos\release\houseos-kernel.bin"

    if (-not (Test-Path $elf)) {
        throw "Kernel ELF not found at $elf"
    }

    & $objcopy -O binary $elf $bin
}
finally {
    Pop-Location
}
