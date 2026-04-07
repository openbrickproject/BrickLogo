use bricklogo_hal::adapter::HardwareAdapter;
use bricklogo_hal::adapters::controllab_adapter::ControlLabAdapter;
use bricklogo_hal::adapters::coral_adapter::CoralAdapter;
use bricklogo_hal::adapters::poweredup_adapter::PoweredUpAdapter;
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
        "connect",
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
                        let serial_path = next_config_entry(
                            &config.controllab,
                            &mut indices,
                            "controllab",
                        )
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
                    _ => {
                        return Err(LogoError::Runtime(
                            "Type must be \"science\", \"pup\", \"wedo\", or \"controllab\""
                                .to_string(),
                        ));
                    }
                };

                // Brief lock to register the connected adapter
                let display = adapter.display_name().to_string();
                pm_ref.lock().unwrap().add_device(&name, adapter);
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
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let mut pm = pm_ref.lock().unwrap();
                if let Some(name_val) = args.first() {
                    let name = name_val.as_string().to_lowercase();
                    if name == "all" {
                        pm.remove_all();
                    } else {
                        pm.remove_device(&name);
                    }
                } else {
                    if let Some(name) = pm.get_active_device_name().map(|s| s.to_string()) {
                        pm.remove_device(&name);
                    }
                }
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

    // ── Motor control ───────────────────────────

    let pm_ref = pm.clone();
    eval.register_primitive(
        "talkto",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let mut pm = pm_ref.lock().unwrap();
                let ports = match &args[0] {
                    LogoValue::List(l) => l.iter().map(|v| v.as_string().to_lowercase()).collect(),
                    other => vec![other.as_string().to_lowercase()],
                };
                pm.talk_to(&ports).map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );
    eval.register_alias("tto", "talkto");

    let pm_ref = pm.clone();
    eval.register_primitive(
        "on",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref
                    .lock()
                    .unwrap()
                    .on()
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "off",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref
                    .lock()
                    .unwrap()
                    .off()
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "onfor",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let tenths = args[0].as_number()? as u32;
                pm_ref
                    .lock()
                    .unwrap()
                    .on_for(tenths)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "setpower",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let level = args[0].as_number()? as u8;
                pm_ref.lock().unwrap().set_power(level);
                Ok(None)
            }),
        },
    );
    eval.register_alias("sp", "setpower");

    let pm_ref = pm.clone();
    eval.register_primitive(
        "seteven",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref.lock().unwrap().set_even();
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "setodd",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref.lock().unwrap().set_odd();
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "setleft",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref.lock().unwrap().set_even();
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "setright",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref.lock().unwrap().set_odd();
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "rd",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref.lock().unwrap().reverse_direction();
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "alloff",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref.lock().unwrap().all_off();
                Ok(None)
            }),
        },
    );
    eval.register_alias("ao", "alloff");

    let pm_ref = pm.clone();
    eval.register_primitive(
        "rotate",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let degrees = args[0].as_number()? as i32;
                pm_ref
                    .lock()
                    .unwrap()
                    .rotate(degrees)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "rotateto",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let position = args[0].as_number()? as i32;
                pm_ref
                    .lock()
                    .unwrap()
                    .rotate_to(position)
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "resetzero",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref
                    .lock()
                    .unwrap()
                    .reset_zero()
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "rotatetohome",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(move |_, _, _| {
                pm_ref
                    .lock()
                    .unwrap()
                    .rotate_to_home()
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "flash",
        PrimitiveSpec {
            min_args: 2,
            max_args: 2,
            func: Arc::new(move |args, _, _| {
                let on_time = args[0].as_number()? as u32;
                let off_time = args[1].as_number()? as u32;
                pm_ref
                    .lock()
                    .unwrap()
                    .flash(on_time, off_time, pm_ref.clone())
                    .map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );

    // ── Sensor primitives ───────────────────────

    let pm_ref = pm.clone();
    eval.register_primitive(
        "listento",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let mut pm = pm_ref.lock().unwrap();
                let ports = match &args[0] {
                    LogoValue::List(l) => l.iter().map(|v| v.as_string().to_lowercase()).collect(),
                    other => vec![other.as_string().to_lowercase()],
                };
                pm.listen_to(&ports).map_err(|e| LogoError::Runtime(e))?;
                Ok(None)
            }),
        },
    );
    eval.register_alias("lto", "listento");

    let pm_ref = pm.clone();
    eval.register_primitive(
        "sensor",
        PrimitiveSpec {
            min_args: 1,
            max_args: 1,
            func: Arc::new(move |args, _, _| {
                let mode = args[0].as_string().to_lowercase();
                let val = pm_ref
                    .lock()
                    .unwrap()
                    .read_sensor(Some(&mode))
                    .map_err(|e| LogoError::Runtime(e))?;
                match val {
                    Some(v) => Ok(Some(v)),
                    None => Err(LogoError::Runtime(
                        "No sensor reading available".to_string(),
                    )),
                }
            }),
        },
    );

    let pm_ref = pm.clone();
    eval.register_primitive(
        "sensor?",
        PrimitiveSpec {
            min_args: 0,
            max_args: 0,
            func: Arc::new(
                move |_, _, _| match pm_ref.lock().unwrap().read_sensor(None) {
                    Ok(Some(val)) => {
                        let truthy = match &val {
                            LogoValue::Word(s) if s == "false" => false,
                            LogoValue::Number(n) if *n == 0.0 => false,
                            LogoValue::Word(s) if s.is_empty() => false,
                            _ => true,
                        };
                        Ok(Some(LogoValue::Word(
                            if truthy { "true" } else { "false" }.to_string(),
                        )))
                    }
                    _ => Ok(Some(LogoValue::Word("false".to_string()))),
                },
            ),
        },
    );

    // Typed sensor readers
    for (name, mode) in &[
        ("color", "color"),
        ("light", "reflect"),
        ("force", "force"),
        ("angle", "rotate"),
    ] {
        let pm_ref = pm.clone();
        let mode = mode.to_string();
        eval.register_primitive(
            name,
            PrimitiveSpec {
                min_args: 0,
                max_args: 0,
                func: Arc::new(move |_, _, _| {
                    let val = pm_ref
                        .lock()
                        .unwrap()
                        .read_sensor(Some(&mode))
                        .map_err(|e| LogoError::Runtime(e))?;
                    match val {
                        Some(v) => Ok(Some(v)),
                        None => Err(LogoError::Runtime(format!("No {} reading available", mode))),
                    }
                }),
            },
        );
    }
}
