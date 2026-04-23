# Hardware Reference

This guide lists every LEGO hub and accessory BrickLogo supports, with its connection method, motor (output) ports, sensor (input) ports, and supported sensor modes.

For the Logo primitives — `connectto`, `listento`, `sensor`, `rotate`, and the rest — see the [Reference Guide](05-reference.md).

## Contents

- [LEGO Education Science (Coral)](#lego-education-science-coral)
- [LEGO Powered UP](#lego-powered-up)
- [LEGO Education WeDo 1.0](#lego-education-wedo-10)
- [LEGO DACTA Control Lab](#lego-dacta-control-lab)
- [LEGO Mindstorms RCX](#lego-mindstorms-rcx)
- [LEGO Mindstorms NXT](#lego-mindstorms-nxt)
- [LEGO Mindstorms EV3](#lego-mindstorms-ev3)
- [LEGO SPIKE Prime / Robot Inventor](#lego-spike-prime--robot-inventor)
- [Raspberry Pi Build HAT](#raspberry-pi-build-hat)

---

## LEGO Education Science (Coral)

Connects via Bluetooth Low Energy. `connectto "science` scans for the first unclaimed hub.

| Device | Output Ports | Sensor Ports |
| --- | --- | --- |
| Double Motor | a, b | tilt, gyro, accel, yaw |
| Single Motor | a | tilt, gyro, accel, yaw |
| Color Sensor | — | color, light, rgb |
| Controller | — | button, joystick |

### Sensor modes — Double Motor / Single Motor

| Port | Mode | Returns |
| --- | --- | --- |
| a, b | `"rotation` | Motor position in degrees |
| a, b | `"speed` | Current motor speed |
| tilt | `"tilt` | Tilt orientation values |
| gyro | `"gyro` | Gyroscope values |
| accel | `"accel` | Accelerometer values |
| yaw | `"yaw` | Yaw angle |

### Sensor modes — Color Sensor

| Port | Mode | Returns |
| --- | --- | --- |
| color | `"color` | Color ID number |
| light | `"light` | Reflected light intensity |
| rgb | `"rgb` | List of `[red green blue]` values |

### Sensor modes — Controller

| Port | Mode | Returns |
| --- | --- | --- |
| button | `"button` | `"true` or `"false` |
| button | `"touch` | `"true` or `"false` |
| joystick | `"joystick` | List of `[left right]` percentages |

---

## LEGO Powered UP

Connects via Bluetooth Low Energy. `connectto "pup` scans for any hub in the LEGO Powered UP / LWP3 family or the older LWP 1.x WeDo 2.0 Smart Hub. Each call picks the first unclaimed hub, so several hubs can be connected at once by repeating the command.

| Hub | Output Ports |
| --- | --- |
| Boost Move Hub | a, b, c, d |
| Powered UP Hub | a, b |
| Technic Medium Hub (Control+) | a, b, c, d |
| Technic Small Hub (SPIKE Essential) | a, b |
| Powered UP Remote | — (input only; button ports `a` and `b`) |
| Duplo Train Base | a |
| WeDo 2.0 Smart Hub | a, b |

Powered UP hubs also expose internal sensors (tilt, voltage, temperature, etc.) as named sensor ports. The LED Light accessory is supported on any output port — `setpower` sets brightness 0–100.

### Sensor modes

Modes depend on which device is attached to each port.

| Device | Modes |
| --- | --- |
| Color/Distance Sensor | color, distance, light, ambient, rgb |
| Technic Color Sensor | color, light, ambient, rgb, hsv, hsvambient |
| Technic Force Sensor | force, touched, tapped |
| Technic Distance Sensor | distance, fastDistance |
| Tacho Motors | rotation |
| Absolute Motors (Technic) | rotation, absolute |
| Tilt Sensor | tilt |
| Internal Voltage | voltage |

### Notes

- The WeDo 2.0 Smart Hub has no position feedback. `rotate`, `rotateto`, `rotatetoabs`, and `resetzero` return an error. Basic motor control (`on`, `off`, `setpower`, `onfor`, `setodd`/`seteven`) and sensors work normally.

---

## LEGO Education WeDo 1.0

Connects via USB. `connectto "wedo` finds the first unclaimed hub.

| Output Ports | Sensor Ports |
| --- | --- |
| a, b | a, b |

### Sensor modes

| Mode | Returns |
| --- | --- |
| `"distance` | Distance (0–100) |
| `"tilt` | Tilt event (0=level, 1=front, 2=back, 3=left, 4=right) |
| `"raw` | Raw sensor value |

Distance and tilt sensors are auto-detected and can be plugged into either port.

---

## LEGO DACTA Control Lab

Connects via serial. `connectto "controllab` requires a serial port listed in `bricklogo.config.json`:

```json
{ "controllab": ["/dev/tty.usbserial-AC018HBC"] }
```

Each `connectto "controllab` call consumes the next path in the array. Typical paths:

- macOS: `/dev/tty.usbserial-XXXXXX`
- Linux: `/dev/ttyUSB0`
- Windows: `COM3` (or similar)

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d, e, f, g, h | 1, 2, 3, 4, 5, 6, 7, 8 |

### Sensor modes

| Mode | Returns |
| --- | --- |
| `"touch` | `"true` or `"false` |
| `"temperature` | Temperature in Celsius |
| `"light` | Light intensity (0–255) |
| `"rotation` | Accumulated rotation count |
| `"raw` | Raw sensor value (0–1023) |

---

## LEGO Mindstorms RCX

Connects via serial or USB IR tower. `connectto "rcx`. USB towers are detected automatically. For a serial tower (PL2303, FTDI, or similar USB-to-serial adapter), list its device path under `"rcx"` in `bricklogo.config.json`:

```json
{ "rcx": ["/dev/ttyS0"] }
```

| Output Ports | Input Ports |
| --- | --- |
| a, b, c | 1, 2, 3 |

### Sensor modes

| Mode | Returns |
| --- | --- |
| `"touch` | `"true` or `"false` |
| `"temperature` | Temperature in Celsius |
| `"light` | Light intensity |
| `"rotation` | Accumulated rotation count |
| `"raw` | Raw sensor value (0–1023) |

### Notes

- The RCX needs firmware loaded once after batteries are inserted. See [Advanced Usage — Firmware upload](04-advanced.md#rcx).

---

## LEGO Mindstorms NXT

Connects via USB or Bluetooth SPP. `connectto "nxt`. USB needs no driver setup on macOS or Linux. For Bluetooth, pair the brick with PIN `1234` at the OS level; macOS then exposes it as `/dev/cu.NXT-DevB`, Linux as `/dev/rfcommN` after `rfcomm bind`.

`connectto "nxt "name` tries USB first and falls back to the next unconsumed entry under `"nxt"` in `bricklogo.config.json`. Valid entries:

- a bare path → Bluetooth SPP (e.g. `"/dev/cu.NXT-DevB"`)
- `"usb"` → force USB, first unclaimed brick
- `"usb:<serial>"` → USB, the brick with the matching iSerial string

```json
{ "nxt": ["/dev/cu.NXT-DevB"] }
```

| Output Ports | Input Ports |
| --- | --- |
| a, b, c | 1, 2, 3, 4 |

### Sensor modes

| Mode | Returns |
| --- | --- |
| `"touch` | `0` or `1` (Touch Sensor) |
| `"light` (alias `"light_active`) | 0–100 light intensity with LED on |
| `"light_inactive` (alias `"ambient`) | 0–100 ambient light with LED off |
| `"sound` (alias `"sound_dba`) | 0–100 A-weighted sound level |
| `"sound_db` | 0–100 unweighted sound level |
| `"pct` | 0–100 raw analog input |
| `"raw` | 0–1023 ADC raw value |
| Motor ports (a–c) `"rotation` | Tacho count in degrees since last `resetzero` |

### Notes

- `rotatetoabs` returns an error — NXT motors have no absolute-position encoder. Use `rotateto 0` after `resetzero` instead.
- I2C ("lowspeed") digital sensors (Ultrasonic 9846, HiTechnic, Mindsensors) are not yet supported.
- EV3-UART sensors (EV3 Color v2, Gyro, IR, Ultrasonic v2) are not supported — NXT firmware has no UART sensor driver.
- Firmware upload is not supported. Custom firmware (NBC/NXC, leJOS NXJ) must be flashed with its own tooling.
- USB on Windows requires a WinUSB driver binding. Use Bluetooth instead, or macOS or Linux.
- The NXT Interactive Servo (9842) works on an EV3 brick. The reverse does not — NXT firmware's encoder sampling is too slow for EV3 Large/Medium servo pulse rates.

---

## LEGO Mindstorms EV3

Connects via USB or Bluetooth SPP. `connectto "ev3`. USB needs no driver setup. For Bluetooth, pair the brick at the OS level and list its serial port path under `"ev3"` in `bricklogo.config.json`. Wi-Fi is not yet implemented.

`connectto "ev3 "name` tries USB first and falls back to the next unconsumed serial path from the config. Valid config entries:

- a bare serial path → Bluetooth SPP
- `"usb"` → force USB HID, first unclaimed brick
- `"usb:<path>"` → USB HID at a specific HID path (useful for multi-EV3 setups)
- `"wifi:discover"` or `"wifi:<ip>"` → reserved for future Wi-Fi support (currently errors)

```json
{ "ev3": ["/dev/cu.EV3-SerialPort-14"] }
```

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d | 1, 2, 3, 4 |

### Sensor modes

Modes depend on the sensor type plugged into the port. Both EV3 and NXT sensors are supported.

| Sensor | Modes |
| --- | --- |
| EV3 Color Sensor | light, ambient, color, rgb |
| EV3 Touch Sensor | touch |
| EV3 Ultrasonic Sensor | distance |
| EV3 Gyro Sensor | angle, rate |
| EV3 Infrared Sensor | distance, seek, remote |
| NXT Touch Sensor | touch |
| NXT Light Sensor | light, ambient |
| NXT Sound Sensor | sound |
| NXT Ultrasonic Sensor | distance |
| NXT Temperature Sensor | temperature |
| Motor ports (a–d) | rotation, raw |

Use `"raw` on any sensor port to read the default mode as a percentage.

### Notes

- `rotatetoabs` returns an error — EV3 motors have no absolute-position encoder. Use `rotateto 0` after `resetzero` instead.
- Motor control uses raw PWM power, matching every other BrickLogo adapter. Motors load-droop under heavy loads.
- Firmware upload and file transfer are not supported.
- Daisy-chained bricks are not supported (layer is always 0).

---

## LEGO SPIKE Prime / Robot Inventor

Connects via USB or Bluetooth Low Energy. `connectto "spike`. Supports the same LPF2 motors and sensors as the Powered UP family and the Build HAT.

Requires Hub OS 3.0 or later. If the hub is running older firmware, see [Advanced Usage — Firmware upload](04-advanced.md#spike-prime--robot-inventor) to upgrade it via BrickLogo.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d, e, f | a, b, c, d, e, f |

Any port can host a motor or sensor. The hub also has a built-in IMU accessible as sensor ports `tilt`, `gyro`, and `accel`.

### Sensor modes

| Device | Modes |
| --- | --- |
| Color Sensor | color, light |
| Distance Sensor | distance |
| Force Sensor | force, touched |
| Tacho Motors | rotation, speed |
| Absolute Motors (Technic) | rotation, speed, absolute |
| Hub IMU (`tilt`) | tilt |
| Hub IMU (`gyro`) | gyro |
| Hub IMU (`accel`) | accel |

---

## Raspberry Pi Build HAT

Connects via serial on the Raspberry Pi (`/dev/serial0`). `connectto "buildhat` needs no configuration — see [NOTES.md](../NOTES.md) for Raspberry Pi setup. Supports the same LPF2 motors and sensors as the Powered UP family over wired ports.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d | a, b, c, d |

### Sensor modes

Same as [Powered UP](#lego-powered-up) above, plus a `speed` mode on motor ports.

### Notes

- Build HAT firmware is uploaded automatically on each `connectto "buildhat`. For manual flashing, see [Advanced Usage — Firmware upload](04-advanced.md#build-hat).

---

## See also

- [Reference Guide](05-reference.md) — Logo language and primitives.
- [Advanced Usage](04-advanced.md) — scripts, networking, firmware upload.
