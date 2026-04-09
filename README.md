# BrickLogo - A modern LEGO/Logo REPL

![BrickLogo screenshot](screenshots/v0.2.0.png)

BrickLogo brings the classic LEGO computer-control model into a modern workspace. It is a terminal Logo environment that can talk to LEGO motors, hubs, and sensors across several generations of hardware, while keeping the direct, command-driven feel of LEGO TC Logo and Control Lab.

This repository contains the BrickLogo application, the Logo language runtime, a hardware abstraction layer, and the lower-level protocols used to speak to supported devices.

The full user guide will come later. This README is the front door.

## What It Does

- Runs a terminal REPL for LEGO/Logo programming
- Implements a Logo parser, evaluator, procedures, variables, lists, control flow, and error handling
- Connects to multiple LEGO devices at once
- Controls motors with TC Logo style commands such as `talkto`, `on`, `off`, `onfor`, and `setpower`
- Reads sensors with `listento`, `sensor`, `sensor?`, and typed sensor readers
- Saves and loads Logo procedures using the classic page model

## Supported Devices

| Type | `connectto` command | Devices |
| --- | --- | --- |
| LEGO Education Science | `connectto "science "name` | Double Motor, Single Motor, Color Sensor, Controller |
| LEGO Powered UP | `connectto "pup "name` | Boost Move Hub, Powered Up Hub, Remote, Control+ Hub, WeDo 2.0 Smart Hub, and other devices that use the Powered Up protocol |
| LEGO Education WeDo 1.0 | `connectto "wedo "name` | WeDo USB Hub |
| LEGO DACTA Control Lab | `connectto "controllab "name` | Interface B / Control Lab over serial |
| LEGO Mindstorms RCX | `connectto "rcx "name` | RCX via serial or USB IR tower |
| Raspberry Pi Build HAT | `connectto "buildhat "name` | Build HAT with Powered UP / SPIKE motors and sensors |

Multiple devices can be connected at the same time and addressed either through the active device or by qualified port names such as `"mybot.a`.

## Quick Start

Download the latest release for your platform from the Codeberg releases page, unpack it, and run the `bricklogo` binary.

For example:

```bash
./bricklogo
```

Example session:

```text
? connectto "pup "mybot
Scanning for Powered UP hub...
Connected to Move Hub as "mybot"

? talkto "a
? setpower 5
? onfor 10

? to square
> repeat 4 [onfor 5 wait 5 rd]
> end

? square
```

You can also work in the older TC Logo style:

```text
? connectto "controllab "lab
? talkto "a
? on
? off
```

## Configuration

BrickLogo looks for `bricklogo.config.json` in the current working directory.

This is mainly useful when a device needs a stable identifier, especially Control Lab serial ports. Example:

```json
{
  "controllab": ["/dev/tty.usbserial-AC018HBC"],
  "rcx": ["/dev/ttyS0"]
}
```

For Control Lab and RCX serial towers, the configured serial paths are used in order as devices are connected. RCX USB towers are detected automatically and do not need a config entry.

## Basic Commands

Connection:

- `connectto "type "name`
- `use "name`
- `disconnect`
- `disconnect "name`
- `disconnect "all`
- `firmware "device "file` (RCX firmware upload, Build HAT custom firmware)

Motor control:

- `talkto "port`
- `talkto [a b]`
- `on`
- `off`
- `onfor <tenths>`
- `setpower <0-8>`
- `setleft` / `seteven`
- `setright` / `setodd`
- `rd`
- `rotate <degrees>`
- `rotateto <position>`
- `resetzero`
- `rotatetohome`
- `flash <on> <off>`
- `alloff`

Sensors:

- `listento "port`
- `sensor "mode`
- `sensor?`
- `color`
- `light`
- `force`
- `angle`

Language and pages:

- `make "name <value>`
- `:name`
- `print <value>`
- `show <value>`
- `repeat <n> [...]`
- `if <cond> [...]`
- `ifelse <cond> [...] [...]`
- `wait <tenths>`
- `to <name> <:params> ... end`
- `erase "name`
- `namepage "name`
- `save`
- `load "name`
- `setdisk "path`

Inside BrickLogo, type `help` for the built-in command summary.

## Development

Build the workspace:

```bash
cargo check
```

Run tests:

```bash
cargo test
```

Run the application:

```bash
cargo run -p bricklogo
```

## Status

The documentation is not finished yet. The goal of this repository is not just to expose device protocols, but to rebuild a usable LEGO Logo environment around them.

## Notes On Style

BrickLogo is not trying to imitate the old manuals line for line. But it does take cues from them: direct language, command-first examples, and the assumption that the computer is there to control real things.
