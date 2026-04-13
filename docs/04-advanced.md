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

## Running scripts

BrickLogo can run a `.logo` file as a script instead of an interactive session:

```
bricklogo path/to/script.logo
```

The script runs to completion, then BrickLogo exits. All hardware is disconnected cleanly on the way out — motors stop, BLE peripherals release — whether the script finishes normally, errors, or is interrupted with Ctrl+C.

A script is ordinary BrickLogo code. It may contain procedure definitions, `connectto` calls, loops, or anything else you would type at the prompt:

```
connectto "pup "hub
talkto "a
setpower 80
onfor 30
disconnect
```

### Reading from standard input

Use `-` as the path to read the script from stdin:

```
echo 'print 1 + 2' | bricklogo -
```

Useful for piping small snippets from a shell.

### Shebang scripts

Add a shebang line and mark the file executable, and the script runs directly:

```
#!/usr/bin/env bricklogo
connectto "pup "hub
talkto "a
onfor 20
```

```
$ chmod +x spin.logo
$ ./spin.logo
```

BrickLogo strips the leading `#!` line and any UTF-8 BOM before evaluating.

### Output

`print`, `show`, and `type` write to stdout. System messages (port attach/detach, connect confirmations) write to stderr. Errors write to stderr. So a script can be cleanly piped:

```
bricklogo lights.logo 2>/dev/null | grep OK
```

Exit status is `0` on success, `1` on any uncaught error, `130` if interrupted by Ctrl+C.

### Networking in scripts

`--host` and `--join` work in script mode just as in REPL mode:

```
bricklogo --host controller.logo
bricklogo --join 192.168.1.50 sensor.logo
```

The host runs the script while accepting network clients; the client runs the script while reading shared variables from the host. Handy for classroom demos where one machine orchestrates and another reads.

### `load` and file paths

When a script is run from a file, `load` resolves relative to the script's directory, so helper procedures can sit next to the main script:

```
project/
├── main.logo
└── helpers.logo
```

`main.logo` can `load "helpers` and the correct file is found regardless of where BrickLogo was invoked from.

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

### Password

The host can require a password. All clients must provide the same password to connect.

```
bricklogo --host --password secret123
bricklogo --join 192.168.1.50 --password secret123
```

If a password is set, clients that do not authenticate within 5 seconds are disconnected. Clients that send the wrong password are disconnected immediately.

Note, the connection is inherently unsecure, with all data being sent in plaintext.

### How it works

Global variables are the only thing shared. Each machine connects to and controls its own hardware. There are no remote motor commands or sensor reads.

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

### Browser clients

The host is a WebSocket server. BrickLogo instances connect over WebSocket and use a compact binary protocol. Browser clients can also connect and exchange variables using JSON.

A browser can connect at `ws://host:9750` using the standard WebSocket API. An example web client is included in `examples/webclient/index.html`. Open it in a browser, enter the host address and optional password, and connect. It shows all shared variables in real time and lets you set variables from the browser.

This makes it possible to build custom dashboards, control panels, or visualisations that interact with BrickLogo programs.

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
