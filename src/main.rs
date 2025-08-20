mod config;
mod process;
mod server;
mod client;


use std::{
    fs,
    thread::sleep,
    time::{Duration, Instant},
    ffi::{c_void, OsStr, OsString},
};

use clap::{Parser, Subcommand};
use serde::{Serialize, Deserialize};
use once_cell::sync::OnceCell;
use windows_service::{define_windows_service, service_dispatcher};

use windows::core::{PCWSTR, w};
use windows::Win32::Foundation as Win32Foundation;
use windows::Win32::System::Threading as Win32Threading;
use windows::Win32::Security as Win32Security;

use windows::Win32::System::Console as Win32Console;

use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle}
};

#[derive(Parser, Serialize, Deserialize, Debug)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    #[doc = "Install the service to the system"]
    install: bool,

    #[arg(short, long)]
    #[doc = "Uninstall the service from the system"]
    uninstall: bool,


    #[arg(long)]
    #[doc = "Start the service"]
    start: bool,

    #[arg(long)]
    #[doc = "Stop the service"]
    stop: bool,

    #[arg(long)]
    run_as_service: bool,

    #[arg(long)]
    run_as_user: bool,
}


#[derive(Subcommand, Serialize, Deserialize, Debug)]
enum Commands {
    #[doc = "Start a service"]
    Start {
        #[arg(index = 1)]
        name: String,
    },

    #[doc = "Stop a service"]
    Stop {
        #[arg(index = 1)]
        name: String,
    },
    #[doc = "Check the status of a service"]
    Status {
        #[arg(index = 1)]
        name: String,
    },
    #[doc = "Reload all service configurations"]
    ReloadConfig,

}




const APP_NAME: &str = "Systemd";



const MUTEX_NAME_WIDE: PCWSTR = w!(r"Global\__{A7A4E39C-1F27-4A55-9C21-6831614E9461}__");

const PIPE_NAME_WIDE: PCWSTR = w!(r"\\.\pipe\__{A7A4E39C-1F27-4A55-9C21-6831614E9461}__");


fn main() {
    let cli = Cli::parse();
    if cli.run_as_service {
        run_as_service();
        return;
    } else if cli.run_as_user {
        run_as_user();
        return;
    } else if cli.start {
        println!("{:?}", start());
        return;
    } else if cli.stop {
        println!("{:?}", stop());
        return;
    } else if cli.install {
        print!("{:?}", install());
        return;
    } else if cli.uninstall {
        print!("{:?}", uninstall());
        return;
    }

    
    unsafe {
        match Win32Threading::CreateMutexW(
            None,
            true,
            PCWSTR::from_raw(MUTEX_NAME_WIDE.as_ptr()),
        ) {
            Ok(mutex_hdl) => {
                if Win32Foundation::GetLastError() == Win32Foundation::ERROR_ALREADY_EXISTS {
                    println!("{}", client::run(&cli));
                } else {
                    eprintln!("Service is not running.");
                    Win32Threading::ReleaseMutex(mutex_hdl).ok();
                }

                Win32Foundation::CloseHandle(mutex_hdl).ok();
            }
            Err(e) => {
                eprintln!("Failed to create mutex: {:?}", e);
            }
        }
    }
}

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}:{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.file().unwrap_or_default(),
                record.line().unwrap_or_default(),
                message
            ))
        })
        .chain(
            fern::log_file(std::env::current_exe()?
                .with_file_name(format!("{}.log", APP_NAME)))?
        ).apply()?;

    Ok(())
}

fn run_as_user() {
    setup_logger().ok();
    unsafe {
        match Win32Threading::CreateMutexW(
            None,
            true,
            PCWSTR::from_raw(MUTEX_NAME_WIDE.as_ptr()),
        ) {
            Ok(mutex_hdl) => {
                if Win32Foundation::GetLastError() == Win32Foundation::ERROR_ALREADY_EXISTS {
                    eprintln!("Service is already running.");
                } else {
                    server::run();

                    loop {
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).unwrap();
                        let input = input.trim();

                        match input {
                            "exit" => {
                                server::stop();
                                break;
                            },
                            _ => eprintln!("Unknown command: {}", input),
                        }
                    }

                    Win32Threading::ReleaseMutex(mutex_hdl).ok();
                }

                Win32Foundation::CloseHandle(mutex_hdl).ok();
            }
            Err(e) => {
                eprintln!("Failed to create mutex: {:?}", e);
            }
        }
    }
}
fn run_as_service(){
    setup_logger().ok();

    unsafe {
        let mut sa = Win32Security::SECURITY_ATTRIBUTES::default();
        let mut sd = Win32Security::SECURITY_DESCRIPTOR::default();
        let p_sd: Win32Security::PSECURITY_DESCRIPTOR = Win32Security::PSECURITY_DESCRIPTOR(&mut sd as *mut _ as *mut c_void);

        if !Win32Security::InitializeSecurityDescriptor(p_sd, 1).is_ok() 
        || !Win32Security::SetSecurityDescriptorDacl(p_sd, true, None, false).is_ok(){
            log::info!("Security descriptor initialized and DACL set.");
        }

        sa.nLength = std::mem::size_of::<Win32Security::SECURITY_ATTRIBUTES>() as u32;
        sa.lpSecurityDescriptor = &mut sd as *mut _ as *mut _;
        sa.bInheritHandle = Win32Foundation::FALSE;



        match Win32Threading::CreateMutexW(
            Some(&sa),
            true,
            PCWSTR::from_raw(MUTEX_NAME_WIDE.as_ptr()),
        ) {
            Ok(mutex_hdl) => {
                if Win32Foundation::GetLastError() == Win32Foundation::ERROR_ALREADY_EXISTS {
                    log::error!("Service is already running.");
                } else {
                    service_dispatcher::start(APP_NAME, ffi_service_main).expect("Failed to start service dispatcher");
                    Win32Threading::ReleaseMutex(mutex_hdl).ok();
                }

                Win32Foundation::CloseHandle(mutex_hdl).ok();
            }
            Err(e) => {
                log::error!("Failed to create mutex: {:?}", e);
            }
        }
    }
}







define_windows_service!(ffi_service_main, service_main);
fn service_main(arguments: Vec<std::ffi::OsString>) {
    log::info!("Service started with arguments: {:?}", arguments);
    if let Err(e) = run_service() {
        log::error!("Service failed: {:?}", e);
    }

}

fn start() -> windows_service::Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service = service_manager.open_service(APP_NAME, ServiceAccess::START)?;

    service.start(&[OsStr::new("Started from Rust!")])?;

    Ok(())
}

fn stop() -> windows_service::Result<()> {

    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service = service_manager.open_service(
        APP_NAME,
        ServiceAccess::STOP,
    )?;

    service.stop()?;

    Ok(())
}

fn install() -> windows_service::Result<()> {

    let service_manager = ServiceManager::local_computer(
        None::<&str>, 
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE
    )?;

    let service_binary_path = 
        std::env::current_exe().map_err(
            windows_service::Error::Winapi
        )?;
    let service_info = ServiceInfo {
        name: OsString::from(APP_NAME),
        display_name: OsString::from(APP_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![OsString::from("--run-as-service")],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };

    let service = service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;

    service.set_description("A Linux-like service manager")?;
    Ok(())
}

fn uninstall() -> windows_service::Result<String> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager.open_service(APP_NAME, service_access)?;

    // The service will be marked for deletion as long as this function call succeeds.
    // However, it will not be deleted from the database until it is stopped and all open handles to it are closed.
    service.delete()?;
    // Our handle to it is not closed yet. So we can still query it.
    if service.query_status()?.current_state != ServiceState::Stopped {
        // If the service cannot be stopped, it will be deleted when the system restarts.
        service.stop()?;
    }
    // Explicitly close our open handle to the service. This is automatically called when `service` goes out of scope.
    drop(service);

    // Win32 API does not give us a way to wait for service deletion.
    // To check if the service is deleted from the database, we have to poll it ourselves.
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        if let Err(windows_service::Error::Winapi(e)) =
            service_manager.open_service(APP_NAME, ServiceAccess::QUERY_STATUS)
        {
            if e.raw_os_error() == Some(Win32Foundation::ERROR_SERVICE_DOES_NOT_EXIST.0 as i32) {
                return Ok(format!("{} is deleted.", APP_NAME));
            }
        }
        sleep(Duration::from_secs(1));
    }
    Ok(format!("{} is marked for deletion.", APP_NAME))
}


static STATUS_HDL: OnceCell<ServiceStatusHandle> = OnceCell::new();

fn run_service() -> windows_service::Result<()> {

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                server::stop();
                if let Some(status_handle) = STATUS_HDL.get() {
                    status_handle.set_service_status(ServiceStatus {
                        service_type: ServiceType::OWN_PROCESS,
                        current_state: ServiceState::Stopped,
                        controls_accepted: ServiceControlAccept::empty(),
                        exit_code: ServiceExitCode::Win32(0),
                        checkpoint: 0,
                        wait_hint: Duration::default(),
                        process_id: None,
                    }).ok();
                    log::info!("Service stopped successfully.");
                } else {
                    log::info!("Service stopped, but status handle is missing.");
                }
                ServiceControlHandlerResult::NoError
            },
            ServiceControl::Interrogate => {
                ServiceControlHandlerResult::NoError
            },
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let hdl = service_control_handler::register(APP_NAME, event_handler)?;

    STATUS_HDL.get_or_init(||{
        hdl
    });

    server::run();

    // Tell the system that the service is running now
    if let Some(status_handle) = STATUS_HDL.get() {
        status_handle.set_service_status(ServiceStatus {
            // Should match the one from system service registry
            service_type: ServiceType::OWN_PROCESS,
            // The new state
            current_state: ServiceState::Running,
            // Accept stop events when running
            controls_accepted: ServiceControlAccept::STOP,
            // Used to report an error when starting or stopping only, otherwise must be zero
            exit_code: ServiceExitCode::Win32(0),
            // Only used for pending states, otherwise must be zero
            checkpoint: 0,
            // Only used for pending states, otherwise must be zero
            wait_hint: Duration::default(),

            process_id: None,
        })?;
    }

    Ok(())
}