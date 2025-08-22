use crate::{SERVICE_PIPE_NAME_WIDE, config, process, Cli, Commands, client, pipe};

use std::thread::{self, JoinHandle};

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};


static WORKER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

static STOP_TOKEN: AtomicBool = AtomicBool::new(false);


pub fn stop() {
    STOP_TOKEN.store(true, Ordering::Relaxed);
    log::info!("The closing message `{}`", client::run(&Cli::default()));

    let mut handle_guard = WORKER_THREAD.lock().unwrap();
    if let Some(handle) = handle_guard.take() {
        handle.join().ok();
    }
}

pub fn run() {
    let mut handle_guard = WORKER_THREAD.lock().unwrap();

    if handle_guard.is_none() {
        let handle = thread::spawn(move || {
            server_init();
            while !STOP_TOKEN.load(Ordering::Relaxed) {
                pipe::listen(SERVICE_PIPE_NAME_WIDE, |recv| {
                    handle_pipe(recv).into_bytes()
                }).unwrap_or_else(|e| {
                    log::error!("Error listening on pipe: {:?}", e);
                });
            }
            log::info!("Worker thread stopped.");
        });
        *handle_guard = Some(handle);
    }
}


fn handle_pipe(recv: &[u8]) -> String {

    let msg = unsafe {String::from_utf8_unchecked(recv.to_vec())};

    match serde_json::from_str::<Cli>(&msg) {
        Ok(cli) => {
            match cli.command {
                Some(cmd) => match cmd {
                    Commands::Start { ref name } => {
                        config::get(name, |config| {
                            match process::spawn(name, &config.service) {
                                Ok(()) => format!("Service `{}` started successfully.", name),
                                Err(e) => format!("Failed to start service `{}`: {:?}", name, e)
                            }
                        }).unwrap_or(format!("Cannot Find Service `{}`", name))
                    }
                    Commands::Status { ref name } => (|name| {
                        let cfg = match config::get(name, |config| {
                            config.clone()
                        }) {
                            Some(cfg) => cfg,
                            None => {
                                return format!("Failed to get config for service `{}`", name);
                            }
                        };

                        let mut ret = String::new();

                        ret.push_str(&format!("{} - {}\n\n", name, cfg.unit.description.unwrap_or("Not provided description".to_string())));

                        ret.push_str(&format!("{:<7}:{:?} \n", "Type", cfg.service.style));
                        ret.push_str(&format!("{:<7}:{}", "Status", 
                            match process::check(name) {
                                Ok(_) => format!("Running"),
                                Err(e) => format!("{:?}", e)
                            }
                        ));
                        
                        ret
                    }) (&name),
                    Commands::Stop { ref name } => {
                        match process::stop(name) {
                            Ok(()) => format!("Service `{}` stopped successfully.", name),
                            Err(e) => format!("Failed to stop service `{}`: {:?}", name, e)
                        }
                    },
                    Commands::ReloadConfig => {
                        match config::load() {
                            Ok(()) => format!("Configuration reloaded successfully."),
                            Err(e) => format!("Error reloading configuration: {:?}", e)
                        }
                    }
                    _ => {
                        format!("Unknown command: {:?}", cmd)
                    }
                }
                None => {
                    format!("No command provided.")
                }
            }
        },
        Err(e) => {
            format!("Failed to parse command: {:?}", e)
        }
    }
}

fn server_init() {
    match config::load() {
        Ok(()) => {
            log::info!("Configuration loaded successfully.");
        }
        Err(err) => {
            log::error!("Error loading configuration: {:?}", err);
        }
    }


    config::for_each(|config| {
        let config::Config{unit, service, other} = config;
        match service.style {
            config::ServiceType::Startup => {
                match process::spawn(&unit.name, service) {
                    Ok(()) => log::info!("Service {} started successfully.", &unit.name),
                    Err(e) => log::error!("Failed to start service {}: {:?}", &unit.name, e)
                }
            }
            _ => {
                log::info!("Service {} is of type {:?}", &unit.name, service.style);
            }
        }
    });

}

