# Instruction Sets Required to Boot Linux

Choosing the right RISCV extension in mandatory.
**Linux on RISC-V almost exclusively targets RV64I** (64-bit). All major distributions
(Debian, Fedora, Ubuntu, Arch) ship RV64 binaries.

## Mandatory — Minimum Viable Linux

| Extension | Full Name | Why It Is Needed |
|-----------|-----------|-----------------|
| **RV64I** | Base Integer ISA (64-bit) | Core arithmetic, loads/stores, branches, jumps. The foundation everything else builds on. |
| **M** | Integer Multiply & Divide | `MUL`, `DIV`, `REM` and their variants. Used pervasively by the kernel and all compiled C code. |
| **A** | Atomic Instructions | `LR`/`SC` (load-reserved / store-conditional) and AMO instructions (`AMOSWAP`, `AMOADD`, …). Required for spinlocks, mutexes, and every other synchronization primitive in the kernel. |
| **Zicsr** | Control & Status Register Instructions | `CSRRW`, `CSRRS`, `CSRRC` and their immediate forms. Required for reading/writing CSRs: interrupt enable, privilege level, timer, cause registers, etc. |
| **Zifencei** | Instruction-Fetch Fence | `FENCE.I`. Required for instruction cache coherence — the kernel uses it when patching code and loading modules. |

Together these are written as: `rv64ima_zicsr_zifencei`

Linux also requires the following architectural features that are not expressed as letter
extensions but are equally mandatory:

| Feature | Description |
|---------|-------------|
| **S-mode** (Supervisor Mode) | The Linux kernel runs in Supervisor mode. The emulator must implement all three privilege levels: M-mode (firmware), S-mode (kernel), and U-mode (userspace). |
| **Sv39 Virtual Memory** | Linux uses a 3-level page table covering a 39-bit virtual address space. The MMU must translate virtual → physical addresses and raise page faults. Sv48 (4-level, 48-bit VA) is also supported by Linux but Sv39 is the minimum default. |
| **SBI** (Supervisor Binary Interface) | The interface between M-mode firmware and the S-mode kernel. Linux uses SBI calls for console output, timer setup, inter-processor interrupts (IPIs), and system reset. Typically implemented by a small M-mode shim inside the emulator. |
| **CLINT / PLIC** (or APLIC) | Core Local Interruptor (timer + software interrupts) and Platform-Level Interrupt Controller (external interrupts). Linux requires at least a timer interrupt to run its scheduler. |

---

## Practical Linux Userspace

Most compiled Linux software and the kernel itself is built for **RV64GC**, which adds:

| Extension | Full Name | Why It Matters |
|-----------|-----------|---------------|
| **F** | Single-Precision Floating-Point | Hardware float. Many programs link against `libm`. Without F, the kernel must emulate floats in software (`CONFIG_FPU=n`), and userspace programs compiled with hardware floats will not run. |
| **D** | Double-Precision Floating-Point | Requires F. `double` in C. The standard Linux ABI (`lp64d`) passes float arguments in FP registers; mismatching the ABI breaks interoperability with pre-built binaries. |
| **C** | Compressed Instructions | 16-bit encodings for common 32-bit instructions. Reduces kernel and userspace binary sizes by ~25–30%. Virtually all Linux distributions enable this. Any pre-built binary may contain compressed instructions. |

**RV64G** is shorthand for `RV64IMAFDZicsrZifencei`.  
**RV64GC** adds the C extension and is the de-facto standard Linux target.

---

## Optional — Performance & Features

These are not required to boot Linux but appear in real hardware and distributions:

| Extension | Full Name | Purpose |
|-----------|-----------|---------|
| **V** | Vector | SIMD operations. Used by optimized `memcpy`, crypto, and media codecs. Increasingly common on new hardware. |
| **Zbb** | Bit Manipulation (Base) | Efficient `CLZ`, `CTZ`, `CPOP`, byte-reverse, sign-extend. The kernel uses these for bitmaps and string operations when detected at boot. |
| **Zba** | Address-Generation Bit Manipulation | `SH1ADD`, `SH2ADD`, `SH3ADD` — efficient scaled-index address generation. |
| **Zbc** | Carry-Less Multiplication | `CLMUL`. Used for CRC computations (e.g., filesystem checksums). |
| **Zicbom/Zicboz** | Cache Block Management / Zeroing | Cache maintenance instructions. Required for non-coherent DMA on real hardware; not critical for emulation. |
| **Zawrs** | Wait-on-Reservation-Set | Allows a hart to sleep in a polling loop until a memory word changes. Useful for efficient spinlock implementations. |

---

## Privilege Levels In Depth

RISC-V defines three privilege levels. To boot Linux, all three must be implemented:

```
┌─────────────────────────────────────┐
│  U-mode  │  User applications        │  Least privileged
├─────────────────────────────────────┤
│  S-mode  │  Linux kernel             │
├─────────────────────────────────────┤
│  M-mode  │  Firmware / SBI shim      │  Most privileged
└─────────────────────────────────────┘
```

- **M-mode** runs first on reset. It sets up the SBI, configures PMP (Physical Memory
  Protection), and then jumps into S-mode to hand off to the kernel.
- **S-mode** is where the Linux kernel lives. It manages virtual memory (Sv39), handles
  syscalls from U-mode via `ecall`, and dispatches interrupts.
- **U-mode** is where userspace processes run. Any privileged instruction or page fault
  traps into S-mode.

## Key CSRs to Implement

| Register | Mode | Purpose |
|----------|------|---------|
| `mstatus` / `sstatus` | M / S | Global interrupt enable, privilege state |
| `mtvec` / `stvec` | M / S | Trap handler base address |
| `mepc` / `sepc` | M / S | Exception program counter (return address after trap) |
| `mcause` / `scause` | M / S | Cause of the last trap |
| `mtval` / `stval` | M / S | Trap value (e.g. faulting address) |
| `mie` / `sie` | M / S | Interrupt enable bits |
| `mip` / `sip` | M / S | Interrupt pending bits |
| `satp` | S | Page-table base address + address translation mode (Sv39) |
| `medeleg` / `mideleg` | M | Delegate exceptions/interrupts to S-mode |
| `mscratch` / `sscratch` | M / S | Scratch register for trap handlers |
| `mhartid` | M | Hardware thread ID (core number) |
| `time` / `timeh` | — | Wall-clock timer (read via `rdtime`) |

---

# Virtual Memory (Sv39)

Sv39 uses a 3-level page table with 4 KiB pages and 39-bit virtual addresses:

```
Virtual Address (39 bits):
 38      30 29      21 20      12 11           0
┌──────────┬──────────┬──────────┬──────────────┐
│  VPN[2]  │  VPN[1]  │  VPN[0]  │ Page Offset  │
│  9 bits  │  9 bits  │  9 bits  │   12 bits    │
└──────────┴──────────┴──────────┴──────────────┘
```

The `satp` CSR holds the physical page number of the root page table and the mode field
(set to `8` for Sv39).

Page table entries carry permission bits: `V` (valid), `R` (read), `W` (write), `X`
(execute), `U` (user-accessible), `A` (accessed), `D` (dirty), plus `RSW` bits reserved
for the OS.

---

# SBI (Supervisor Binary Interface)

The kernel communicates with M-mode firmware using `ecall` from S-mode. The emulator must
handle at least these SBI extension calls:

| Extension ID | Name | Required For |
|---|---|---|
| `0x01` | Legacy Console Putchar | Early boot console output |
| `0x54494D45` (`TIME`) | Timer | `clock_nanosleep`, scheduler ticks |
| `0x735049` (`sPI`) | IPI | Cross-core scheduling (SMP) |
| `0x52464E43` (`RFNC`) | Remote Fence | TLB shootdowns |
| `0x48534D` (`HSM`) | Hart State Management | CPU hotplug, SMP bring-up |
| `0x53525354` (`SRST`) | System Reset | `reboot` / `poweroff` |

For a single-core boot, `TIME` and `RFNC` are the most critical to implement first.

---

# Recommended Implementation Order

1. **Upgrade to 64-bit** — change registers and PC from `u32` to `u64`.
2. **Instruction decoder** — parse the 32-bit (and 16-bit compressed) instruction encoding.
3. **RV64I** — implement all base integer instructions.
4. **M extension** — multiply and divide instructions.
5. **CSRs + trap handling** — `ecall`, `ebreak`, exceptions, interrupt dispatch.
6. **M-mode SBI shim** — minimal firmware layer for timer and console.
7. **S-mode + `satp`** — privilege level switching, `medeleg`/`mideleg`.
8. **Sv39 MMU** — page-table walker, TLB, page-fault exceptions.
9. **A extension** — atomics (`LR`/`SC`, AMOs).
10. **F + D extensions** — floating-point register file and instructions.
11. **C extension** — compressed instruction decoding.
12. **Timer + PLIC** — hardware timer interrupt, external interrupt controller.
13. **Virtio devices** — block device and network for a useful Linux environment.

---

# References

- [RISC-V Unprivileged ISA Specification](https://github.com/riscv/riscv-isa-manual/releases/latest)
- [RISC-V Privileged ISA Specification](https://github.com/riscv/riscv-isa-manual/releases/latest)
- [SBI Specification](https://github.com/riscv-non-isa/riscv-sbi-doc/releases/latest)
- [Linux RISC-V Kconfig](https://github.com/torvalds/linux/blob/master/arch/riscv/Kconfig)
- [RISC-V Linux Boot Requirements](https://www.kernel.org/doc/html/latest/riscv/boot.html)
