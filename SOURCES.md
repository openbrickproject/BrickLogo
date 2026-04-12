# Protocol Sources and References

This document lists the external sources used to implement each hardware protocol in BrickLogo.

## LEGO Mindstorms RCX (`rust-rcx`)

### IR/USB Protocol
- [RCX Opcode Reference](http://www.mralligator.com/rcx/opcodes.html) — Kekoa Proudfoot. Complete opcode listing with parameters, reply formats, and source types. Primary reference for all motor, sensor, and system commands.
- [RCX Internals](http://www.mralligator.com/rcx/) — Kekoa Proudfoot. Serial protocol framing (55 FF 00 header, complement bytes, checksum), bit encoding (2400 baud, NRZ, odd parity, 38kHz IR modulation), and hub architecture.
- [Basic Protocol Description](http://www.mralligator.com/rcx/protocol.html) — Kekoa Proudfoot. Packet-level protocol description.
- [Decoding RCX IR command protocol](https://ofalcao.pt/blog/2017/decoding-rcx-ir-command-protocol) — O Falcao. Message structure walkthrough with byte-level example.

### USB Tower
- [Linux kernel legousbtower.c](https://github.com/torvalds/linux/blob/master/drivers/usb/misc/legousbtower.c) — Linux kernel USB driver. USB vendor/product IDs (0x0694:0x0001), interrupt endpoint discovery, reset vendor request (0x04), and timing parameters.
- [InfraredBrickTower](https://github.com/hangrydave/InfraredBrickTower) — Cross-platform USB tower application. Referenced for vendor request patterns.
- [LegoUSB Project](https://legousb.sourceforge.net/legousbtower/index.shtml) — Linux USB tower driver documentation.

### Firmware Upload
- [firmdl.c (brickOS)](https://github.com/dlove24/brickos/blob/master/util/firmdl/firmdl.c) — Firmware download utility source. Block size (200 bytes), S-Record parsing, upload sequence (delete, start, transfer, unlock), checksum truncation at 0xCC00, and retry logic.
- [RCX Firmware info](https://pbrick.info/index.html-p=74.html) — Firmware file formats and versions.

### NQC Reference Implementation
- [NQC (BrickBot fork)](https://github.com/BrickBot/nqc) — `RCX_Constants.h` for opcode values, `RCX_Cmd.h` and `RCX_Cmd.cpp` for command construction patterns, `RCX_Link.cpp` for transport layer.

### Sensor Calibration
- [Gaston Journal 2](https://www.convict.lu/Jeunes/Gaston/Gaston_Journal2.htm) — Corrected temperature sensor formula: `T [C] = (817.6 - RAW) / 10.27`. Original Gasperi formula `(785 - RAW) / 8` found to be inaccurate.
- [Building a RCX-compatible temperature sensor](https://www.philohome.com/sensors/tempsensor.htm) — Philo. NTC thermistor characteristics, calibration curves.

## LEGO DACTA Control Lab (`rust-controllab`)

### Serial Protocol
- [node-controllab](https://github.com/nathankellenicki/node-controllab) — Nathan Kellenicki. Node.js reference implementation. Serial handshake sequence, output power encoding, sensor message format, keep-alive protocol, and sensor calibration formulas.
- The handshake strings (`p\0###Do you byte, when I knock?$$$` / `###Just a bit off the block!$$$`) are from the original LEGO Control Lab firmware.
- Baud rate: 9600 (from LEGO DACTA documentation and node-controllab).
- Sensor message format (19 bytes, offsets [14, 10, 6, 2, 16, 12, 8, 4]) from node-controllab protocol implementation.

### Sensor Calibration
- Temperature: `fahrenheit = (760 - raw) / 4.4 + 32`, `celsius = ((760 - raw) / 4.4) * (5/9)` — from node-controllab, originally from LEGO Control Lab Reference Guide.
- Light: `intensity = 146 - raw / 7` — from node-controllab.
- Touch: `pressed = raw < 1000`, `force = 100 - (raw / 1024) * 100` — from node-controllab.
- Rotation delta: extracted from state byte bits 0-1 with sign from bit 2 — from node-controllab.

## LEGO Education WeDo 1.0 (`rust-wedo`)

### HID Protocol
- [node-wedo](https://github.com/nathankellenicki/node-wedo) — Nathan Kellenicki. Node.js reference implementation. USB HID vendor/product IDs (0x0694:0x0003), motor command encoding (9-byte HID report), sensor message format, and sensor calibration.

### Sensor Calibration
- Distance: raw range 71-219 mapped linearly to 0-100 — from node-wedo.
- Tilt events: raw value thresholds for Level/Front/Back/Left/Right — from node-wedo, originally from Linux WeDo driver ranges.
- Sensor type detection: raw type ID ranges for Tilt (28-47) and Distance (170-190) — from node-wedo.

## LEGO Education Science / Coral (`rust-coral`)

### BLE Protocol
- [node-coral](https://github.com/nathankellenicki/node-coral) — Nathan Kellenicki. Node.js reference implementation. Service/characteristic UUIDs, device kind identification from manufacturer data, message type IDs, motor command encoding, sensor notification decoding, and request/response matching.
- LEGO Company ID (0x0397) for BLE manufacturer data — from node-coral.
- Service UUID `0000fd02-*` — from node-coral, originally from LEGO Education Science BLE specification.

### Protocol Details
- Message types and command/result pairs (command ID + 1 = result ID) — from node-coral `protocol.ts`.
- `RESULT_TO_COMMAND` mapping for request/response matching — from node-coral `protocol.ts`.
- `getRequestKey` / `getResponseKey` pattern using message type name + motor bit mask — from node-coral `connection.ts`.
- Device notification format (ID 60, 2 reserved bytes, then sensor payloads) — from node-coral.
- Motor command format (motor bits, direction, end state) — from node-coral.
- Device kinds from hardware byte: SingleMotor (0), DoubleMotor (1), ColorSensor (2), Controller (3) — from node-coral.

### Sensor Data
- Motor notification: bit mask, state, absolute position, power, speed, relative position — from node-coral.
- Color sensor: color ID, reflection, raw RGB, HSV — from node-coral.
- IMU: orientation, yaw face, yaw/pitch/roll, accelerometer XYZ, gyroscope XYZ — from node-coral.
- Color palette (Black=0 through White=10) — from node-coral.

## LEGO Powered UP / LWP3 (`rust-poweredup`)

### BLE Protocol
- [node-poweredup](https://github.com/nathankellenicki/node-poweredup) — Nathan Kellenicki. Node.js reference implementation. Service/characteristic UUIDs, hub type identification, device type enumeration, port output commands, sensor modes, and feedback handling.
- [LEGO Wireless Protocol 3.0](https://lego.github.io/lego-ble-wireless-protocol-docs/) — LEGO. Official LWP3 specification (referenced via node-poweredup implementation).

### Hub Identification
- Hub types from manufacturer data byte: Duplo Train Base (0x20), Move Hub (0x40), Hub (0x41), Remote Control (0x42), Technic Medium Hub (0x80), Technic Small Hub (0x83) — from node-poweredup.
- WeDo 2.0 Smart Hub identified by service UUID rather than manufacturer byte — from node-poweredup.

### Service/Characteristic UUIDs
- LPF2 Service: `00001623-1212-efde-1623-785feabcd123` — from LWP3 spec via node-poweredup.
- LPF2 Characteristic: `00001624-1212-efde-1623-785feabcd123` — from LWP3 spec via node-poweredup.
- WeDo 2.0 UUIDs (Port Type, Sensor Value, Motor Write, etc.) — from node-poweredup.

### Device Types
- 44 device types with IDs from node-poweredup `constants.ts`, cross-referenced with LWP3 specification.
- Motor classification (basic vs tacho vs absolute) — from node-poweredup.

### Sensor Modes and Calibration
- All sensor mode mappings (mode number to event name) per device type — from node-poweredup `devices.ts`.
- Calibration formulas for voltage, current, force, distance, accelerometer, gyroscope — from node-poweredup sensor data parsing.
- Color/distance sensor combined mode parsing — from node-poweredup.
- Duplo color sensor RGB scaling (raw / 4.0) — from node-poweredup.

### Port Output Commands
- Sub-command IDs (0x05-0x0E, 0x51) for motor control — from LWP3 spec via node-poweredup.
- Braking styles: Float (0), Hold (126), Brake (127) — from node-poweredup.
- Feedback flags: Buffer Free (0x01), Completed (0x02), Discarded (0x04), Buffer Empty (0x08) — from node-poweredup.

## Raspberry Pi Build HAT (`rust-buildhat`)

### Serial Protocol
- [Build HAT Serial Protocol](https://datasheets.raspberrypi.com/build-hat/build-hat-serial-protocol.pdf) — Raspberry Pi. Official serial protocol documentation. ASCII text commands at 115200 baud 8N1, port selection, PWM/PID controller modes, sensor mode selection, combi modes, and firmware upload sequence.
- [python-build-hat](https://github.com/RaspberryPiFoundation/python-build-hat) — Raspberry Pi Foundation. Official Python library. Connection strings for attach/detach parsing (`connected to active ID`, `connected to passive ID`, `disconnected`, `timeout during data phase: disconnecting`), firmware upload sequence (clear → load → STX/data/ETX → signature → reboot), and device enumeration.
- [buildhat-alternative](https://github.com/gregorianrants/buildhat-alternative) — Community alternative Python library. Key findings: `selrate` command for setting sensor output period (undocumented in official PDF), speed mode values are in units of 10 deg/sec, default `plimit` is 0.1 (10% power), software PID approach bypassing on-board PID for synchronized multi-motor control.

### Firmware
- [Build HAT firmware source](https://github.com/raspberrypi/buildhat) — Raspberry Pi. BSD 3-Clause licensed. RP2040-based firmware built with Pico SDK. Pre-built binary and signature bundled in `firmware/buildhat/` (sourced from [python-build-hat `buildhat/data/`](https://github.com/RaspberryPiFoundation/python-build-hat/tree/main/buildhat/data)). Uploaded on every power-on when bootloader is detected.
- Checksum algorithm: CRC-32 variant with polynomial `0x1D872B41`, initial value 1 — from python-build-hat source.

### PID Controller
- PID parameter format: `pid <pvport> <pvmode> <pvoffset> <pvformat> <pvscale> <pvunwrap> <Kp> <Ki> <Kd> <windup>` — from Build HAT serial protocol PDF.
- Process variable formats: `s1` (signed byte) for speed, `s4` (signed 32-bit int) for position — from protocol PDF and python-build-hat.
- Speed PID gains (Kp=0.05, Ki=0.03, Kd=0) tuned for minimal ramp-up. Position PID gains (Kp=5, Ki=0, Kd=0.1) from python-build-hat defaults.

### Device Types
- LPF2 device type IDs shared with Powered UP (same physical devices): same hex IDs for motors (0x01, 0x02, 0x26, 0x2E, 0x2F, 0x30, 0x31, 0x41, 0x4B, 0x4C), sensors (0x22, 0x23, 0x25, 0x3D, 0x3E, 0x3F), and other devices (0x08, 0x40) — cross-referenced between Build HAT `list` output and node-poweredup device constants.
- Passive devices (IDs 1-11 decimal) include train motors and lights; active devices communicate via UART with the Build HAT firmware — from Build HAT serial protocol PDF.

### Sensor Modes
- Mode numbers identical to Powered UP / LWP3 for all LPF2 devices — same hardware, same firmware on the device side. Mode mappings verified against node-poweredup `devices.ts`.
- Color/Distance sensors require `set -1` after attachment to enable onboard LEDs — from python-build-hat initialization sequence.
- Motor combi mode `combi 0 1 0 2 0 3 0` combines speed (mode 1), position (mode 2), and absolute position (mode 3) into a single data stream — from Build HAT serial protocol PDF and buildhat-alternative.

## macOS-Specific

### WeDo HID Pre-flight Check
- `ioreg -r -c IOUSBHostDevice -l` for USB device presence detection — determined empirically on macOS Sequoia. Used to avoid hidapi SIGTRAP crash when no WeDo device is connected.
- `system_profiler SPUSBDataType` found unreliable on macOS Sequoia — tested and rejected.

### BLE Retry (Linux)
- `bluez-async` 0.8.2 D-Bus SIGTRAP on notification subscription — observed on Raspberry Pi. Wrapped BLE connect in `catch_unwind` with retry.

## General References

### TC Logo and Control Lab Documentation
- [LEGO TC Logo Reference Guide](https://archive.org/details/lego-tc-logo-reference-guide) — Internet Archive. Original 1989 reference guide for TC Logo commands and syntax. Informed the command model (talkto, on, off, onfor, setpower, etc.).
- [LEGO Control Lab Reference Guide](https://archive.org/details/cl_reference) — Internet Archive. 1995 reference guide. Informed sensor commands and the page model (namepage, save, load).

### S-Record Format
- Motorola S-Record format (S0, S1, S9 records) — standard specification. Per-line checksum validation, 16-bit addressing for S1 records.

## Future / Research (Not Yet Implemented)

### LEGO Scout
- [NXC RCX and Scout Opcode Constants](https://bricxcc.sourceforge.net/nbc/nxcdoc/nxcapi/group___r_c_x_opcode_constants.html) — Scout-specific opcodes: RCX_ScoutOp (0x47), RCX_ScoutRulesOp (0xD5). The Scout shares the RCX IR protocol (2400 baud, 38kHz, same framing) with 2 motor ports and 2 sensor ports.

### LEGO Micro Scout / VLL
- [Programming the LEGO Micro Scout](https://www.elecbrick.com/vll/) — VLL (Visible Light Link) protocol specification. 35-bit barcode encoding with 20ms/40ms timing, 7-bit data + 3-bit checksum, ~29 documented opcodes. Motor commands for fixed-duration forward/reverse (0.5s, 5s) and stop.
- [Controlling a MicroScout from an RCX using VLL](https://pbrick.info/index.html-p=45.html) — VLL through the RCX IR tower. RCX 2.0 firmware enables VLL output mode.
- [mindstorms-vll (GitHub)](https://github.com/JorgePe/mindstorms-vll) — Multiple methods for controlling Code Pilot and MicroScout with VLL.
- VLL checksum formula: `7 - ((n + (n >> 2) + (n >> 4)) & 7)` — from elecbrick.com VLL documentation.

### LEGO Power Functions IR
- [LEGO Power Functions RC Protocol v1.10](https://www.philohome.com/pf/LEGO_Power_Functions_RC_v110.pdf) — Official protocol specification. Pulse-distance encoding at 38kHz, 16-bit messages with toggle/escape/channel/mode/data fields.
- [LEGO Power Functions RC Protocol v1.20](http://images.groundzero.com.pt/LEGO_Power_Functions_RC_v120.pdf) — Updated protocol version.
- [Build an IR-Based LEGO Train Controller](https://circuitcellar.com/research-design-hub/projects/build-an-ir-based-lego-train-controller-part-1/) — Circuit Cellar. Practical implementation of PF IR control.
- Not directly compatible with RCX serial/USB IR tower — PF uses pulse-distance encoding while the tower outputs NRZ serial. Bit-banging through the serial tower is theoretically possible but unreliable.
