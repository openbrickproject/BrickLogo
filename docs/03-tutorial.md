# Tutorial

This tutorial walks through a first session with BrickLogo. You will connect to a hub, turn a motor, and write a procedure that makes it go forward and back.

You need BrickLogo running on your computer (see [Getting Started](02-getting-started.md)) and a LEGO hub with a motor on port A.

The examples below use a Powered UP hub. If you are using a different device, the only line that changes is `connectto`. Use `connectto "science "mybot` for a Science motor, `connectto "controllab "mybot` for a Control Lab, or `connectto "buildhat "mybot` for a Build HAT. This works as long as there is a port A with a motor attached.

## Connecting

Turn on your hub. At the prompt, type:

```
? connectto "pup "mybot
Scanning for Powered UP hub...
Connected to Powered UP Hub as "mybot"
```

The name `"mybot` is your choice. You will use it to refer to this device for the rest of the session.

## Turning the motor on and off

Tell BrickLogo which port the motor is on:

```
? talkto "a
```

Turn the motor on:

```
? on
```

The motor turns at the default power (4 out of 8) in the default direction (even). Turn it off:

```
? off
```

## Power

Set the power before (or while) the motor runs:

```
? setpower 8
? on
```

Full power. Now change it while the motor is still running:

```
? setpower 2
```

The motor slows down. Power levels go from 0 (stopped) to 8 (full).

```
? off
```

## Running for a set time

`onfor` turns the motor on for a number of tenths of a second, then stops it.

```
? onfor 10
```

One second. The prompt comes back when the motor stops.

```
? onfor 50
```

Five seconds.

## Direction

While the motor is running:

```
? on
? rd
```

`rd` reverses the direction. Type it again to reverse back.

```
? rd
? off
```

You can also set direction explicitly. `seteven` is forward. `setodd` is reverse.

## Forward and back

This line makes the motor go forward for one second, pause for one second, reverse for one second, and pause again:

```
? seteven onfor 10 wait 10 setodd onfor 10 wait 10
```

It runs once. To repeat it:

```
? repeat 5 [seteven onfor 10 wait 10 setodd onfor 10 wait 10]
```

Five times. To run it until you press Escape:

```
? forever [seteven onfor 10 wait 10 setodd onfor 10 wait 10]
```

## Procedures

Give that sequence a name so you do not have to type it each time:

```
? to backforth
> seteven onfor 10 wait 10
> setodd onfor 10 wait 10
> end
```

The prompt changes to `>` while you are inside the definition. Type `end` to finish.

Now use it:

```
? backforth
```

```
? repeat 10 [backforth]
```

```
? forever [backforth]
```

## Parameters

Add a parameter to control the duration:

```
? to backforth :time
> seteven onfor :time wait 10
> setodd onfor :time wait 10
> end

? backforth 10
? backforth 20
? backforth 5
```

## Saving

Name your page and save:

```
? namepage "motors
? save
Saved motors.logo
```

Next time, load it:

```
? load "motors
Loaded motors.logo
? backforth 10
```

## Disconnecting

```
? disconnect
```

Or type `bye` to quit. The device disconnects automatically.

## Going further

Plug a second motor into port B. Select both ports with `talkto [a b]` and try the same commands. Both motors run together.

Plug in a sensor. Use `listento` to select it and `sensor` to read it. Use `waituntil` to make your program react to what the sensor sees.

The [Reference Guide](05-reference.md) describes every command.
