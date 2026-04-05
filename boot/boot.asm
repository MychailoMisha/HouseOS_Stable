; HouseOS stage1 boot sector (512 bytes).
; Loads stage2 (fixed 64 sectors) to 0x8000 and jumps to it.

[bits 16]
[org 0x7c00]

%define STAGE2_SEG 0x0800
%define STAGE2_OFF 0x0000
%define STAGE2_SECTORS 64

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    mov [boot_drive], dl

    ; Read stage2 (LBA 1..STAGE2_SECTORS) into 0x8000
    mov byte [dap], 0x10
    mov byte [dap+1], 0
    mov word [dap+2], STAGE2_SECTORS
    mov word [dap+4], STAGE2_OFF
    mov word [dap+6], STAGE2_SEG
    mov dword [dap+8], 1
    mov dword [dap+12], 0

    mov si, dap
    mov ah, 0x42
    mov dl, [boot_drive]
    int 0x13
    jc hang

    jmp 0x0000:0x8000

hang:
    hlt
    jmp hang

boot_drive: db 0

dap:
    db 0x10
    db 0x00
    dw 0
    dw 0
    dw 0
    dd 0
    dd 0

times 510 - ($ - $$) db 0
dw 0xAA55
