# Introduction to BrickLogo

BrickLogo is a Logo programming environment for controlling LEGO motors and sensors. You type commands at a prompt and the hardware responds.

## LEGO/Logo

Logo is a programming language created by Seymour Papert in the late 1960s. Papert also designed LEGO/Logo, which connects the language to physical LEGO models through motors and sensors. LEGO produced two implementations: LEGO TC Logo (1988) and LEGO DACTA Control Lab (1993).

BrickLogo is a spiritual evolution of LEGO/Logo. It draws from both TC Logo and Control Lab, adds support for current hardware, and introduces modern elements like concurrent tasks and networking, in the spirit of the original.

If you have used either TC Logo or Control Lab, the commands will be familiar. If you have not, the language is small and the commands do what their names suggest.

## Hardware

BrickLogo works with six families of LEGO hardware:

| Hardware | Connection | Devices |
| --- | --- | --- |
| LEGO Education Science | Bluetooth | Double Motor, Single Motor, Color Sensor, Controller |
| LEGO Powered UP | Bluetooth | Move Hub, Powered UP Hub, Technic Hub, Remote Control |
| LEGO Education WeDo 1.0 | USB | WeDo Hub with motors and sensors |
| LEGO DACTA Control Lab | Serial | Interface B (8 outputs, 8 sensor inputs) |
| LEGO Mindstorms RCX | IR Tower (serial or USB) | RCX programmable brick |
| Raspberry Pi Build HAT | Serial (Pi GPIO) | Powered UP and SPIKE motors and sensors |

You can connect devices from different families at the same time.

## What you can do

With BrickLogo, you can:

- Turn motors on and off, set their speed and direction, run them for a set time, or rotate them to a position.
- Read sensors: color, distance, force, tilt, temperature, light, rotation.
- Connect several devices at once and address them by name.
- Define procedures (your own commands) and build up complex behaviour from simple parts.
- Run tasks at the same time with `launch`.
- Share variables between BrickLogo instances on different computers over a network.

## The prompt

When you start BrickLogo, you see:

```
?
```

This is the prompt. You type a command and press Enter. BrickLogo runs it and shows the prompt again.

To control a motor, you connect to a device, select a port, and give commands:

```
? connectto "pup "mybot
? talkto "a
? on
```

The motor on port A turns. Type `off` to stop it.

To read a sensor:

```
? listento "b
? print sensor "distance
42
```

To define your own command:

```
? to wiggle
> onfor 10
> rd
> onfor 10
> rd
> end

? wiggle
```

## See also

- [Getting Started](02-getting-started.md) for download and setup.
- [Tutorial](03-tutorial.md) for a guided first session.
- [Advanced Usage](04-advanced.md) for tasks, networking, and multiple devices.
- [Reference Guide](05-reference.md) for the complete list of commands.
