# SPIKE Prime / Robot Inventor Firmware

These are LEGO Education SPIKE Prime hub firmware images, Hub OS version 3.4.0
(built 2025-03-27), extracted unmodified from the LEGO Education SPIKE app
v3.6.0. LEGO has announced that the SPIKE app is being deprecated and will no
longer be distributed; BrickLogo bundles the firmware here so BrickLogo users
can continue to update their hubs after the app is withdrawn.

SPIKE Prime (set 45678) and MINDSTORMS Robot Inventor (set 51515) share the
same hub hardware and the same Hub OS build. Two hardware revisions of the
hub exist, compiled for different STM32 microcontrollers:

| File | MCU | Hardware revision |
| --- | --- | --- |
| `prime-f4-hubos-3.4.0-dfuse.gz` | STM32F413 | Original (45678 and 51515 retail) |
| `prime-h5-hubos-3.4.0-dfuse.gz` | STM32H562 | Refreshed (2026+) |

Each file is a gzipped STM32 DfuSe image. BrickLogo decompresses in memory and uploads via the STM32 DFU bootloader.

`bricklogo --firmware spike` uploads through the STM32 DFU bootloader and automatically selects the correct image by reading the bootloader's flash memory map. To enter the bootloader, hold the Bluetooth button on the hub while plugging in the USB cable.

BrickLogo includes these files solely to let end users update their own hubs
once the LEGO Education SPIKE app is no longer available. The firmware is
LEGO's proprietary work; the copyright remains with the LEGO Group, and this
inclusion should not be construed as a license. If the LEGO Group has any
objection to its redistribution here, please open an issue on the BrickLogo
repository and it will be removed promptly.
