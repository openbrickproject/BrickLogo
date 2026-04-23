use bricklogo_hal::adapter::HardwareAdapter;
use bricklogo_hal::adapters::buildhat_adapter::BuildHATAdapter;
use bricklogo_hal::adapters::controllab_adapter::ControlLabAdapter;
use bricklogo_hal::adapters::coral_adapter::CoralAdapter;
use bricklogo_hal::adapters::ev3_adapter::EV3Adapter;
use bricklogo_hal::adapters::nxt_adapter::NxtAdapter;
use bricklogo_hal::adapters::poweredup_adapter::PoweredUpAdapter;
use bricklogo_hal::adapters::rcx_adapter::RcxAdapter;
use bricklogo_hal::adapters::spike_adapter::SpikeAdapter;
use bricklogo_hal::adapters::wedo_adapter::WeDoAdapter;
use bricklogo_hal::port_manager::PortManager;
use bricklogo_lang::error::LogoError;
use bricklogo_lang::evaluator::{Evaluator, PrimitiveSpec};
use bricklogo_lang::value::LogoValue;
use std::sync::Arc;
use std::sync::Mutex;

/// Config for device connections.
#[derive(serde::Deserialize, Default)]
pub struct BrickLogoConfig {
    #[serde(default)]
    pub controllab: Vec<String>,
    #[serde(default)]
    pub wedo: Vec<String>,
    #[serde(default)]
    pub pup: Vec<String>,
    #[serde(default)]
    pub science: Vec<String>,
    #[serde(default)]
    pub rcx: Vec<String>,
    #[serde(default)]
    pub ev3: Vec<String>,
    #[serde(default)]
    pub nxt: Vec<String>,
    #[serde(default)]
    pub spike: Vec<String>,
}

impl BrickLogoConfig {
    pub fn load() -> Self {
        let config_path = std::path::Path::new("bricklogo.config.json");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(config_path) {
                if let Ok(config) = serde_json::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }
}

/// Register all hardware and system primitives into the evaluator.
pub fn register_hardware_primitives(
    eval: &mut Evaluator,
    pm: Arc<Mutex<PortManager>>,
    system_fn: Arc<dyn Fn(&str) + Send + Sync>,
) {
    let config = Arc::new(Mutex::new(BrickLogoConfig::load()));
    let used_indices: Arc<Mutex<std::collections::HashMap<String, usize>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));

    // ── Connection ──────────────────────────────

    let pm_ref = pm.clone();
    let config_ref = config.clone();
    let indices_ref = used_indices.clone();
    let system_fn_ref = system_fn.clone();
    let stop_flag = eval.stop_flag();
    eval.register_primitive(
        "connectto",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(move |args, _, _| {
                let device_type = args[0].as_string().to_lowercase();
                let name = args[1].as_string().to_lowercase();

                // Check if name already exists (brief lock)
                {
                    let pm = pm_ref.lock().unwrap();
                    if pm.get_connected_device_names().contains(&name) {
                        return Err(LogoError::Runtime(format!(
                            "Device \"{}\" already exists",
                            name
                        )));
                    }
                }

                let config = config_ref.lock().unwrap();
                let mut indices = indices_ref.lock().unwrap();

                fn next_config_entry(
                    list: &[String],
                    indices: &mut std::collections::HashMap<String, usize>,
                    key: &str,
                ) -> Option<String> {
                    let idx = indices.get(key).copied().unwrap_or(0);
                    if idx < list.len() {
                        *indices.entry(key.to_string()).or_insert(0) = idx + 1;
                        Some(list[idx].clone())
                    } else {
                        None
                    }
                }

                // Connect outside the port manager lock so the UI can redraw
                let adapter: Box<dyn HardwareAdapter> = match device_type.as_str() {
                    "wedo" => {
                        let identifier = next_config_entry(&config.wedo, &mut indices, "wedo");
                        let mut adapter = WeDoAdapter::new(identifier.as_deref());
                        system_fn_ref("Scanning for LEGO WeDo...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "controllab" => {
                        let serial_path =
                            next_config_entry(&config.controllab, &mut indices, "controllab")
                                .ok_or_else(|| {
                                    LogoError::Runtime(
                                "No Control Lab serial port configured in bricklogo.config.json"
                                    .to_string(),
                            )
                                })?;
                        let mut adapter = ControlLabAdapter::new(&serial_path);
                        system_fn_ref("Scanning for LEGO Control Lab...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "science" => {
                        let mut adapter = CoralAdapter::new();
                        adapter.set_stop_flag(stop_flag.clone());
                        system_fn_ref("Scanning for LEGO Education Science...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "pup" => {
                        let mut adapter = PoweredUpAdapter::new();
                        adapter.set_stop_flag(stop_flag.clone());
                        system_fn_ref("Scanning for Powered UP hub...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "rcx" => {
                        let serial_path = next_config_entry(&config.rcx, &mut indices, "rcx");
                        let mut adapter = RcxAdapter::new(serial_path.as_deref());
                        system_fn_ref("Scanning for LEGO Mindstorms RCX...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "buildhat" => {
                        let mut adapter = BuildHATAdapter::new();
                        system_fn_ref("Connecting to Raspberry Pi Build HAT (this may take up to 30 seconds)...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "ev3" => {
                        let identifier = next_config_entry(&config.ev3, &mut indices, "ev3");
                        let mut adapter = EV3Adapter::new(identifier.as_deref());
                        system_fn_ref("Connecting to LEGO Mindstorms EV3...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "nxt" => {
                        let identifier = next_config_entry(&config.nxt, &mut indices, "nxt");
                        let mut adapter = NxtAdapter::new(identifier.as_deref());
                        system_fn_ref("Connecting to LEGO Mindstorms NXT...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    "spike" => {
                        let serial_path = next_config_entry(&config.spike, &mut indices, "spike");
                        let mut adapter = SpikeAdapter::new(serial_path.as_deref());
                        system_fn_ref("Connecting to LEGO SPIKE Prime...");
                        adapter
                            .connect()
                            .map_err(|e| LogoError::Runtime(format!("Could not connect: {}", e)))?;
                        Box::new(adapter)
                    }
                    _ => {
                        return Err(LogoError::Runtime(
                            "Type must be \"science\", \"pup\", \"wedo\", \"controllab\", \"rcx\", \"buildhat\", \"ev3\", \"nxt\", or \"spike\""
                                .to_string(),
                        ));
                    }
                };

                // Brief lock to register the connected adapter
                let display = adapter.display_name().to_string();
                pm_ref.lock().unwrap().add_device(&name, adapter, &device_type);
                system_fn_ref(&format!("Connected to {} as \"{}\"", display, name));
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "disconnect",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                let mut pm = pm_ref.lock().unwrap();
                let name = pm
                    .get_active_device_name()
                    .map(|s| s.to_string())
                    .ok_or_else(|| LogoError::Runtime("No active device".to_string()))?;
                pm.remove_device(&name);
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "use",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let name = args[0].as_string().to_lowercase();
                pm_ref
                    .lock()
                    .unwrap()
                    .set_active_device(&name)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    // ── Device queries ──────────────────────────

    let pm_ref = pm.clone();
    eval.register_primitive(
        "connected",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                let pm = pm_ref.lock().unwrap();
                let names: Vec<LogoValue> = pm
                    .get_connected_device_names()
                    .into_iter()
                    .map(LogoValue::Word)
                    .collect();
                Ok(Some(LogoValue::List(names)))
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "connected?",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let name = args[0].as_string().to_lowercase();
                let pm = pm_ref.lock().unwrap();
                let result = pm.is_device_connected(&name);
                Ok(Some(LogoValue::Word(
                    if result { "true" } else { "false" }.to_string(),
                )))
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "device",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let name = args[0].as_string().to_lowercase();
                let pm = pm_ref.lock().unwrap();
                let dtype = pm
                    .get_device_type(&name)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(Some(LogoValue::Word(dtype)))
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "outputs",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let name = args[0].as_string().to_lowercase();
                let pm = pm_ref.lock().unwrap();
                let ports = pm
                    .get_device_outputs(&name)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(Some(LogoValue::List(
                    ports.into_iter().map(LogoValue::Word).collect(),
                )))
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "inputs",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let name = args[0].as_string().to_lowercase();
                let pm = pm_ref.lock().unwrap();
                let ports = pm
                    .get_device_inputs(&name)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(Some(LogoValue::List(
                    ports.into_iter().map(LogoValue::Word).collect(),
                )))
            }),
        },
    );

    // ── Motor control ───────────────────────────

    let pm_ref = pm.clone();
    eval.register_primitive(
        "talkto",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, eval| {
                let ports: Vec<String> = match &args[0] {
                    LogoValue::List(l) => l.iter().map(|v| v.as_string().to_lowercase()).collect(),
                    other => vec![other.as_string().to_lowercase()],
                };
                pm_ref.lock().unwrap().ensure_port_states(&ports).map_err(|e| LogoError::Runtime(e))?;
                eval.set_selected_outputs(ports);
                Ok(None)
            }),
        },
    );
    eval.register_alias("tto", "talkto");

    let pm_ref = pm.clone();
    eval.register_primitive("on", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().on(&ports).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("off", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().off(&ports).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("onfor", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let tenths = args[0].as_number()? as u32;
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().on_for(&ports, tenths).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("setpower", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let raw = args[0].as_number()?;
            if !raw.is_finite() || raw < 0.0 || raw > 255.0 {
                return Err(LogoError::Runtime(
                    format!("setpower: power must be a non-negative integer, got {}", raw),
                ));
            }
            let level = raw as u8;
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().set_power(&ports, level)
                .map_err(LogoError::Runtime)?;
            Ok(None)
        }),
    });
    eval.register_alias("sp", "setpower");

    let pm_ref = pm.clone();
    eval.register_primitive("seteven", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().set_even(&ports);
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("setodd", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().set_odd(&ports);
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("setleft", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().set_even(&ports);
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("setright", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().set_odd(&ports);
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("rd", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().reverse_direction(&ports);
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("alloff", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, _| {
            pm_ref.lock().unwrap().all_off();
            Ok(None)
        }),
    });
    eval.register_alias("ao", "alloff");

    let pm_ref = pm.clone();
    eval.register_primitive("rotate", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let degrees = args[0].as_number()? as i32;
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().rotate(&ports, degrees).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("rotateto", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let position = args[0].as_number()? as i32;
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().rotate_to(&ports, position).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("resetzero", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().reset_zero(&ports).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("rotatetoabs", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let position = args[0].as_number()? as i32;
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().rotate_to_abs(&ports, position).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("flash", PrimitiveSpec {
        min_args: 2, max_args: 2,
        func: Arc::new(move |args, _, eval| {
            let on_time = args[0].as_number()? as u32;
            let off_time = args[1].as_number()? as u32;
            let ports = eval.selected_outputs().to_vec();
            pm_ref.lock().unwrap().flash(&ports, on_time, off_time, pm_ref.clone()).map_err(|e| LogoError::Runtime(e))?;
            Ok(None)
        }),
    });

    // ── Sensor primitives ───────────────────────

    eval.register_primitive("listento", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let ports: Vec<String> = match &args[0] {
                LogoValue::List(l) => l.iter().map(|v| v.as_string().to_lowercase()).collect(),
                other => vec![other.as_string().to_lowercase()],
            };
            eval.set_selected_inputs(ports);
            Ok(None)
        }),
    });
    eval.register_alias("lto", "listento");

    let pm_ref = pm.clone();
    eval.register_primitive("sensor", PrimitiveSpec {
        min_args: 1, max_args: 1,
        func: Arc::new(move |args, _, eval| {
            let mode = args[0].as_string().to_lowercase();
            let ports = eval.selected_inputs().to_vec();
            let val = pm_ref.lock().unwrap().read_sensor(&ports, Some(&mode)).map_err(|e| LogoError::Runtime(e))?;
            match val {
                Some(v) => Ok(Some(v)),
                None => Err(LogoError::Runtime("No sensor reading available".to_string())),
            }
        }),
    });

    let pm_ref = pm.clone();
    eval.register_primitive("sensor?", PrimitiveSpec {
        min_args: 0, max_args: 0,
        func: Arc::new(move |_, _, eval| {
            let ports = eval.selected_inputs().to_vec();
            match pm_ref.lock().unwrap().read_sensor(&ports, None) {
                Ok(Some(val)) => {
                    let truthy = match &val {
                        LogoValue::Word(s) if s == "false" => false,
                        LogoValue::Number(n) if *n == 0.0 => false,
                        LogoValue::Word(s) if s.is_empty() => false,
                        _ => true,
                    };
                    Ok(Some(LogoValue::Word(if truthy { "true" } else { "false" }.to_string())))
                }
                _ => Ok(Some(LogoValue::Word("false".to_string()))),
            }
        }),
    });

    // Typed sensor readers
    for (name, mode) in &[
        ("color", "color"),
        ("light", "light"),
        ("force", "force"),
        ("rotation", "rotation"),
        ("tilt", "tilt"),
        ("distance", "distance"),
        ("touch", "touch"),
        ("temperature", "temperature"),
    ] {
        let pm_ref = pm.clone();
        let mode = mode.to_string();
        eval.register_primitive(name, PrimitiveSpec {
            min_args: 0, max_args: 0,
            func: Arc::new(move |_, _, eval| {
                let ports = eval.selected_inputs().to_vec();
                let val = pm_ref.lock().unwrap().read_sensor(&ports, Some(&mode)).map_err(|e| LogoError::Runtime(e))?;
                match val {
                    Some(v) => Ok(Some(v)),
                    None => Err(LogoError::Runtime(format!("No {} reading available", mode))),
                }
            }),
        });
    }
}

#[cfg(test)]
#[path = "tests/bridge.rs"]
mod tests;
