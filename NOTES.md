# Platform Notes

## Raspberry Pi (all models)

### Serial console must be disabled

The Build HAT uses the Pi's serial port. By default, the Pi uses this for a login console which must be disabled:

1. Run `sudo raspi-config`
2. Interface Options > Serial Port
3. "Would you like a login shell to be accessible over serial?" — **No**
4. "Would you like the serial port hardware to be enabled?" — **Yes**
5. Reboot

### Linux dependencies

Building from source on Raspbian requires:

```bash
sudo apt install libdbus-1-dev libudev-dev pkg-config
```

## Raspberry Pi 5

The Pi 5 uses a different UART controller and requires additional configuration for the Build HAT.

Add the following to `/boot/firmware/config.txt`:

```
enable_uart=1
dtoverlay=buildhat
```

Then reboot. On Pi 3 and Pi 4 these are not needed — the default UART configuration works with the Build HAT.
