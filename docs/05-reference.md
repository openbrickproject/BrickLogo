# BrickLogo Reference Guide

*v0.2.0 — A modern LEGO/Logo programming environment*

## Contents

1. [Connecting to Devices](#1-connecting-to-devices)
2. [Motor Control](#2-motor-control)
3. [Sensors](#3-sensors)
4. [Logo Language](#4-logo-language)
5. [Procedures](#5-procedures)
6. [Control Flow](#6-control-flow)
7. [Variables](#7-variables)
8. [Arithmetic](#8-arithmetic)
9. [Logic and Predicates](#9-logic-and-predicates)
10. [Words and Lists](#10-words-and-lists)
11. [Output](#11-output)
12. [Timing](#12-timing)
13. [Pages and Files](#13-pages-and-files)
14. [REPL Commands](#14-repl-commands)
15. [Configuration](#15-configuration)
- [Appendix A: Supported Devices](#appendix-a-supported-devices)
- [Appendix B: Sensor Modes by Device](#appendix-b-sensor-modes-by-device)

---

## 1. Connecting to Devices

Before you can control motors or read sensors, you must connect to a device. BrickLogo supports six families of LEGO hardware. Each connected device is given a name that you choose, and that name is how you refer to it for the rest of the session.

### `connectto`

```
connectto "type "name
```

Scans for and connects to a LEGO device. The *type* tells BrickLogo what kind of device to look for. The *name* is any word you choose — it becomes the device's identifier for the rest of the session.

| Type | Hardware |
| --- | --- |
| `"science` | LEGO Education Science (Coral) — Single Motor, Double Motor, Color Sensor, Controller |
| `"pup` | LEGO Powered UP — Move Hub, Powered UP Hub, Technic hubs, Remote Control, Duplo Train Base |
| `"wedo` | LEGO Education WeDo 1.0 — USB Hub |
| `"controllab` | LEGO DACTA Control Lab — Interface B over serial |
| `"rcx` | LEGO Mindstorms RCX — via serial or USB IR tower |
| `"buildhat` | Raspberry Pi Build HAT — Powered UP / SPIKE motors and sensors |

```
? connectto "science "robot
Scanning for LEGO Education Science...
Connected to LEGO Education Science Double Motor as "robot"
```

```
? connectto "controllab "lab
Scanning for LEGO Control Lab...
Connected to LEGO Control Lab as "lab"
```

You can connect multiple devices at the same time. Each must have a unique name.

```
? connectto "science "robot
? connectto "controllab "lab
```

The first device you connect becomes the **active device**. Commands like `talkto` and `listento` apply to the active device unless you specify otherwise.

### `disconnect`

```
disconnect
disconnect "name
disconnect "all
```

Disconnects a device. With no argument, disconnects the active device. With a name, disconnects that specific device. With `"all`, disconnects every device.

```
? disconnect "lab
? disconnect "all
? disconnect
```

### `use`

```
use "name
```

Sets the active device. When multiple devices are connected, `use` tells BrickLogo which one to address with subsequent commands.

```
? use "robot
? talkto "a
? on
```

You can also address ports on a specific device without changing the active device by using qualified port names:

```
? talkto "lab.a
```

This selects port A on the device named "lab", regardless of which device is currently active.

### Why names matter

Every connected device has a name. This name is how you refer to it in every command that touches hardware. You pick the name when you connect, and it stays for the duration of the session.

Names let you work with multiple devices naturally. A classroom robot might be `"turtle`, a conveyor belt might be `"belt`, and a sensor station might be `"station`. Qualified port names like `"turtle.a` or `"station.1` let you reach any port on any device from anywhere in your program.

> **Control Lab note:** The Control Lab connects over a serial port. BrickLogo reads the serial port path from `bricklogo.config.json`. See the [Configuration](#15-configuration) section.

---

## 2. Motor Control

Motor control in BrickLogo follows the LEGO/Logo model. You select which ports to talk to, then issue commands. The selected ports remember their direction and power until you change them.

### `talkto`

*alias: tto*

```
talkto "port
talkto [port1 port2 ...]
```

Selects one or more output ports for motor commands. All subsequent motor commands (`on`, `off`, `onfor`, `setpower`, etc.) apply to the selected ports.

```
? talkto "a
? talkto [a b]
? talkto "robot.a
```

Port names depend on the device. A Science Double Motor has ports `a` and `b`. A Control Lab has ports `a` through `h`. A Powered UP hub has ports matching the physical labels on the hub.

### `on`

```
on
```

Turns on the selected ports. The motors run at the current power level and direction until you tell them to stop.

```
? talkto [a b]
? on
```

### `off`

```
off
```

Turns off the selected ports.

```
? off
```

### `onfor`

```
onfor tenths
```

Turns on the selected ports for the given number of tenths of a second, then turns them off. The command does not return until the time has elapsed.

```
? onfor 10
```

Runs the motors for 1 second.

```
? onfor 50
```

Runs the motors for 5 seconds.

> On devices with encoder motors (Science, Powered UP tacho motors, Build HAT tacho motors), the device handles the timing internally and reports when it finishes. On devices without encoders (WeDo, Control Lab, basic Powered UP motors), BrickLogo times the run in software.

### `setpower`

*alias: sp*

```
setpower level
```

Sets the power level for the selected ports. The level is a number from 0 to 8, where 0 is off and 8 is full power. The default power level is 4.

```
? setpower 8
? on
```

If the motors are already running, the power change takes effect immediately.

### `seteven`

*alias: setleft*

```
seteven
```

Sets the direction of the selected ports to "even" (the default forward direction). If the motors are running, the direction change takes effect immediately.

### `setodd`

*alias: setright*

```
setodd
```

Sets the direction of the selected ports to "odd" (the reverse direction).

### `rd`

```
rd
```

Reverses the direction of the selected ports. If they were set to even, they become odd, and vice versa. If the motors are running, the change takes effect immediately.

```
? talkto "a
? on
? rd
? off
```

### `rotate`

```
rotate degrees
```

Rotates the selected ports by the given number of degrees. The command does not return until the rotation is complete. Requires a device with encoder motors (Science, Powered UP tacho motors, or Build HAT tacho motors).

```
? rotate 360
```

Rotates one full turn.

```
? rotate 90
```

Rotates a quarter turn.

### `rotateto`

```
rotateto position
```

Rotates the selected ports to an absolute position in degrees. The device determines the shortest path. The command does not return until the motor reaches the position. Requires encoder motors.

```
? rotateto 0
? rotateto 180
```

### `resetzero`

```
resetzero
```

Resets the encoder zero point on the selected ports. After this command, the current physical position of the motor is considered position 0.

### `rotatetohome`

```
rotatetohome
```

Rotates the selected ports back to absolute position 0. The command does not return until the motors reach home. Requires encoder motors.

### `flash`

```
flash on-tenths off-tenths
```

Flashes the selected ports on and off repeatedly. The port turns on for *on-tenths* tenths of a second, then off for *off-tenths*, and repeats until another motor command is issued or `alloff` is called.

```
? flash 5 5
```

Flashes half a second on, half a second off.

```
? flash 2 10
```

Brief flash every second.

### `alloff`

*alias: ao*

```
alloff
```

Immediately stops all motors on all connected devices. Also cancels any active flashes. This is the emergency stop.

```
? alloff
```

---

## 3. Sensors

Sensors work like motors in reverse. You select a port to listen to, then read its value.

### `listento`

*alias: lto*

```
listento "port
listento [port1 port2 ...]
```

Selects one or more input ports for sensor reading. Subsequent sensor commands apply to the selected ports.

```
? listento "1
? listento [1 2]
? listento "tilt
```

Port names depend on the device. A Control Lab has input ports `1` through `8`. A Science device has named sensor ports like `tilt`, `gyro`, `accel`, and `color`. Powered UP hubs have sensors on the same lettered ports as motors, plus internal sensors like `tilt` and `voltage`.

### `sensor`

```
sensor "mode
```

Reads the current value from the selected sensor port in the given mode. Returns a number, a word, or a list, depending on the sensor and mode.

```
? listento "1
? print sensor "touch
true

? listento "color
? print sensor "color
3
```

The available modes depend on the device and port. See [Appendix B](#appendix-b-sensor-modes-by-device) for a complete list.

### `sensor?`

```
sensor?
```

Reads the selected sensor with no mode specified and returns the result as a boolean word (`"true` or `"false`). Useful in `waituntil` and `if` conditions.

```
? waituntil [sensor?]
```

### `color`

```
color
```

Shorthand for reading the selected sensor in color mode. Returns the color ID as a number.

```
? listento "color
? print color
9
```

### `light`

```
light
```

Shorthand for reading the selected sensor in reflect mode. Returns the reflected light intensity as a number.

### `force`

```
force
```

Shorthand for reading the selected sensor in force mode. Returns the force value as a number.

### `angle`

```
angle
```

Shorthand for reading the selected sensor in rotate mode. Returns the motor position in degrees as a number.

---

## 4. Logo Language

BrickLogo implements the Logo programming language. If you have used LEGO TC Logo, LEGO DACTA Control Lab, or any other Logo dialect, the basics will be familiar.

### Words

A word is a piece of text. You write a word by putting a `"` in front of it:

```
? print "hello
hello
```

Words with spaces use vertical bars:

```
? print |hello world|
hello world
```

### Numbers

Numbers are written as you would expect:

```
? print 42
42
? print 3.14
3.14
? print -7
-7
```

### Lists

A list is a collection of items enclosed in square brackets:

```
? print [a b c]
a b c
? print [1 2 3]
1 2 3
```

### Variables

You set a variable with `make` and read it with a colon:

```
? make "speed 5
? print :speed
5
```

### Infix operators

BrickLogo supports `+`, `-`, `*`, `/`, `=`, `<`, and `>` as infix operators:

```
? print 3 + 4
7
? print :speed * 2
10
? if :speed > 3 [print "fast]
fast
```

### Comments

A semicolon begins a comment. Everything after it on that line is ignored:

```
? on ; start the motors
```

---

## 5. Procedures

A procedure is a named sequence of commands that you define. Once defined, you can use it like any built-in command.

### `to` ... `end`

```
to name :param1 :param2 ...
```

Defines a new procedure. Type `to` followed by the name and any parameters (prefixed with `:`). BrickLogo shows `>` to indicate you are inside a definition. Type `end` to finish.

```
? to square :size
> repeat 4 [onfor :size]
> end

? square 10
```

Procedures can call other procedures, including themselves (recursion).

```
? to countdown :n
> if :n = 0 [stop]
> print :n
> wait 10
> countdown :n - 1
> end

? countdown 5
5
4
3
2
1
```

### `output` / `op`

```
output value
op value
```

Returns a value from a procedure. The procedure ends immediately and the value is passed back to the caller.

```
? to double :x
> output :x * 2
> end

? print double 5
10
```

### `stop`

```
stop
```

Exits the current procedure early without returning a value.

```
? to careful :n
> if :n < 0 [print "negative stop]
> print :n
> end
```

### `erase`

```
erase "name
```

Removes a procedure definition.

```
? erase "square
```

---

## 6. Control Flow

### `repeat`

```
repeat count [commands]
```

Runs the commands the given number of times.

```
? repeat 4 [onfor 10 rd]
```

### `forever`

```
forever [commands]
```

Runs the commands in an endless loop. Press Escape to stop.

```
? forever [onfor 10 wait 5]
```

### `if`

```
if condition [commands]
```

Runs the commands only if the condition is `"true`.

```
? if :speed > 5 [print "fast]
```

### `ifelse`

```
ifelse condition [then-commands] [else-commands]
```

Runs the first block if the condition is `"true`, the second block if `"false`.

```
? ifelse :speed > 5 [print "fast] [print "slow]
```

`ifelse` can also be used as a reporter inside an expression:

```
? make "label ifelse :speed > 5 ["fast] ["slow]
```

### `waituntil`

```
waituntil [condition]
```

Pauses execution until the condition becomes `"true`. The condition is re-evaluated approximately 60 times per second. Useful for waiting on sensor readings.

```
? waituntil [sensor? "1]
? waituntil [timer > 100]
```

### `carefully`

```
carefully [body] [handler]
```

Runs the body. If any error occurs during the body, runs the handler instead. Useful for error recovery.

```
? carefully [connectto "science "bot] [print "failed]
```

`carefully` can also be used as a reporter — the handler can return a fallback value:

```
? make "val carefully [sensor "touch] ["false]
```

---

## 7. Variables

### `make`

```
make "name value
```

Creates or updates a variable. The name must be preceded by `"`. The value can be a number, word, or list.

```
? make "speed 5
? make "greeting "hello
? make "ports [a b c]
```

### `:name`

```
:name
```

Reads the value of a variable. This is not a command — it is an expression that produces the variable's value.

```
? print :speed
5
```

### `thing`

```
thing "name
```

Returns the value of a variable. Equivalent to `:name`, but takes the name as a quoted word. Useful when the variable name is itself computed.

```
? make "x 42
? print thing "x
42
```

---

## 8. Arithmetic

### `sum`

```
sum a b
```

Returns the sum of two numbers.

```
? print sum 3 4
7
```

### `difference`

```
difference a b
```

Returns *a* minus *b*.

```
? print difference 10 3
7
```

### `product`

```
product a b
```

Returns the product of two numbers.

```
? print product 6 7
42
```

### `quotient`

```
quotient a b
```

Returns *a* divided by *b*.

```
? print quotient 10 3
3.3333333333333335
```

### `remainder`

```
remainder a b
```

Returns the remainder when *a* is divided by *b*.

```
? print remainder 10 3
1
```

### `minus`

```
minus number
```

Returns the negation of a number.

```
? print minus 5
-5
```

### `abs`

```
abs number
```

Returns the absolute value of a number.

```
? print abs -7
7
```

### `sqrt`

```
sqrt number
```

Returns the square root of a number.

```
? print sqrt 16
4
```

### `sin`

```
sin degrees
```

Returns the sine of an angle in degrees.

```
? print sin 90
1
```

### `cos`

```
cos degrees
```

Returns the cosine of an angle in degrees.

```
? print cos 0
1
```

### `tan`

```
tan degrees
```

Returns the tangent of an angle in degrees.

```
? print tan 45
1
```

### `random`

```
random max
```

Returns a random integer from 0 up to (but not including) *max*.

```
? print random 10
7
```

### `int`

```
int number
```

Returns the integer part of a number (truncates toward zero).

```
? print int 3.7
3
? print int -3.7
-3
```

### `round`

```
round number
```

Rounds a number to the nearest integer.

```
? print round 3.5
4
? print round 3.4
3
```

---

## 9. Logic and Predicates

Logic operations work with the words `"true` and `"false`. Predicates are commands that return one of these values.

### `and`

```
and a b
```

Returns `"true` if both *a* and *b* are `"true`.

```
? print and "true "false
false
```

### `or`

```
or a b
```

Returns `"true` if either *a* or *b* is `"true`.

### `not`

```
not value
```

Returns `"true` if the value is `"false`, and vice versa.

### `equal?`

```
equal? a b
```

Returns `"true` if *a* and *b* are equal. Word comparison is case-insensitive.

```
? print equal? 3 3
true
? print equal? "hello "HELLO
true
```

### `number?`

```
number? value
```

Returns `"true` if the value is a number.

### `word?`

```
word? value
```

Returns `"true` if the value is a word.

### `list?`

```
list? value
```

Returns `"true` if the value is a list.

### `empty?`

```
empty? value
```

Returns `"true` if the value is an empty list or an empty word.

### `member?`

```
member? item list-or-word
```

Returns `"true` if the item is found in the list or word.

```
? print member? "b [a b c]
true
? print member? "x "hello
false
```

### `name?`

```
name? "name
```

Returns `"true` if a variable with that name exists.

```
? make "x 5
? print name? "x
true
? print name? "y
false
```

---

## 10. Words and Lists

### `first`

```
first value
```

Returns the first element of a list, or the first character of a word.

```
? print first [a b c]
a
? print first "hello
h
```

### `last`

```
last value
```

Returns the last element of a list, or the last character of a word.

```
? print last [a b c]
c
```

### `butfirst`

*alias: bf*

```
butfirst value
```

Returns everything except the first element.

```
? print butfirst [a b c]
b c
? print bf "hello
ello
```

### `butlast`

*alias: bl*

```
butlast value
```

Returns everything except the last element.

### `item`

```
item index value
```

Returns the item at the given position (1-based) from a list or word.

```
? print item 2 [a b c]
b
```

### `count`

```
count value
```

Returns the number of elements in a list, or the number of characters in a word.

```
? print count [a b c]
3
? print count "hello
5
```

### `fput`

```
fput item list
```

Returns a new list with the item added at the front.

```
? print fput "x [a b c]
x a b c
```

### `lput`

```
lput item list
```

Returns a new list with the item added at the end.

```
? print lput "x [a b c]
a b c x
```

### `list`

```
list a b
```

Returns a list containing the two items.

```
? print list 1 2
1 2
```

### `sentence`

*alias: se*

```
sentence a b
```

Returns a flat list combining both values. If either value is a list, its elements are included directly (not nested).

```
? print sentence [1 2] [3 4]
1 2 3 4
? print se "a "b
a b
```

### `word`

```
word a b
```

Joins two words into a single word.

```
? print word "hello "world
helloworld
```

---

## 11. Output

### `print`

*alias: pr*

```
print value
```

Prints the value on a new line. Lists are printed without brackets.

```
? print 42
42
? print [a b c]
a b c
```

### `show`

```
show value
```

Like `print`, but lists are printed with their brackets.

```
? show [a b c]
[a b c]
```

### `type`

```
type value
```

Prints the value without a newline. Subsequent `type` commands continue on the same line.

```
? type "hello type "world
helloworld
```

---

## 12. Timing

### `wait`

```
wait tenths
```

Pauses execution for the given number of tenths of a second.

```
? wait 10
```

Waits 1 second.

### `timer`

```
timer
```

Returns the number of tenths of a second since BrickLogo started (or since the last `resett`).

```
? print timer
243
```

### `resett`

```
resett
```

Resets the timer to zero.

```
? resett
? wait 10
? print timer
10
```

---

## 13. Pages and Files

BrickLogo uses the classic Logo page model for saving and loading procedures. A page is a file on disk that contains all your procedure definitions.

### `namepage`

*alias: np*

```
namepage "name
```

Sets the current page name. This name is used when you call `save`.

```
? namepage "turtle
```

### `save`

```
save
```

Saves all currently defined procedures to a file based on the page name. The file is saved in the current disk directory with a `.logo` extension.

```
? namepage "turtle
? save
Saved /home/user/turtle.logo
```

### `load`

*aliases: getpage, gp*

```
load "name
```

Loads procedures from a `.logo` file. The procedures are defined and ready to use immediately. Also sets the page name to the loaded file.

```
? load "turtle
Loaded /home/user/turtle.logo
```

### `setdisk`

```
setdisk "path
```

Changes the directory where pages are saved and loaded. The path is relative to the current disk directory.

```
? setdisk "projects
Disk set to /home/user/projects
```

### `disk`

```
disk
```

Returns the current disk directory path.

```
? print disk
/home/user
```

---

## 14. REPL Commands

These commands are part of the BrickLogo terminal, not the Logo language. They cannot be used inside procedures.

| Command | Action |
| --- | --- |
| `help` | Show the built-in command summary. Scroll with arrow keys, press `q` or Escape to exit. |
| `clear` | Clear the output history. |
| `bye` / `exit` | Quit BrickLogo. Disconnects all devices. |
| `firmware "device "file` | Upload firmware to an RCX or Build HAT. |

Press **Escape** during a running program to stop it.

Press **Ctrl+C** to quit immediately.

Use the **up** and **down** arrow keys to browse command history.

---

## 15. Configuration

BrickLogo looks for a file called `bricklogo.config.json` in the current working directory when it starts. This file provides device-specific settings.

```json
{
  "controllab": ["/dev/tty.usbserial-AC018HBC"],
  "rcx": ["/dev/ttyS0"]
}
```

### Control Lab serial ports

The Control Lab connects over a serial port. The `controllab` array lists the serial port paths in order. Each time you `connectto "controllab`, the next path in the list is used.

On macOS, the path looks like `/dev/tty.usbserial-XXXXXX`. On Windows, it is `COM3` or similar. On Linux, it is `/dev/ttyUSB0` or similar.

### RCX serial towers

The `rcx` array lists serial port paths for the RCX IR tower. RCX USB towers are detected automatically and do not need a config entry.

### Multiple devices

If you have multiple Control Labs, list all their serial ports:

```json
{ "controllab": ["/dev/ttyUSB0", "/dev/ttyUSB1"] }
```

The first `connectto "controllab` uses the first path, the second uses the second, and so on.

### Build HAT

The Build HAT requires no configuration. It always uses `/dev/serial0` on the Raspberry Pi. See [NOTES.md](../NOTES.md) for Raspberry Pi setup instructions.

---

## Appendix A: Supported Devices

### LEGO Education Science (Coral)

Connects via Bluetooth Low Energy. Use `connectto "science`.

| Device | Output Ports | Sensor Ports |
| --- | --- | --- |
| Double Motor | a, b | tilt, gyro, accel, yaw |
| Single Motor | a | tilt, gyro, accel, yaw |
| Color Sensor | — | color, reflect, rgb |
| Controller | — | button, joystick |

### LEGO Powered UP

Connects via Bluetooth Low Energy. Use `connectto "pup`.

| Hub | Output Ports |
| --- | --- |
| Move Hub | a, b, c, d |
| Powered UP Hub | a, b |
| Technic Medium Hub | a, b, c, d |
| Technic Small Hub | a, b |
| Remote Control | a, b |
| Duplo Train Base | a |

Powered UP hubs also expose internal sensors (tilt, voltage, temperature, etc.) as named sensor ports.

### LEGO Education WeDo 1.0

Connects via USB. Use `connectto "wedo`.

| Ports | Sensors |
| --- | --- |
| a, b | distance, tilt |

### LEGO DACTA Control Lab

Connects via serial. Use `connectto "controllab`. Requires a serial port configured in `bricklogo.config.json`.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d, e, f, g, h | 1, 2, 3, 4, 5, 6, 7, 8 |

### LEGO Mindstorms RCX

Connects via serial or USB IR tower. Use `connectto "rcx`.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c | 1, 2, 3 |

### Raspberry Pi Build HAT

Connects via serial on the Raspberry Pi. Use `connectto "buildhat`. Supports the same Powered UP / SPIKE motors and sensors as the Powered UP family over wired LPF2 ports.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d | a, b, c, d |

---

## Appendix B: Sensor Modes by Device

### Control Lab

| Mode | Returns |
| --- | --- |
| `"touch` | `"true` or `"false` |
| `"temperature` | Temperature in Celsius |
| `"light` | Light intensity (0–255) |
| `"rotation` | Accumulated rotation count |
| `"raw` | Raw sensor value (0–1023) |

### WeDo 1.0

| Mode | Returns |
| --- | --- |
| `"distance` | Distance (0–100) |
| `"tilt` | Tilt event (0=level, 1=front, 2=back, 3=left, 4=right) |
| `"raw` | Raw sensor value |

### Science — Double Motor / Single Motor

| Port | Mode | Returns |
| --- | --- | --- |
| a, b | `"rotation` | Motor position in degrees |
| a, b | `"speed` | Current motor speed |
| tilt | `"tilt` | Tilt orientation values |
| gyro | `"gyro` | Gyroscope values |
| accel | `"accel` | Accelerometer values |
| yaw | `"yaw` | Yaw angle |

### Science — Color Sensor

| Port | Mode | Returns |
| --- | --- | --- |
| color | `"color` | Color ID number |
| reflect | `"reflect` | Reflected light intensity |
| rgb | `"rgb` | List of [red green blue] values |

### Science — Controller

| Port | Mode | Returns |
| --- | --- | --- |
| button | `"button` | `"true` or `"false` |
| button | `"touch` | `"true` or `"false` |
| joystick | `"joystick` | List of [left right] percentages |

### RCX

| Mode | Returns |
| --- | --- |
| `"touch` | `"true` or `"false` |
| `"temperature` | Temperature in Celsius |
| `"light` | Light intensity |
| `"rotation` | Accumulated rotation count |
| `"raw` | Raw sensor value (0–1023) |

### Powered UP / Build HAT

Sensor modes depend on which device is attached to each port. Common modes include:

| Device | Modes |
| --- | --- |
| Color/Distance Sensor | color, distance, light, ambient, rgb |
| Technic Color Sensor | color, light, ambient, rgb, hsv, hsvambient |
| Technic Force Sensor | force, touched, tapped |
| Technic Distance Sensor | distance, fastDistance |
| Tacho Motors | rotation, speed |
| Absolute Motors | rotation, speed, absolute |
| Tilt Sensor | tilt |
| Internal Voltage | voltage |

---

*BrickLogo — by the Open Brick Project*
*A modern LEGO/Logo programming environment*
