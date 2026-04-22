//! MicroPython agent uploaded to the hub. Runs permanently, accepts
//! newline-delimited JSON commands via `hub.config["module_tunnel"]`, and
//! responds with newline-delimited JSON.
//!
//! `module_tunnel` is an undocumented-but-stable SPIKE Prime 3.x firmware API
//! that exposes the host-program bidirectional Atlantis TunnelMessage (id
//! 0x32) channel to MicroPython. Confirmed by the LEGO web app's own
//! experiment code (see `learning-and-dynamics/ml-with-bricks`).

/// Python source for the agent. Uploaded to `/flash/program0/program.py` and
/// started via `ProgramFlowRequest(Start, 0)`.
pub const AGENT_SOURCE: &str = r#"# BrickLogo SPIKE Prime agent.
import hub, motor, runloop, color_sensor, distance_sensor, force_sensor
from hub import port, motion_sensor
import json

_tunnel = hub.config["module_tunnel"]
_rx_buf = bytearray()
_cmds = []
_port_map = {"a": port.A, "b": port.B, "c": port.C, "d": port.D, "e": port.E, "f": port.F}

def _on_tunnel(data):
    global _rx_buf
    raw = bytes(data)
    print("rx:", len(raw), raw[:48])
    _rx_buf += raw
    while True:
        i = _rx_buf.find(b"\n")
        if i < 0:
            break
        line = bytes(_rx_buf[:i])
        _rx_buf = _rx_buf[i + 1:]
        try:
            _cmds.append(json.loads(line))
        except ValueError as e:
            print("parse err:", e, "line:", line[:48])

_tunnel.callback(_on_tunnel)

def _reply(obj):
    _tunnel.send((json.dumps(obj) + "\n").encode())

def _p(s):
    return _port_map[s.lower()]

async def _exec(cmd):
    op = cmd.get("op")
    cid = cmd.get("id")
    try:
        if op == "motor_run":
            motor.run(_p(cmd["port"]), cmd["velocity"])
            _reply({"id": cid, "ok": True})
        elif op == "motor_stop":
            motor.stop(_p(cmd["port"]))
            _reply({"id": cid, "ok": True})
        elif op == "motor_reset":
            motor.reset_relative_position(_p(cmd["port"]), cmd.get("offset", 0))
            _reply({"id": cid, "ok": True})
        elif op == "motor_run_for_time":
            await motor.run_for_time(_p(cmd["port"]), cmd["ms"], cmd["velocity"])
            _reply({"id": cid, "ok": True})
        elif op == "motor_run_for_degrees":
            await motor.run_for_degrees(_p(cmd["port"]), cmd["degrees"], cmd["velocity"])
            _reply({"id": cid, "ok": True})
        elif op == "motor_run_to_abs":
            await motor.run_to_absolute_position(
                _p(cmd["port"]), cmd["position"], cmd["velocity"], direction=cmd["direction"])
            _reply({"id": cid, "ok": True})
        elif op == "parallel_run_for_time":
            ms = cmd["ms"]
            for e in cmd["entries"]:
                motor.run(_p(e["port"]), e["velocity"])
            await runloop.sleep_ms(ms)
            for e in cmd["entries"]:
                motor.stop(_p(e["port"]))
            _reply({"id": cid, "ok": True})
        elif op == "parallel_run_for_degrees":
            tasks = [motor.run_for_degrees(_p(e["port"]), e["degrees"], e["velocity"]) for e in cmd["entries"]]
            for t in tasks:
                await t
            _reply({"id": cid, "ok": True})
        elif op == "parallel_run_to_abs":
            tasks = [motor.run_to_absolute_position(_p(e["port"]), e["position"], e["velocity"], direction=e["direction"]) for e in cmd["entries"]]
            for t in tasks:
                await t
            _reply({"id": cid, "ok": True})
        elif op == "read":
            mode = cmd["mode"]
            p = cmd.get("port")
            if mode == "rotation":   v = motor.relative_position(_p(p))
            elif mode == "absolute": v = motor.absolute_position(_p(p))
            elif mode == "speed":    v = motor.velocity(_p(p))
            elif mode == "color":    v = color_sensor.color(_p(p))
            elif mode == "light":    v = color_sensor.reflection(_p(p))
            elif mode == "distance": v = distance_sensor.distance(_p(p))
            elif mode == "force":    v = force_sensor.force(_p(p))
            elif mode == "touched":  v = force_sensor.pressed(_p(p))
            elif mode == "tilt":     v = list(motion_sensor.tilt_angles())
            elif mode == "gyro":     v = list(motion_sensor.angular_velocity())
            elif mode == "accel":    v = list(motion_sensor.acceleration())
            else: raise ValueError("unknown mode: " + str(mode))
            _reply({"id": cid, "value": v})
        elif op == "ping":
            _reply({"id": cid, "ok": True})
        else:
            _reply({"id": cid, "error": "unknown op: " + str(op)})
    except Exception as e:
        _reply({"id": cid, "error": str(e)})

async def _heartbeat():
    while True:
        try:
            _reply({"op": "heartbeat"})
        except Exception:
            pass
        await runloop.sleep_ms(2000)

async def _main():
    _reply({"op": "ready"})
    while True:
        while _cmds:
            cmd = _cmds.pop(0)
            await _exec(cmd)
        await runloop.sleep_ms(10)

runloop.run(_main(), _heartbeat())
"#;

/// CRC32 of the agent source, computed with the 4-byte-padding scheme the
/// hub uses for file transfers.
pub fn agent_crc32() -> u32 {
    crate::atlantis::crc32_padded(AGENT_SOURCE.as_bytes(), 0)
}

#[cfg(test)]
#[path = "tests/agent.rs"]
mod tests;
