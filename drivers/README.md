HouseOS Drivers (stubs)
=======================

This folder contains small, self-contained driver scaffolds for a 32-bit x86
kernel. They are not wired into the kernel yet, but provide a clean starting
point.

Included
- port_io.rs: in/out helpers for I/O ports
- video/vesa_lfb.rs: linear framebuffer helper (24/32bpp)
- input/ps2_controller.rs: basic PS/2 controller I/O
- input/keyboard_ps2.rs: simple scancode reader
- input/mouse_ps2.rs: PS/2 mouse packet reader
- interrupts/pic8259.rs: PIC remap and masking
- timer/pit8253.rs: PIT init (timer)
- serial/uart16550.rs: COM1 init and write

Integration (suggested)
- Add a drivers module to the kernel and include these files.
- Initialize PIC and PIT early.
- Initialize PS/2 and poll or enable IRQs for keyboard and mouse.
- Use the VESA LFB helper to draw.
