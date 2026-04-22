# Reference Guide

This is the complete command reference for BrickLogo. Each command is listed with its syntax, a description, and examples. For a guided introduction, see the [Tutorial](03-tutorial.md).

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
16. [CLI Flags](#16-cli-flags)
- [Appendix A: Supported Devices](#appendix-a-supported-devices)
- [Appendix B: Sensor Modes by Device](#appendix-b-sensor-modes-by-device)

---

## 1. Connecting to Devices

Before you can control motors or read sensors, you must connect to a device. Each connected device is given a name that you choose. That name is how you refer to it for the rest of the session.

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
| `"spike` | LEGO SPIKE Prime / Robot Inventor — via USB or Bluetooth Low Energy |
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

When the active device is disconnected, BrickLogo falls back to the most-recently-used remaining device — whichever one you last `use`d, or the most recently connected if you haven't switched. When the last device is disconnected, there is no active device; the next command that needs one will error until you `connectto` or `use` another.

### `disconnect`

```
disconnect
```

Disconnects the active device. Takes no arguments. To disconnect a specific device, switch to it first with `use`:

```
? use "lab
? disconnect
```

Errors if there is no active device. To disconnect all devices, call `disconnect` once per device.

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

> On devices with encoder motors (Science, Powered UP tacho motors, Build HAT tacho motors, EV3), the device handles the timing internally and reports when it finishes. On devices without encoders (WeDo, Control Lab, RCX, basic Powered UP motors), BrickLogo times the run in software.

### `setpower`

*alias: sp*

```
setpower level
```

Sets the power level for the selected ports. The accepted range is device-native — modern hubs accept 0 to 100, older hubs a smaller range:

| Device | Power range |
|---|---|
| Build HAT | 0–100 |
| Powered UP / WeDo 2.0 | 0–100 |
| WeDo 1.0 | 0–100 |
| Science (Coral) | 0–100 |
| Control Lab | 0–8 |
| RCX | 0–7 |

The default power on a fresh port is half the device's maximum. A value outside the accepted range errors — when several devices are selected, the value must be valid for every one of them, so `setpower 50` works on a Powered UP hub but errors if an RCX is also in the selection.

```
? setpower 100
? on
```

If the motors are already running, the power change takes effect immediately.

### `seteven`

*alias: setleft*

```
seteven
```

Sets the direction of the selected ports to "even" (the default forward direction). If the motors are running, the direction change takes effect immediately.

### `setleft`

```
setleft
```

Same as `seteven`. Sets the direction of the selected ports to "even" (the default forward direction).

### `setodd`

*alias: setright*

```
setodd
```

Sets the direction of the selected ports to "odd" (the reverse direction).

### `setright`

```
setright
```

Same as `setodd`. Sets the direction of the selected ports to "odd" (the reverse direction).

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
rotateto angle
```

Rotates the selected ports to an angular position (0–359) relative to the zero set by `resetzero`. The motor takes the path in the direction set by `seteven`/`setodd`, always within one revolution. The command does not return until the motor reaches the position. Requires encoder motors.

```
? resetzero
? rotateto 90
? rotateto 0
```

`rotateto 90` turns 90 degrees from zero. `rotateto 0` returns to zero. The direction controls which way the motor turns — if the target is "behind" the current angle in the set direction, the motor wraps the long way around rather than reversing.

### `resetzero`

```
resetzero
```

Resets the encoder zero point on the selected ports. After this command, the current physical position of the motor is considered angle 0 for `rotateto`.

### `rotatetoabs`

```
rotatetoabs angle
```

Rotates the selected ports to an absolute angular position on the motor's physical encoder. The command does not return until the motors reach the position. Requires motors with an absolute-position encoder (Technic angular motors). Non-absolute tacho motors (Boost motors, train motors, EV3 motors) do not support this command — use `rotateto` after `resetzero` instead.

```
? rotatetoabs 0
```

Returns to the motor's physical zero (home).

```
? rotatetoabs 90
```

Rotates to 90 degrees on the absolute encoder.

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

Shorthand for reading the selected sensor in light mode. Returns the reflected light intensity as a number.

### `force`

```
force
```

Shorthand for reading the selected sensor in force mode. Returns the force value as a number.

### `rotation`

```
rotation
```

Shorthand for reading the selected sensor in rotation mode. Returns the motor position in degrees as a number.

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

BrickLogo supports `+`, `-`, `*`, `/`, `=`, `<`, `>`, `<=`, `>=`, and `<>` as infix operators:

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

### `foreach`

```
foreach "var list [commands]
```

Runs the commands once for each item in the list. The variable is set to the current item on each iteration.

```
? foreach "x [1 2 3] [print :x]
1
2
3
```

If a word is given instead of a list, iterates over its characters.

```
? foreach "p [a b c] [talkto :p on]
```

### `while`

```
while [condition] [commands]
```

Runs the commands repeatedly as long as the condition is `"true`. The condition is evaluated before each iteration.

```
? make "n 5
? while [:n > 0] [print :n make "n :n - 1]
5
4
3
2
1
```

### `until`

```
until [condition] [commands]
```

Runs the commands repeatedly until the condition becomes `"true`. The condition is evaluated before each iteration.

```
? make "n 1
? until [:n > 3] [print :n make "n :n + 1]
1
2
3
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

### `launch`

```
launch [commands]
```

Runs the commands in a background thread. Execution continues immediately — `launch` does not wait for the commands to finish. Useful for running independent motor sequences or sensor monitors in parallel.

```
? launch [forever [onfor 10 wait 5]]
? print "launched
launched
```

Background threads share variables with the main program. Press Escape to stop the foreground program; use `stopall` to stop all background threads.

### `stopall`

```
stopall
```

Stops all background threads started with `launch`.

```
? stopall
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

### `local`

```
local "name
```

Creates a local variable in the current procedure's scope. The variable starts empty and does not affect any global variable with the same name. Must be used inside a procedure.

```
? to test
> local "x
> make "x 42
> print :x
> end

? test
42
? print :x
I don't know about "x"
```

### `localmake`

```
localmake "name value
```

Creates a local variable and sets its value in one step. Equivalent to `local "name` followed by `make "name value`. The variable does not affect any global variable with the same name.

```
? to double :n
> localmake "result :n * 2
> output :result
> end

? print double 5
10
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

Returns the remainder when *a* is divided by *b*. The sign of the result follows the dividend.

```
? print remainder 10 3
1
```

### `modulo`

```
modulo a b
```

Returns *a* modulo *b*. Unlike `remainder`, the sign of the result follows the divisor. Useful for wrapping angles into a positive range.

```
? print modulo 10 3
1
? print modulo (minus 10) 3
2
```

### `power`

```
power base exponent
```

Returns *base* raised to the *exponent*.

```
? print power 2 10
1024
? print power 9 0.5
3
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

Truncates a number toward zero, removing the fractional part.

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

### `uppercase`

```
uppercase word
```

Returns the word converted to uppercase.

```
? print uppercase "hello
HELLO
```

### `lowercase`

```
lowercase word
```

Returns the word converted to lowercase.

```
? print lowercase "HELLO
hello
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

### SPIKE Prime

Connects via USB or Bluetooth Low Energy. No configuration needed. `connectto "spike` uses USB if a hub is attached, otherwise scans for a hub over BLE. The hub must be running Hub OS 3.0 or later; update it through the LEGO Education SPIKE app.

### Build HAT

The Build HAT requires no configuration. It always uses `/dev/serial0` on the Raspberry Pi. See [NOTES.md](../NOTES.md) for Raspberry Pi setup instructions.

---

## 16. CLI Flags

BrickLogo runs as an interactive REPL by default. Passing a script path or `-` runs the given script and exits; flags can appear in any order.

| Invocation | Behavior |
| --- | --- |
| `bricklogo` | Interactive REPL |
| `bricklogo path/to/script.logo` | Run the script and exit |
| `bricklogo -` | Read a script from stdin and exit |

| Flag | Description |
| --- | --- |
| `--host` | Start as a network host on port 9750 |
| `--host <port>` | Start as a network host on a custom port |
| `--join <addr>` | Join a network host at the given address |
| `--join <addr:port>` | Join a network host at a custom port |
| `--password <value>` | Require a password for network connections |

Flags work in either mode — e.g. `bricklogo --host script.logo` runs a script while also hosting a network session.

See [Advanced Usage](04-advanced.md) for details on script mode, networking, passwords, and browser clients.

---

## Appendix A: Supported Devices

### LEGO Education Science (Coral)

Connects via Bluetooth Low Energy. Use `connectto "science`.

| Device | Output Ports | Sensor Ports |
| --- | --- | --- |
| Double Motor | a, b | tilt, gyro, accel, yaw |
| Single Motor | a | tilt, gyro, accel, yaw |
| Color Sensor | — | color, light, rgb |
| Controller | — | button, joystick |

### LEGO Powered UP

Connects via Bluetooth Low Energy. Use `connectto "pup`. Covers every hub in the LEGO Powered UP / LWP3 family plus the older LWP 1.x WeDo 2.0 Smart Hub.

| Hub | Output Ports |
| --- | --- |
| Boost Move Hub | a, b, c, d |
| Powered UP Hub | a, b |
| Technic Medium Hub (Control+) | a, b, c, d |
| Technic Small Hub (SPIKE Essential) | a, b |
| Powered UP Remote | a, b |
| Duplo Train Base | a |
| WeDo 2.0 Smart Hub | a, b |

Powered UP hubs also expose internal sensors (tilt, voltage, temperature, etc.) as named sensor ports. The LED Light (Powered UP accessory) is supported on any output port — `setpower` sets brightness 0–100.

Multiple Powered UP hubs can be connected at once — BrickLogo skips any hub already claimed by another `connectto` and picks the next one during the scan.

**WeDo 2.0 Smart Hub:** tacho-motor operations (`rotate`, `rotateto`, `rotatetoabs`, `resetzero`) are rejected with a clear error, since the WeDo 2.0 hub firmware does not implement position feedback. Basic motor control (`on`, `off`, `setpower`, `onfor`, `setodd`/`seteven`) and sensors work normally.

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

### LEGO Mindstorms EV3

Connects via USB HID (default, no setup required) or Bluetooth SPP (pair the brick at the OS level, add the serial port path to `bricklogo.config.json`). Wi-Fi is planned but not yet implemented. Use `connectto "ev3`.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d | 1, 2, 3, 4 |

`connectto "ev3 "name` tries USB first; if no USB EV3 is found, falls back to the next unconsumed serial path from `bricklogo.config.json`. The config array accepts:

- a bare serial path → Bluetooth SPP
- `"usb"` → force USB HID, first unclaimed brick
- `"usb:<path>"` → USB HID at a specific HID path (for multi-EV3 setups)
- `"wifi:discover"` or `"wifi:<ip>"` → future Wi-Fi (currently errors)

Example:

```json
{ "ev3": ["/dev/cu.EV3-SerialPort-14"] }
```

**Limitations on EV3:**

- `rotateto` works on EV3 — it rotates to the given angle relative to the last `resetzero`, same as every other adapter with encoder motors.
- `rotatetoabs` errors out — EV3 motors have no absolute-position encoder. Use `rotateto 0` after `resetzero` instead.
- Motor control uses raw PWM power (matching every other BrickLogo adapter). Motors load-droop under heavy loads — this is intentional and consistent across devices.
- Firmware upload and file transfer are not supported.
- Daisy-chained bricks are not supported (layer is always 0).

### LEGO SPIKE Prime / Robot Inventor

Connects via USB or Bluetooth Low Energy. Use `connectto "spike`. Supports the same LPF2 motors and sensors as the Powered UP family and Build HAT.

Requires Hub OS 3.0 or later. Update the hub through the LEGO Education SPIKE app.

| Output Ports | Input Ports |
| --- | --- |
| a, b, c, d, e, f | a, b, c, d, e, f |

Any port can host a motor or sensor. The hub also has a built-in IMU accessible as sensor ports `tilt`, `gyro`, and `accel`.

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

Note: WeDo 1.0 sensors are auto-detected — distance and tilt sensors can be plugged into either port.

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
| light | `"light` | Reflected light intensity |
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

### EV3

Sensor modes depend on the sensor type plugged into the port. Both EV3 and NXT sensors are supported.

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

### SPIKE Prime / Robot Inventor

Sensor modes depend on which device is attached to each port.

| Device | Modes |
| --- | --- |
| Color Sensor | color, light |
| Distance Sensor | distance |
| Force Sensor | force, touched |
| Tacho Motors | rotation, speed |
| Absolute Motors (Technic) | rotation, speed, absolute |
| Hub IMU (tilt) | tilt |
| Hub IMU (gyro) | gyro |
| Hub IMU (accel) | accel |

### Powered UP / Build HAT

Sensor modes depend on which device is attached to each port. Common modes include:

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

Build HAT motors additionally support a `speed` mode via combined sensor data.

