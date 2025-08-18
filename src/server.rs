
use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;

use clap::Parser;
use windows::core::{PCWSTR, w};
use windows::Win32::Foundation as Win32Foundation;
use windows::Win32::Storage::FileSystem as Win32FileSystem;
use windows::Win32::System::Pipes as Win32Pipes;

use crate::{PIPE_NAME_WIDE, config, process, Cli, Commands, client};

use std::thread::{self, JoinHandle};

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};


static WORKER_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

static STOP_TOKEN: AtomicBool = AtomicBool::new(false);


pub fn stop() {
    STOP_TOKEN.store(true, Ordering::Relaxed);
    log::info!("{}", client::run(&Cli::parse()));

    let mut handle_guard = WORKER_THREAD.lock().unwrap();
    if let Some(handle) = handle_guard.take() {
        handle.join().ok();
    }
}

pub fn run() {
    let mut handle_guard = WORKER_THREAD.lock().unwrap();

    if handle_guard.is_none() {
        let handle = thread::spawn(move || {
            while !STOP_TOKEN.load(Ordering::Relaxed) {
                unsafe {

                    let pipe_hdl = Win32Pipes::CreateNamedPipeW(
                        PCWSTR::from_raw(PIPE_NAME_WIDE.as_ptr()),
                        Win32FileSystem::PIPE_ACCESS_DUPLEX,
                        Win32Pipes::PIPE_TYPE_MESSAGE | Win32Pipes::PIPE_READMODE_MESSAGE | Win32Pipes::PIPE_WAIT,
                        Win32Pipes::PIPE_UNLIMITED_INSTANCES,
                        1024,
                        1024,
                        0,
                        None,
                    );

                    if pipe_hdl.is_invalid() {
                        eprintln!("Failed to create named pipe server instance. Error: {:?}", Win32Foundation::GetLastError());
                        break;
                    }

                    // Start listening for incoming connections.
                    Win32Pipes::ConnectNamedPipe(pipe_hdl, None).ok();

                    let mut bytes_written: u32 = 0;

                    Win32FileSystem::WriteFile(
                        pipe_hdl,
                        Some(handle_pipe(pipe_hdl).as_bytes()),
                        Some(&mut bytes_written), 
                        None
                    ).ok();
                    
                    Win32FileSystem::FlushFileBuffers(pipe_hdl).ok();
                    
                    Win32Pipes::DisconnectNamedPipe(pipe_hdl).ok();
                    Win32Foundation::CloseHandle(pipe_hdl).ok();  
                }
            }
            println!("Worker thread stopped.");
        });
        *handle_guard = Some(handle);
    }
}


fn handle_pipe(pipe_hdl: Win32Foundation::HANDLE) -> String {
unsafe {
    let mut buffer = [0u8; 1024];
    let mut bytes_read: u32 = 0;
    if Win32FileSystem::ReadFile(
        pipe_hdl, 
        Some(&mut buffer), 
        Some(&mut bytes_read), 
        None
    ).is_ok()
    && bytes_read > 0
    {
        let msg = String::from_utf8_unchecked(buffer[..bytes_read as usize].to_vec());

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

                            ret.push_str(&format!("{} - {}\n", name, cfg.unit.description.unwrap_or("Not provided".to_string())));

                            ret.push_str(&format!("{:<7}:{:?} \n", "Type", cfg.service.style));
                            ret.push_str(&format!("{:<7}:{} \n", "Status", 
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
    } else {
        format!("Failed to read from pipe or no data received.")
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

