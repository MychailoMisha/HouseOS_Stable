; HouseOS stage2 loader.
; Sets VESA 0x11A (1024x768x32) and reads the image via ATA PIO into the LFB.

[bits 16]
[org 0x8000]

%define VBE_MODE_INFO 0x9000
; 0x11A = 1024x768x32bpp (common VBE mode)
%define VBE_MODE 0x11A

%define IMG_WIDTH 1024
%define IMG_HEIGHT 768
%define IMG_BYTES (IMG_WIDTH*IMG_HEIGHT*4)
%define IMG_SECTORS (IMG_BYTES/512)
%define ROW_BYTES (IMG_WIDTH*4)
%define ROW_SECTORS (ROW_BYTES/512)
%define IMG_ADDR 0x200000

%define IMAGE_LBA 65

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    mov [boot_drive], dl

    ; Try VBE mode 0x11A (32bpp). If unavailable, fall back to 0x118 (24bpp).
    mov ax, 0x4f02
    mov bx, VBE_MODE
    or bx, 0x4000
    int 0x10
    cmp ax, 0x004f
    je .mode_ok
    mov ax, 0x4f02
    mov bx, 0x118
    or bx, 0x4000
    int 0x10
    cmp ax, 0x004f
    jne hang
.mode_ok:

    ; Query VBE mode info after setting mode
    xor ax, ax
    mov es, ax
    mov ax, 0x4f01
    mov cx, bx
    mov di, VBE_MODE_INFO
    int 0x10
    cmp ax, 0x004f
    jne hang
    xor ax, ax
    mov ds, ax
    mov al, byte [VBE_MODE_INFO + 0x19]
    mov [bpp], al
    mov eax, dword [VBE_MODE_INFO + 0x28]
    mov [lfb_base], eax
    mov ax, word [VBE_MODE_INFO + 0x10]
    mov [pitch], ax
    mov ax, word [VBE_MODE_INFO + 0x12]
    mov [xres], ax
    mov ax, word [VBE_MODE_INFO + 0x14]
    mov [yres], ax

    ; Enable A20
    in al, 0x92
    or al, 2
    out 0x92, al

    ; Load GDT and enter protected mode
    lgdt [gdt_descriptor]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp CODE_SEL:pm_entry

hang:
    hlt
    jmp hang

[bits 32]
pm_entry:
    mov ax, DATA_SEL
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000
    cld

    ; Read entire image into a scratch buffer, then blit to LFB with pitch.
    mov edi, IMG_ADDR
    mov ecx, IMG_SECTORS
    mov ebx, IMAGE_LBA
read_all:
    push ecx
    push ebx
    call ata_read_sector
    pop ebx
    pop ecx
    inc ebx
    dec ecx
    jnz read_all

    mov esi, IMG_ADDR
    mov edi, dword [lfb_base]
    movzx ebp, word [pitch]
    movzx ecx, word [xres]
    movzx edx, word [yres]
    mov eax, ecx
    cmp eax, IMG_WIDTH
    jbe .w_ok
    mov eax, IMG_WIDTH
.w_ok:
    mov [copy_w], ax
    mov eax, edx
    cmp eax, IMG_HEIGHT
    jbe .h_ok
    mov eax, IMG_HEIGHT
.h_ok:
    mov [copy_h], ax

    ; Choose blit path based on reported bpp
    mov al, [bpp]
    cmp al, 32
    je blit_32
    cmp al, 24
    je blit_24
    cmp al, 16
    je blit_16
    jmp hang

blit_32:
    ; If dest resolution matches source, do a fast row copy.
    movzx eax, word [xres]
    cmp eax, IMG_WIDTH
    jne blit_32_scale
    movzx eax, word [yres]
    cmp eax, IMG_HEIGHT
    jne blit_32_scale

blit_32_copy:
    movzx eax, word [copy_w]
    shl eax, 2             ; row_bytes_dst = width * 4
    mov [row_bytes_dst], ax
    movzx ebx, word [pitch]
    sub ebx, eax           ; pad = pitch - row_bytes_dst
    mov [row_pad], ebx
    mov ecx, ROW_BYTES
    sub ecx, eax           ; src_skip = src_row_bytes - dst_row_bytes
    mov [src_skip], cx
    movzx ebp, word [copy_h]
row32_copy:
    push ebp
    movzx ecx, word [row_bytes_dst]
    rep movsb
    movzx eax, word [src_skip]
    add esi, eax
    mov eax, [row_pad]
    add edi, eax
    pop ebp
    dec ebp
    jnz row32_copy
    jmp blit_done

blit_32_scale:
    ; Nearest-neighbor scale from 1024x768 BGRA to current xres/yres.
    movzx ecx, word [xres]
    movzx ebx, word [yres]
    mov eax, IMG_WIDTH
    shl eax, 16
    xor edx, edx
    div ecx
    mov [step_x], eax
    mov eax, IMG_HEIGHT
    shl eax, 16
    xor edx, edx
    div ebx
    mov [step_y], eax
    mov dword [src_y_fp], 0

scale_y:
    push ebx
    mov eax, [src_y_fp]
    mov edx, eax
    shr edx, 16            ; src_y
    mov eax, edx
    imul eax, ROW_BYTES
    mov esi, IMG_ADDR
    add esi, eax
    mov dword [src_x_fp], 0
    movzx ecx, word [xres]
scale_x:
    mov eax, [src_x_fp]
    mov edx, eax
    shr edx, 16            ; src_x
    mov eax, edx
    shl eax, 2
    mov eax, [esi + eax]
    mov [edi], eax
    add edi, 4
    mov eax, [step_x]
    add [src_x_fp], eax
    dec ecx
    jnz scale_x
    movzx eax, word [xres]
    shl eax, 2
    movzx edx, word [pitch]
    sub edx, eax
    add edi, edx
    mov eax, [step_y]
    add [src_y_fp], eax
    pop ebx
    dec ebx
    jnz scale_y
    jmp blit_done

blit_24:
    ; If dest resolution matches source, do a fast convert-copy.
    movzx eax, word [xres]
    cmp eax, IMG_WIDTH
    jne blit_24_scale
    movzx eax, word [yres]
    cmp eax, IMG_HEIGHT
    jne blit_24_scale

    ; Convert BGRA source to 24bpp (B,G,R) in LFB.
    movzx eax, word [copy_w]
    lea edx, [eax*2 + eax] ; row_bytes_dst = width * 3
    mov [row_bytes_dst], dx
    movzx ebx, word [pitch]
    sub ebx, edx           ; pad = pitch - row_bytes_dst
    mov [row_pad], ebx
    movzx eax, word [copy_w]
    shl eax, 2             ; src bytes for row = width * 4
    mov ecx, ROW_BYTES
    sub ecx, eax           ; src_skip = src_row_bytes - src_bytes_for_row
    mov [src_skip], cx
    movzx ebp, word [copy_h]
row24_copy:
    push ebp
    movzx ecx, word [copy_w]
pix24_copy:
    mov al, [esi]          ; B
    mov [edi], al
    mov al, [esi+1]        ; G
    mov [edi+1], al
    mov al, [esi+2]        ; R
    mov [edi+2], al
    add esi, 4
    add edi, 3
    dec ecx
    jnz pix24_copy
    movzx eax, word [src_skip]
    add esi, eax
    mov eax, [row_pad]
    add edi, eax
    pop ebp
    dec ebp
    jnz row24_copy
    jmp blit_done

blit_24_scale:
    ; Nearest-neighbor scale from 1024x768 BGRA to current xres/yres (24bpp dest).
    movzx ecx, word [xres]
    movzx ebx, word [yres]
    mov eax, IMG_WIDTH
    shl eax, 16
    xor edx, edx
    div ecx
    mov [step_x], eax
    mov eax, IMG_HEIGHT
    shl eax, 16
    xor edx, edx
    div ebx
    mov [step_y], eax
    mov dword [src_y_fp], 0

scale24_y:
    push ebx
    mov eax, [src_y_fp]
    mov edx, eax
    shr edx, 16            ; src_y
    mov eax, edx
    imul eax, ROW_BYTES
    mov esi, IMG_ADDR
    add esi, eax
    mov dword [src_x_fp], 0
    movzx ecx, word [xres]
scale24_x:
    mov eax, [src_x_fp]
    mov edx, eax
    shr edx, 16            ; src_x
    mov eax, edx
    shl eax, 2
    mov al, [esi + eax]    ; B
    mov [edi], al
    mov al, [esi + eax + 1]; G
    mov [edi + 1], al
    mov al, [esi + eax + 2]; R
    mov [edi + 2], al
    add edi, 3
    mov eax, [step_x]
    add [src_x_fp], eax
    dec ecx
    jnz scale24_x
    movzx eax, word [xres]
    lea eax, [eax*2 + eax] ; xres*3
    movzx edx, word [pitch]
    sub edx, eax
    add edi, edx
    mov eax, [step_y]
    add [src_y_fp], eax
    pop ebx
    dec ebx
    jnz scale24_y
    jmp blit_done

blit_16:
    ; If dest resolution matches source, do a fast convert-copy.
    movzx eax, word [xres]
    cmp eax, IMG_WIDTH
    jne blit_16_scale
    movzx eax, word [yres]
    cmp eax, IMG_HEIGHT
    jne blit_16_scale

    ; Convert BGRA to RGB565.
    movzx eax, word [copy_w]
    shl eax, 1             ; row_bytes_dst = width * 2
    mov [row_bytes_dst], ax
    movzx ebx, word [pitch]
    sub ebx, eax           ; pad = pitch - row_bytes_dst
    mov [row_pad], ebx
    movzx eax, word [copy_w]
    shl eax, 2             ; src bytes for row = width * 4
    mov ecx, ROW_BYTES
    sub ecx, eax           ; src_skip = src_row_bytes - src_bytes_for_row
    mov [src_skip], cx
    movzx ebp, word [copy_h]
row16_copy:
    push ebp
    movzx ecx, word [copy_w]
pix16_copy:
    movzx eax, byte [esi+2] ; R
    shr eax, 3
    shl eax, 11
    movzx edx, byte [esi+1] ; G
    shr edx, 2
    shl edx, 5
    or eax, edx
    movzx edx, byte [esi]   ; B
    shr edx, 3
    or eax, edx
    mov [edi], ax
    add esi, 4
    add edi, 2
    dec ecx
    jnz pix16_copy
    movzx eax, word [src_skip]
    add esi, eax
    mov eax, [row_pad]
    add edi, eax
    pop ebp
    dec ebp
    jnz row16_copy
    jmp blit_done

blit_16_scale:
    ; Nearest-neighbor scale from 1024x768 BGRA to current xres/yres (16bpp dest).
    movzx ecx, word [xres]
    movzx ebx, word [yres]
    mov eax, IMG_WIDTH
    shl eax, 16
    xor edx, edx
    div ecx
    mov [step_x], eax
    mov eax, IMG_HEIGHT
    shl eax, 16
    xor edx, edx
    div ebx
    mov [step_y], eax
    mov dword [src_y_fp], 0

scale16_y:
    push ebx
    mov eax, [src_y_fp]
    mov edx, eax
    shr edx, 16            ; src_y
    mov eax, edx
    imul eax, ROW_BYTES
    mov esi, IMG_ADDR
    add esi, eax
    mov dword [src_x_fp], 0
    movzx ecx, word [xres]
scale16_x:
    mov eax, [src_x_fp]
    mov edx, eax
    shr edx, 16            ; src_x
    mov eax, edx
    shl eax, 2
    movzx edx, byte [esi + eax + 2] ; R
    shr edx, 3
    shl edx, 11
    movzx ebp, byte [esi + eax + 1] ; G
    shr ebp, 2
    shl ebp, 5
    or edx, ebp
    movzx ebp, byte [esi + eax]     ; B
    shr ebp, 3
    or edx, ebp
    mov [edi], dx
    add edi, 2
    mov eax, [step_x]
    add [src_x_fp], eax
    dec ecx
    jnz scale16_x
    movzx eax, word [xres]
    shl eax, 1             ; xres*2
    movzx edx, word [pitch]
    sub edx, eax
    add edi, edx
    mov eax, [step_y]
    add [src_y_fp], eax
    pop ebx
    dec ebx
    jnz scale16_y
    jmp blit_done

blit_done:

halt:
    hlt
    jmp halt

; Read one sector from LBA in EBX into [EDI], advance EDI by 512.
ata_read_sector:
    call ata_wait

    mov dx, 0x1F2
    mov al, 1
    out dx, al

    mov eax, ebx
    mov dx, 0x1F3
    out dx, al          ; LBA 0-7
    inc dx              ; 1F4
    mov al, ah
    out dx, al          ; LBA 8-15
    inc dx              ; 1F5
    shr eax, 16
    out dx, al          ; LBA 16-23
    inc dx              ; 1F6
    mov al, 0xE0
    or al, ah           ; LBA 24-27
    out dx, al

    mov dx, 0x1F7
    mov al, 0x20
    out dx, al

    call ata_wait_drq

    mov dx, 0x1F0
    mov ecx, 256
    rep insw
    ret

ata_wait:
    mov dx, 0x1F7
    in al, dx
    test al, 0x80
    jnz ata_wait
    ret

ata_wait_drq:
    mov dx, 0x1F7
    in al, dx
    test al, 0x80
    jnz ata_wait_drq
    test al, 0x08
    jz ata_wait_drq
    ret

; Read one image row (IMG_WIDTH*3 bytes) from disk into [EDI], using LBA in ESI.
; (row reader removed)

; GDT (flat 4GB)
gdt_start:
    dq 0x0000000000000000
gdt_code:
    dq 0x00CF9A000000FFFF
gdt_data:
    dq 0x00CF92000000FFFF
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

CODE_SEL equ gdt_code - gdt_start
DATA_SEL equ gdt_data - gdt_start

boot_drive: db 0
lfb_base: dd 0
pitch: dw 0
xres: dw 0
yres: dw 0
bpp: db 0
bpp_bytes: db 0
copy_w: dw 0
copy_h: dw 0
row_bytes_dst: dw 0
src_skip: dw 0
row_pad: dd 0
step_x: dd 0
step_y: dd 0
src_x_fp: dd 0
src_y_fp: dd 0
