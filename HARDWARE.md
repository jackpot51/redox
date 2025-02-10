# Hardware Compatibility

This document tracks the current hardware compatibility of Redox.

- [Why hardware reports are needed?](#why-hardware-reports-are-needed)
- [What if my computer is customized?](#what-if-my-computer-is-customized)
- [Status](#status)
- [General](#general)
- [Template](#template)
- [Recommended](#recommended)
- [Booting](#booting)
- [Broken](#broken)

## Why hardware reports are needed?

Each computer model have different hardware interface/firmware implementations and devices, which can cause the following problems:

- Boot bugs
- Lack of device support
- Performance degradation

These reports helps us to fix the problems above.

## What if my computer is customized?

You can use the "Custom" word on the "Vendor" and "Model" categories, we also recommend to add your `pciutils` log on [this document link](https://gitlab.redox-os.org/redox-os/drivers/-/blob/master/COMMUNITY-HW.md?ref_type=heads) to help us with probable porting.

## Status

- Broken - The system can't boot.
- Booting - The system boots with some issues.
- Recommended - The system start with all features working.

## General

This section cover things to consider.

- ACPI support is incomplete (some things are hardcoded on the kernel)
- Only USB input devices are supported
- Wi-Fi is not supported
- GPU drivers aren't supported (only VESA and UEFI GOP)
- Automatic operating system discovery on boot loader is not implemented (remember this before installing Redox)

## Template

You will use this template to insert your computer on the table.

```
|  |  |  |  |  |  |  |  |
```

## Recommended

| **Vendor** | **Model** | **Redox Version** | **Image Date** | **Variant** | **CPU Architecture** | **Motherboard Firmware** | **Report** |
|------------|-----------|-------------------|----------------|-------------|----------------------|--------------------------|------------|
| System76 | Galago Pro (galp5) | 0.8.0 | 11-11-2022 | desktop | x86-64 | UEFI | Boots to Orbital |
| System76 | Lemur Pro (lemp9) | 0.8.0 | 11-11-2022 | desktop | x86-64 | UEFI | Boots to Orbital |
| Lenovo | IdeaPad Y510P | 0.8.0 | 11-11-2022 | desktop | x86-64 | BIOS, UEFI | Boots to Orbital |

## Booting

| **Vendor** | **Model** | **Redox Version** | **Image Date** | **Variant** | **CPU Architecture** | **Motherboard Firmware** | **Report** |
|------------|-----------|-------------------|----------------|-------------|----------------------|--------------------------|------------|
| System76 | Oryx Pro (oryp10) | 0.8.0 | 11-11-2022 | desktop | x86-64 | UEFI | Boots to Orbital, No touchpad support, though it should be working |
| System76 | Pangolin (pang12) | 0.8.0 | 11-11-2022 | desktop | x86-64 | UEFI | Boots to Orbital, No touchpad support, requires I2C HID |
| Dell | XPS 13 (9350) | 0.8.0 | 11-11-2022 | desktop | x86-64 | BIOS, UEFI | Boots to Orbital, NVMe driver livelocks |
| Dell | XPS 13 (9350) | 0.8.0 | 11-11-2022 | desktop | i686 | BIOS | Boots to Orbital, NVMe driver livelocks |
| HP | Dev One | 0.8.0 | 11-11-2022 | desktop | x86-64 | UEFI | Boots to Orbital, No touchpad support, requires I2C HID |
| ASUS | X554L | 0.8.0 | 11-11-2022 | desktop | x86-64 | BIOS | Boots to Orbital, No audio, HDA driver cannot find output pins |
| ASUS | ROG g55vw | 0.8.0 | 11-11-2023 | desktop | x86-64 | BIOS | Boots to Orbital, UEFI panic in SETUP |
| ASUS | PRIME B350M-E (custom) | 0.9.0 | 20-09-2024 | desktop | x86-64 | UEFI | Partial support for the PS/2 keyboard, PS/2 mouse is broken |
| ASUS | Eee PC 900 | 0.8.0 | 11-11-2022 | desktop | i686 | BIOS | Boots to Orbital, No ethernet driver, Correct video mode not offered (firmware issue) |
| Toshiba | Satellite L500 | 0.8.0 | 11-11-2022 | desktop | x86-64 | BIOS | Boots to Orbital, No ethernet driver, Correct video mode not offered (firmware issue) |
| Raspberry Pi | 3 Model B+ | 0.8.0 | Unknown | server | ARM64 | U-Boot | Boots to UART serial console (pl011) |
| Lenovo | ThinkPad Yoga 260 Laptop - Type 20FE | 0.9.0 | 07-09-2024 | demo | x86-64 | UEFI | Boots to Orbital, No audio |
| Lenovo | ThinkPad P14s | 0.9.0 | 17-11-2024 | demo | x86-64 | UEFI | Boots to Orbital, No trackpad or USB mouse input support |
| Lenovo | Yoga S730-13IWL | 0.9.0 | 09-11-2024 | desktop | x86-64 | UEFI | Boots to Orbital, No trackpad or USB mouse input support |

## Broken

| **Vendor** | **Model** | **Redox Version** | **Image Date** | **Variant** | **CPU Architecture** | **Motherboard Firmware** | **Report** |
|------------|-----------|-------------------|----------------|-------------|----------------------|--------------------------|------------|
| HP | EliteBook 2570p | 0.8.0 | 23-11-2022 | demo | x86-64 | BIOS (CSM mode?) | Gets to resolution selection, Fails assert in `src/os/bios/mod.rs:77` after selecting resolution |
| BEELINK | U59 | 0.8.0 | 30-05-2024 | server | x86-64 | Unknown | Aborts after panic in xhcid |
| ASUS | PN41 | 0.8.0 | 30-05-2024 | server | x86-64 | Unknown | Aborts after panic in xhcid |
| Lenovo | G570 | 0.8.0 | 11-11-2022 | desktop | x86-64 | BIOS | Bootloader panics in `alloc_zeroed_page_aligned`, Correct video mode not offered (firmware issue) |
| Lenovo | IdeaPad Y510P | 0.8.0 | 11-11-2022 | desktop | i686 | BIOS | Panics on `phys_to_virt overflow`, probably having invalid mappings for 32-bit |
| Framework | Laptop 16 (AMD Ryzen 7040 Series) | 0.9.0 | 07-09-2024 | server, demo | x86-64 | UEFI | Black screen and unresponsive after the bootloader and resolution selection |
| Toshiba | Satellite L500 | 0.8.0 | 11-11-2022 | desktop | i686 | BIOS | Correct video mode not offered (firmware issue), Panics on `phys_to_virt overflow`, probably having invalid mappings for 32-bit |
| Panasonic | Toughbook CF-18 | 0.8.0 | 11-11-2022 | desktop | i686 | BIOS | Hangs after PIT initialization |
| XMG (Schenker) | Apex 17 (M21) | 0.9.0 | 30-09-2024 | demo, server | x86-64 | UEFI | After selecting resolution, (release) repeats `...::interrupt::irq::ERROR -- Local apic internal error: ESR=0x40` a few times before it freezes; (daily) really slowly prints statements from `...::rmm::INFO` before it abruptly aborts |
| HP | Compaq nc6120 | 0.9.0 | 2024-11-08 | desktop, server | i686 | BIOS | Unloads into memory at a rate slower than 1MB/s after selecting resolution. When unloading is complete the logger initializes and crashes after kernel::acpi, some information about APIC is printed. Boot logs do not progress after this point. |

