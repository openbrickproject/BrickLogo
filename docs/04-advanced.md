# Advanced Usage

## Concurrent tasks

`launch` runs a block of commands in the background. The prompt returns immediately and the launched task runs alongside whatever you do next.

```
? launch [forever [print sensor "distance wait 5]]
? talkto "a
? on
```

The sensor prints every half second while you control the motor from the prompt.

Launched tasks share global variables with the main program and with each other. You can use `make` in one task and read the variable with `:name` in another.

```
? launch [forever [make "d sensor "distance wait 5]]
? waituntil [:d < 10]
? off
```

The first task updates `:d` continuously. The main program waits until the distance is below 10, then stops the motor.

Each task has its own port selection. A `talkto` or `listento` in one task does not affect another.

Use `stopall` to stop all background tasks. Escape stops the current command but does not affect launched tasks that are already running in the background.

## Multiple devices

You can connect several devices at once. Each gets a name when you connect it.

```
? connectto "pup "arm
? connectto "controllab "belt
```

The first device you connect becomes the active device. Switch with `use`:

```
? use "belt
? talkto "a
? on
```

You can also address a port on any device without switching, by qualifying the port name:

```
? talkto "arm.a
? on
```

This turns on port A on the device named `"arm`, regardless of which device is active.

Qualified names work with `listento` as well:

```
? listento "arm.b
? print sensor "distance
```

## Networking

BrickLogo instances on different computers can share variables over a network. One instance runs as the host. Others join it. Any variable set with `make` on one machine is visible on all the others.

### Starting a host

```
bricklogo --host
```

BrickLogo starts normally and listens for connections on port 9750. The status bar shows `[net: hosting (0 clients)]`. The number updates as clients connect and disconnect.

To use a different port:

```
bricklogo --host 5000
```

### Joining a host

```
bricklogo --join 192.168.1.50
```

BrickLogo connects to the host, receives a copy of all current variables, then starts normally. The status bar shows `[net: connected]`.

To join on a non-default port:

```
bricklogo --join 192.168.1.50:5000
```

If the host cannot be reached, BrickLogo prints an error and exits.

### How it works

Variables are the only thing shared. Each machine connects to and controls its own hardware. There are no remote motor commands or sensor reads.

When you run `make "x 42` on one machine, the value appears on every other machine. When you read `:x` on another machine, you get 42. The only variables that are not shared are procedure parameters, which are local to that procedure call.

The host is the source of truth. If two machines write to the same variable at the same time, the last write to reach the host wins. There is no locking.

### Example: sensor station and robot

Machine A has a distance sensor on a Build HAT. Machine B has a Powered UP motor.

Machine A:

```
? connectto "buildhat "sensor
? listento "a
? forever [make "dist sensor "distance wait 5]
```

Machine B:

```
? connectto "pup "robot
? talkto "a
? forever [ifelse :dist < 20 [off] [on] wait 5]
```

Machine A reads the sensor and writes the distance to `:dist`. Machine B reads `:dist` and decides whether to run the motor. Neither machine knows or cares where the variable comes from.

### Disconnection

If the connection to the host is lost, BrickLogo continues running with the last known variable values. The status bar shows `[net: disconnected]`. BrickLogo tries to reconnect every few seconds. When it does, it receives a fresh copy of all variables from the host.

Variables written locally while disconnected are not sent to the host. When the host's snapshot arrives, it replaces all local values.

## Firmware upload

Some devices need firmware uploaded before they can be used.

### RCX

The RCX needs firmware loaded once after batteries are inserted. Use the `firmware` command at the prompt (not inside a program):

```
? firmware "myrcx "firm0332.lgo
```

The firmware file is included in the `firmware/rcx/` directory of the release.

### Build HAT

The Build HAT needs firmware uploaded every time the Pi powers on. BrickLogo does this automatically during `connectto "buildhat`. You do not need to run `firmware` yourself unless you want to load custom firmware.

## See also

- [Tutorial](03-tutorial.md) for a first session.
- [Reference Guide](05-reference.md) for the complete list of commands.
