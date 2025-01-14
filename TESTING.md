# Call For Testing

This document covers our call for testing when some feature freeze happens before a new Redox release.

A call for testing is when we freeze the code to only fix bugs in QEMU and real hardware, first Jeremy test the code on his computers and the community is called to test on QEMU and on their hardware.

- [Weekly Images](#weekly-images)
- [Real Hardware](#real-hardware)
    - [Ready Images](#ready-images)
    - [Build Your Image](#build-your-image)
- [QEMU](#qemu)
    - [Ready Images](#ready-images-1)
    - [Build Your Image](#build-your-image-1)

## Weekly Images

Weekly images are Redox images created with the new changes from each week, they contain the latest improvements and bug fixes.

When we start the feature freeze, the weekly images don't add new features but only bug fixes.

## Real Hardware

To start your testing on real hardware you need to download or build a bootable image, this section will explain how to do this.

(To test on real hardware you need the `*livedisk.iso` image variant)

### Ready Images

You can download the weekly images [here](https://static.redox-os.org/img/) or run this command:

(This is an x86-64 image for Intel/AMD computers)

```sh
wget https://static.redox-os.org/img/x86_64/redox_desktop_x86_64*_livedisk.iso.zst
```

- It's using the `desktop` variant which contains Orbital and some tools.
- The `demo` variant contains some games and programs, if you want an advanced testing use this variant.
- If you have problems with the `desktop` variant, try the `server` variant.

(You will need a USB storage device with 4GB or more of space)

### Linux Instructions

We recommend that you use the [Popsicle](https://github.com/pop-os/popsicle) tool on Linux to flash your USB device, follow the steps below:

- Click on [this](https://github.com/pop-os/popsicle/releases) link to open the Popsicle releases page and download the `.AppImage` file of the most recent version.
- Open your file manager, click with the right-button of your mouse on the `.AppImage` file and open the "Properties", find the "Permissions" section and mark it as executable.
- Open the Popsicle AppImage file, select the downloaded ISO and your USB device (in most cases it's the only available device).
- Confirm the flash process and wait until the progress bar reach 100%
- If the flash process had no errors it will give a success warning.
- Now you can restart your Linux distribution and boot Redox.
- Some computers don't come with USB booting enabled, to enable it press the keyboard key to open your UEFI or BIOS setup and allow the booting from USB devices (the name varies from firmware to firmware).
- If you don't know the keyboard keys to open your UEFI/BIOS setup or boot menu, press the Esc or F keys (from 1 until 12), if you press the wrong key or got the wrong timing, don't stop your operating system boot process to try again, as it could corrupt your data.

### Windows Instructions

We recommend that you use the [Rufus](https://rufus.ie/) tool on Windows to flash your USB device, follow the steps below:

- Click on [this](https://rufus.ie/) link to open the Rufus website, move the page until the "Download" section and download the latest version.
- Open Rufus, select the downloaded ISO, wait the Rufus image scanning, select your USB device and click on "Start".
- Confirm the permission to erase the data of your device and wait until the progress bar reach 100%
- If it show a choice window with "ISO" and "DD" mode, select the "DD" mode.
- If the flash process had no errors it will give a success warning.
- Now you can restart your Windows and boot Redox.
- Some computers don't come with USB booting enabled, to enable it press the keyboard key to open your UEFI or BIOS setup and allow the booting from USB devices (the name varies from firmware to firmware).
- If you don't know the keyboard keys to open your UEFI/BIOS setup or boot menu, press the Esc or F keys (from 1 until 12), if you press the wrong key or got the wrong timing, don't stop your operating system boot process to try again, as it could corrupt your data.

### Build Your Image

If you have the Redox build system, run the following command:

```sh
make pull rebuild live
```

This will create the bootable image at `build/your-cpu-arch/livedisk.iso`

(You will need a USB storage device with 4GB or more of space)

### Linux Instructions

We recommend that you use the [Popsicle](https://github.com/pop-os/popsicle) tool on Linux to flash your USB device, follow the steps below:

- Click on [this](https://github.com/pop-os/popsicle/releases) link to open the Popsicle releases page and download the `.AppImage` file of the most recent version.
- Open your file manager, click with the right-button of your mouse on the `.AppImage` file and open the "Properties", find the "Permissions" section and mark it as executable.
- Open the Popsicle AppImage file, select the ISO at `build/your-cpu-arch/livedisk.iso` and your USB device (in most cases it's the only available device).
- Confirm the flash process and wait until the progress bar reach 100%
- If the flash process had no errors it will give a success warning.
- Now you can restart your Linux distribution and boot Redox.
- Some computers don't come with USB booting enabled, to enable it press the keyboard key to open your UEFI or BIOS setup and allow the booting from USB devices (the name varies from firmware to firmware).
- If you don't know the keyboard keys to open your UEFI/BIOS setup or boot menu, press the Esc or F keys (from 1 until 12), if you press the wrong key or got the wrong timing, don't stop your operating system boot process to try again, as it could corrupt your data.

### Windows Instructions

We recommend that you use the [Rufus](https://rufus.ie/) tool on Windows to flash your USB device, follow the steps below:

- Click on [this](https://rufus.ie/) link to open the Rufus website, move the page until the "Download" section and download the latest version.
- Open Rufus, select the ISO at `build/your-cpu-arch/livedisk.iso`, wait the Rufus image scanning, select your USB device and click on "Start".
- Confirm the permission to erase the data of your device and wait until the progress bar reach 100%
- If it show a choice window with "ISO" and "DD" mode, select the "DD" mode.
- If the flash process had no errors it will give a success warning.
- Now you can restart your Windows and boot Redox.
- Some computers don't come with USB booting enabled, to enable it press the keyboard key to open your UEFI or BIOS setup and allow the booting from USB devices (the name varies from firmware to firmware).
- If you don't know the keyboard keys to open your UEFI/BIOS setup or boot menu, press the Esc or F keys (from 1 until 12), if you press the wrong key or got the wrong timing, don't stop your operating system boot process to try again, as it could corrupt your data.

## QEMU

To start your testing on QEMU you need an harddisk image from the build server or build system, this section will explain how to do this.

(To test on QEMU you need the `*harddrive.img` image variant)

### Ready Images

You can download the weekly images [here](https://static.redox-os.org/img/) or run this command:

(This is an x86-64 image for Intel/AMD computers)

```sh
wget https://static.redox-os.org/img/x86_64/redox_desktop_x86_64*_harddrive.img.zst
```

- It's using the `desktop` variant which contains Orbital and some tools.
- The `demo` variant contains some games and programs, if you want an advanced testing use this variant.
- If you have problems with the `desktop` variant, try the `server` variant.

After the image download, open your terminal and run:

(You need to have QEMU installed with the `x86_64` emulator)

```sh
SDL_VIDEO_X11_DGAMOUSE=0 qemu-system-x86_64 -d cpu_reset,guest_errors -smp 4 -m 2048 \
    -chardev stdio,id=debug,signal=off,mux=on,"" -serial chardev:debug -mon chardev=debug \
    -machine q35 -device ich9-intel-hda -device hda-duplex -netdev user,id=net0 \
    -device e1000,netdev=net0 -device nec-usb-xhci,id=xhci -enable-kvm -cpu host \
	-drive file=`echo $HOME/Downloads/redox_desktop_x86_64*_harddrive.img`,format=raw
```

This command will open a QEMU window and boot Redox.

### Build Your Image

If you have the Redox build system, run the following command:

```sh
make pull rebuild qemu
```

This command will update the build system, recipes, build a QEMU image and boot Redox.

## How To Report Errors and Bugs

Read [this](https://doc.redox-os.org/book/ch08-05-troubleshooting.html#debug-methods) section and send a message on the [chat](https://doc.redox-os.org/book/ch13-01-chat.html) or open a [GitLab issue](https://doc.redox-os.org/book/ch12-05-filing-issues.html).