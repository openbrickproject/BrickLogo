# BrickLogo

![BrickLogo screenshot](screenshots/v0.2.0.png)

BrickLogo is a Logo programming environment for controlling LEGO motors and sensors. It runs in a terminal and works with hardware from the original DACTA Control Lab through to current Powered UP and LEGO Education Science devices.

The language is considered a spiritual evolution of LEGO TC Logo and Control Lab Logo. If you have used either, the commands should be familiar.

## Supported Devices

| Type | Command | Devices |
| --- | --- | --- |
| LEGO Education Science | `connectto "science "name` | Double Motor, Single Motor, Color Sensor, Controller |
| LEGO Powered UP | `connectto "pup "name` | Move Hub, Powered UP Hub, Technic Hub, Remote, Duplo Train |
| LEGO Education WeDo 1.0 | `connectto "wedo "name` | WeDo USB Hub |
| LEGO DACTA Control Lab | `connectto "controllab "name` | Interface B over serial |
| LEGO Mindstorms RCX | `connectto "rcx "name` | RCX via serial or USB IR tower |
| Raspberry Pi Build HAT | `connectto "buildhat "name` | Powered UP and SPIKE motors and sensors |

Multiple devices can be connected at the same time. Each is given a name and addressed by that name or by qualified port names (for example `"mybot.a`).

## Quick Start

Download the latest release for your platform, unpack it, and run the binary.

```
./bricklogo
```

Example session:

```
? connectto "pup "mybot
Scanning for Powered UP hub...
Connected to Move Hub as "mybot"

? talkto "a
? setpower 5
? onfor 10

? to backandforward
> repeat 4 [onfor 5 wait 5 rd]
> end

? backandforward
```

## Configuration

BrickLogo looks for `bricklogo.config.json` in the current working directory. This is needed for devices that connect over serial.

```json
{
  "controllab": ["/dev/tty.usbserial-AC018HBC"],
  "rcx": ["/dev/ttyS0"]
}
```

Serial paths are used in order as devices are connected. RCX USB towers are detected automatically and do not need a config entry.

## Commands

Connection:

- `connectto "type "name`
- `use "name`
- `disconnect`, `disconnect "name`, `disconnect "all`
- `firmware "device "file` (RCX, Build HAT)

Motor control:

- `talkto "port` or `talkto [a b]`
- `on`, `off`, `onfor`, `setpower`
- `seteven`, `setodd`, `rd`
- `rotate`, `rotateto`, `resetzero`, `rotatetohome`
- `flash`, `alloff`

Sensors:

- `listento "port`
- `sensor "mode`, `sensor?`
- `color`, `light`, `force`, `angle`

Language:

- `make`, `:variable`, `print`, `show`
- `repeat`, `forever`, `if`, `ifelse`, `waituntil`
- `to ... end`, `output`, `stop`, `erase`
- `launch`
- `wait`, `timer`, `resett`
- `namepage`, `save`, `load`, `setdisk`

Type `help` inside BrickLogo for the built-in command summary.

## Documentation

See the [docs](docs/) folder:

- [Introduction](docs/01-introduction.md)
- [Getting Started](docs/02-getting-started.md)
- [Tutorial](docs/03-tutorial.md)
- [Advanced Usage](docs/04-advanced.md)
- [Reference Guide](docs/05-reference.md)

## Development

```
cargo check
cargo test
cargo run -p bricklogo
```

## Status

BrickLogo is in active development. The goal is a usable and fun LEGO/Logo environment for automation and learning.
