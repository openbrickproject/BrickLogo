# Getting Started

## What you need

- A computer running macOS, Windows, or Linux (including Raspberry Pi).
- A LEGO device to control.
- For Bluetooth devices: a Bluetooth adapter (built in on most laptops and all Raspberry Pi models).
- For Control Lab or RCX serial tower: a USB-to-serial adapter.
- For Build HAT: a Raspberry Pi with the Build HAT attached.

## Install

### macOS / Linux

```
curl -fsSL https://raw.githubusercontent.com/openbrickproject/BrickLogo/main/scripts/install.sh | sh
```

### Windows (PowerShell)

```
irm https://raw.githubusercontent.com/openbrickproject/BrickLogo/main/scripts/install.ps1 | iex
```

This installs BrickLogo to `~/.bricklogo/` (including example scripts and documentation) and adds it to your PATH.

### Manual install

Alternatively, download the latest release for your platform from the [GitHub releases page](https://github.com/openbrickproject/BrickLogo/releases). The release is a zip containing the `bricklogo` binary and example programs. Unpack it anywhere.

## Running

Open a terminal and run:

```
bricklogo
```

If you installed manually, go to the folder where you unpacked the release and run `./bricklogo` (macOS / Linux) or `bricklogo.exe` (Windows).

You should see the BrickLogo header and a `?` prompt.

## Permissions

On Linux, some devices require elevated access. You may need `sudo` or a udev rule to allow access.

```
sudo ./bricklogo
```

## Configuration 

### Control Lab and RCX serial tower

Control Lab and the RCX serial IR tower use a serial port. Create a file called `bricklogo.config.json` in the same directory as the binary to tell BrickLogo which port to use:

```json
{
  "controllab": ["/dev/tty.usbserial-AC018HBC"]
}
```

The path depends on your operating system. On macOS it looks like `/dev/tty.usbserial-XXXXXX`. On Linux it is `/dev/ttyUSB0` or similar. On Windows it is `COM3` or similar.

For multiple Control Labs, list all ports:

```json
{
  "controllab": ["/dev/ttyUSB0", "/dev/ttyUSB1"]
}
```

The first `connectto "controllab` uses the first path, the second uses the second.

The RCX serial tower uses the same pattern:

```json
{
  "rcx": ["/dev/ttyS0"]
}
```

RCX USB towers are detected automatically and do not need a config entry.

### Raspberry Pi with Build HAT

The Build HAT uses the Pi's GPIO serial port. Some system configuration is needed before BrickLogo can use it.

#### Disable serial console (all Pi models)

1. Run `sudo raspi-config`.
2. Go to Interface Options, then Serial Port.
3. "Would you like a login shell to be accessible over serial?" Choose No.
4. "Would you like the serial port hardware to be enabled?" Choose Yes.
5. Reboot.

#### Pi 5 only

Add these lines to `/boot/firmware/config.txt`:

```
enable_uart=1
dtoverlay=buildhat
```

Reboot. This is not needed on Pi 3 or Pi 4.

## Quitting

Type `bye` or `exit` at the prompt. You can also press Ctrl+C. All devices are disconnected automatically.

## See also

- [Tutorial](03-tutorial.md) for a guided first session.
- [Advanced Usage](04-advanced.md) for tasks, networking, and multiple devices.
- [Reference Guide](05-reference.md) for the complete list of commands.
