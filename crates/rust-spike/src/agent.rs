//! MicroPython agent uploaded to the hub. Runs permanently, accepts
//! binary commands via `hub.config["module_tunnel"]`, and responds with
//! binary replies. See `protocol.rs` for the wire format.

/// Python source for the agent. Uploaded to `/flash/program0/program.py` and
/// started via `ProgramFlowRequest(Start, 0)`.
pub const AGENT_SOURCE: &str = r#"# BrickLogo SPIKE Prime agent (binary protocol).
import hub, motor, device, runloop, color_sensor, distance_sensor, force_sensor
from hub import port, motion_sensor
import struct

_tunnel = hub.config["module_tunnel"]
_cmds = []
_ports = [port.A, port.B, port.C, port.D, port.E, port.F]

# ── Opcodes / reply kinds ───────────────────
OP_MOTOR_RUN = 0x01
OP_MOTOR_STOP = 0x02
OP_MOTOR_RESET = 0x03
OP_MOTOR_RUN_FOR_TIME = 0x04
OP_MOTOR_RUN_FOR_DEGREES = 0x05
OP_MOTOR_RUN_TO_ABS = 0x06
OP_PARALLEL_RUN_FOR_TIME = 0x07
OP_PARALLEL_RUN_FOR_DEGREES = 0x08
OP_PARALLEL_RUN_TO_ABS = 0x09
OP_READ = 0x0A
OP_READ_HUB = 0x0B
OP_PING = 0x0C
OP_PORT_TYPES = 0x0D
OP_PORT_PWM = 0x0E

R_OK = 0x00
R_INT = 0x01
R_LIST = 0x02
R_BOOL = 0x03
R_ERR = 0x04
R_TYPE_LIST = 0x05
R_READY = 0x10
R_HB = 0x11
R_PORT_EVENT = 0x12

MODE_ROTATION = 0x00
MODE_ABSOLUTE = 0x01
MODE_SPEED = 0x02
MODE_COLOR = 0x03
MODE_LIGHT = 0x04
MODE_DISTANCE = 0x05
MODE_FORCE = 0x06
MODE_TOUCHED = 0x07
MODE_TILT = 0x08
MODE_GYRO = 0x09
MODE_ACCEL = 0x0A

def _on_tunnel(data):
    _cmds.append(bytes(data))

_tunnel.callback(_on_tunnel)

def _ok(rid):
    _tunnel.send(struct.pack("<BH", R_OK, rid))

def _int(rid, v):
    _tunnel.send(struct.pack("<BHi", R_INT, rid, v))

def _list(rid, values):
    buf = struct.pack("<BHB", R_LIST, rid, len(values))
    for v in values:
        buf += struct.pack("<i", int(v))
    _tunnel.send(buf)

def _bool(rid, v):
    _tunnel.send(struct.pack("<BHB", R_BOOL, rid, 1 if v else 0))

def _err(rid, msg):
    mb = str(msg).encode("utf-8")[:255]
    _tunnel.send(struct.pack("<BHB", R_ERR, rid, len(mb)) + mb)

# SPIKE 3 firmware exposes the `device` module for per-port introspection
# (LEGO removed the older `port.X.info()` / `port.X.callback()` API in 2023
# but added module-level equivalents). `device.id(port)` returns the LPF2
# type ID; raises when no device is attached (or briefly while one is
# settling after attach — the 1 Hz watcher polls again until it stabilises).

def _query_port_type(idx):
    try:
        return int(device.id(_ports[idx])) & 0xFFFF
    except Exception:
        return 0

def _set_port_pwm(idx, value):
    # OP_PORT_PWM is only dispatched by the host for *non-tacho* devices —
    # tacho motors go through OP_MOTOR_RUN / motor.run() directly. So this
    # path drives lights and passive (non-encoder) motors via the `device`
    # module, which is the universal LPF2 PWM driver. The motor module
    # rejects passive motors with ENODEV — don't use it here.
    #
    # `device.set_duty_cycle` is documented as 0..10000 (unsigned), but
    # firmware versions vary on whether they accept negative values for
    # direction. Try signed first; fall back to abs on rejection so at
    # least forward drive always works.
    p = _ports[idx]
    v = max(-100, min(100, int(value)))
    scaled = v * 100
    try:
        device.set_duty_cycle(p, scaled)
    except Exception:
        device.set_duty_cycle(p, abs(scaled))

def _read_value(p_idx, mode):
    p = _ports[p_idx]
    if mode == MODE_ROTATION: return ("int", motor.relative_position(p))
    if mode == MODE_ABSOLUTE: return ("int", motor.absolute_position(p))
    if mode == MODE_SPEED:    return ("int", motor.velocity(p))
    if mode == MODE_COLOR:    return ("int", color_sensor.color(p))
    if mode == MODE_LIGHT:    return ("int", color_sensor.reflection(p))
    if mode == MODE_DISTANCE: return ("int", distance_sensor.distance(p))
    if mode == MODE_FORCE:    return ("int", force_sensor.force(p))
    if mode == MODE_TOUCHED:  return ("bool", force_sensor.pressed(p))
    raise ValueError("bad mode")

def _read_hub(mode):
    if mode == MODE_TILT:  return list(motion_sensor.tilt_angles())
    if mode == MODE_GYRO:  return list(motion_sensor.angular_velocity())
    if mode == MODE_ACCEL: return list(motion_sensor.acceleration())
    raise ValueError("bad hub mode")

async def _exec(data):
    op = data[0]
    rid = struct.unpack_from("<H", data, 1)[0]
    try:
        if op == OP_MOTOR_RUN:
            p, v = struct.unpack_from("<Bh", data, 3)
            motor.run(_ports[p], v)
            _ok(rid)
        elif op == OP_MOTOR_STOP:
            p = data[3]
            motor.stop(_ports[p])
            _ok(rid)
        elif op == OP_MOTOR_RESET:
            p, off = struct.unpack_from("<Bi", data, 3)
            motor.reset_relative_position(_ports[p], off)
            _ok(rid)
        elif op == OP_MOTOR_RUN_FOR_TIME:
            p, ms, v = struct.unpack_from("<BIh", data, 3)
            await motor.run_for_time(_ports[p], ms, v)
            _ok(rid)
        elif op == OP_MOTOR_RUN_FOR_DEGREES:
            p, deg, v = struct.unpack_from("<Bih", data, 3)
            await motor.run_for_degrees(_ports[p], deg, v)
            _ok(rid)
        elif op == OP_MOTOR_RUN_TO_ABS:
            p, pos, v, d = struct.unpack_from("<BihB", data, 3)
            await motor.run_to_absolute_position(_ports[p], pos, v, direction=d)
            _ok(rid)
        elif op == OP_PARALLEL_RUN_FOR_TIME:
            ms = struct.unpack_from("<I", data, 3)[0]
            n = data[7]
            entries = []
            for i in range(n):
                off = 8 + i * 3
                p, v = struct.unpack_from("<Bh", data, off)
                entries.append((p, v))
            for p, v in entries:
                motor.run(_ports[p], v)
            await runloop.sleep_ms(ms)
            for p, _ in entries:
                motor.stop(_ports[p])
            _ok(rid)
        elif op == OP_PARALLEL_RUN_FOR_DEGREES:
            n = data[3]
            tasks = []
            for i in range(n):
                off = 4 + i * 7
                p, deg, v = struct.unpack_from("<Bih", data, off)
                tasks.append(motor.run_for_degrees(_ports[p], deg, v))
            for t in tasks:
                await t
            _ok(rid)
        elif op == OP_PARALLEL_RUN_TO_ABS:
            n = data[3]
            tasks = []
            for i in range(n):
                off = 4 + i * 8
                p, pos, v, d = struct.unpack_from("<BihB", data, off)
                tasks.append(motor.run_to_absolute_position(_ports[p], pos, v, direction=d))
            for t in tasks:
                await t
            _ok(rid)
        elif op == OP_READ:
            p, mode = struct.unpack_from("<BB", data, 3)
            kind, val = _read_value(p, mode)
            if kind == "int": _int(rid, val)
            else: _bool(rid, val)
        elif op == OP_READ_HUB:
            mode = data[3]
            _list(rid, _read_hub(mode))
        elif op == OP_PING:
            _ok(rid)
        elif op == OP_PORT_PWM:
            p = data[3]
            value = struct.unpack_from("<b", data, 4)[0]
            _set_port_pwm(p, value)
            _ok(rid)
        elif op == OP_PORT_TYPES:
            types = [_query_port_type(i) for i in range(len(_ports))]
            buf = struct.pack("<BHB", R_TYPE_LIST, rid, len(types))
            for t in types:
                buf += struct.pack("<H", t & 0xFFFF)
            _tunnel.send(buf)
        else:
            _err(rid, "unknown op")
    except Exception as e:
        _err(rid, e)

async def _heartbeat():
    while True:
        try:
            _tunnel.send(bytes([R_HB]))
        except Exception:
            pass
        await runloop.sleep_ms(2000)

# Cache of last-seen device type per port. Initialised lazily on first poll
# so we don't fire 6 spurious "attach" events at startup before the host
# has done its initial OP_PORT_TYPES snapshot.
_port_types_cache = [None] * len(_ports)

async def _port_watcher():
    # Poll device.id() for each port at ~1 Hz. Only emit a port_event when
    # the type id actually changes — idle ports cost nothing on the wire.
    while True:
        for i in range(len(_ports)):
            t = _query_port_type(i)
            if _port_types_cache[i] != t:
                _port_types_cache[i] = t
                try:
                    _tunnel.send(struct.pack("<BBH", R_PORT_EVENT, i, t & 0xFFFF))
                except Exception:
                    pass
        await runloop.sleep_ms(1000)

async def _main():
    _tunnel.send(bytes([R_READY]))
    while True:
        while _cmds:
            await _exec(_cmds.pop(0))
        await runloop.sleep_ms(5)

runloop.run(_main(), _heartbeat(), _port_watcher())
"#;

/// CRC32 of the agent source, computed with the 4-byte-padding scheme the
/// hub uses for file transfers.
pub fn agent_crc32() -> u32 {
    crate::atlantis::crc32_padded(AGENT_SOURCE.as_bytes(), 0)
}

#[cfg(test)]
#[path = "tests/agent.rs"]
mod tests;
