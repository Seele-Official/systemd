mod config;
mod process;
mod server;
mod client;
mod pipe;

use std::{
    mem,
    fmt::Display,
    thread::{self, sleep},
    time::{Duration, Instant},
    ffi::{c_void, OsStr, OsString},
};

use clap::{Parser, Subcommand, ArgGroup};
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

#[derive(Parser, Serialize, Deserialize, Debug, Default)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Serialize, Deserialize, Debug, Default, Clone)]
#[command(group(
    ArgGroup::new("service_actions")
        .args([
            "install",
            "uninstall",
            "register",
            "unregister",
            "start_service",
            "stop_service",
            "stop",
            "run_as_service",
            "run_as_user",
        ])
        .multiple(false)
))]
struct ManageOption {
    #[arg(short, long)]
    #[doc = "Install the systemd to the windows service"]
    install: bool,

    #[arg(short, long)]
    #[doc = "Uninstall the systemd from the windows service"]
    uninstall: bool,

    #[arg(long)]
    #[doc = "Register the systemd to registry"]
    register: bool,

    #[arg(long)]
    #[doc = "Unregister the systemd from registry"]
    unregister: bool,

    #[arg(long)]
    #[doc = "Start the installed service systemd"]
    start_service: bool,

    #[arg(long)]
    #[doc = "Stop the installed service systemd"]
    stop_service: bool,

    #[arg(long)]
    #[doc = "Stop systemd"]
    stop: bool,

    #[arg(long)]
    run_as_service: bool,

    #[arg(long)]
    run_as_user: bool,
}


#[derive(Subcommand, Serialize, Deserialize, Debug, Clone)]
enum Commands {
    #[doc = "Manage systemd"]
    Setting(ManageOption),
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

#[derive(Debug)]
enum Error {
    WinService(windows_service::Error),
    Win32(windows::core::Error),
    Io(std::io::Error),
    String(String),
}

impl From<windows_service::Error> for Error {
    fn from(e: windows_service::Error) -> Self {
        Error::WinService(e)
    }
}

impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error::Win32(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::WinService(e) => write!(f, "Windows Service Error: {}", e),
            Error::Win32(e) => write!(f, "Windows API Error: {}", e),
            Error::Io(e) => write!(f, "IO Error: {}", e),
            Error::String(s) => write!(f, "Error: {}", s),
        }
    }
    
}

type Result<T> = std::result::Result<T, Error>;

const APP_NAME: &str = "Systemd";

const SERVICE_MUTEX_NAME_WIDE: PCWSTR = w!(r"Global\__{A7A4E39C-1F27-4A55-9C21-6831614E9461}__");

const SERVICE_PIPE_NAME_WIDE: PCWSTR = w!(r"\\.\pipe\__{A7A4E39C-1F27-4A55-9C21-6831614E9461}__");

const STOPPED_PIPE_NAME_WIDE: PCWSTR = w!(r"\\.\pipe\__{A7A4E39C-1F27-4A55-9C21-6831614E9461}__stopped");

struct MutexGuard {
    handle: Win32Foundation::HANDLE,
    is_holding: bool,
}

impl MutexGuard {
    fn new(attribute: Option<*const Win32Security::SECURITY_ATTRIBUTES>, owner: bool, name: PCWSTR) -> Result<Self> {
        unsafe {
            let mutex_hdl = Win32Threading::CreateMutexW(
                attribute,
                owner,
                name,
            )?;

            if Win32Foundation::GetLastError() == Win32Foundation::ERROR_ALREADY_EXISTS {
                Ok(Self {
                    handle: mutex_hdl,
                    is_holding: false,
                })
            } else {
                Ok(Self {
                    handle: mutex_hdl,
                    is_holding: true,
                })
            }
        }
    }


    fn is_holding(&self) -> bool {
        self.is_holding
    }

}


impl Drop for MutexGuard {
    fn drop(&mut self) {
        unsafe {
            if self.is_holding {
                Win32Threading::ReleaseMutex(self.handle).ok();
            }
            Win32Foundation::CloseHandle(self.handle).ok();
        }
    }
    
}




fn main() {

    let cli = Cli::parse();

    if let Some(command) = cli.command.clone() {
        match command {
            Commands::Setting(opt) => {
                if opt.run_as_service {
                    run_as_service();
                    return;
                } else if opt.run_as_user {
                    run_as_user();
                    return;
                } else if opt.start_service {
                    println!("{:?}", start_service());
                    return;
                } else if opt.stop_service {
                    println!("{:?}", stop_service());
                    return;
                } else if opt.stop {
                    println!("{:?}", stop());
                    return;
                } else if opt.install {
                    println!("{:?}", install());
                    return;
                } else if opt.uninstall {
                    println!("{:?}", uninstall());
                    return;
                }
            },
            _ => {}
        }
    }

    let mutex = 
        MutexGuard::new(None, true, SERVICE_MUTEX_NAME_WIDE)
            .unwrap_or_else(|e| {
                eprintln!("Failed to create mutex: {}", e);
                std::process::exit(1);
            });

    if mutex.is_holding() {
        eprintln!("Service is not running.");
    } else {
        println!("{}", client::run(&cli));
    }
}

fn setup_logger() -> Result<()> {
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
        ).apply().map_err(|e| {
            Error::String(format!("Failed to setup logger: {}", e))
        })?;

    Ok(())
}


fn wait_for_stop_signal() -> Result<()> {
    pipe::listen(STOPPED_PIPE_NAME_WIDE, |recv| {
        let msg = unsafe {String::from_utf8_unchecked(recv.to_vec())};
        if msg == "stop" {
            server::stop();
            return b"Service stopped".to_vec();
        }
        b"Unknown command".to_vec()
    })
}

fn run_as_user() {
    setup_logger().ok();

    let mutex =
        match MutexGuard::new(None, true, SERVICE_MUTEX_NAME_WIDE) {
            Ok(mutex) => mutex,
            Err(e) => {
                log::error!("Failed to create mutex: {:?}", e);
                return;
            }
        };
    if mutex.is_holding() {
        server::run();
        match wait_for_stop_signal() {
            Ok(()) => {
                log::info!("Received stop signal");
            }
            Err(e) => {
                log::error!("Failed to listen on stop signal pipe: {:?}", e);
            }
        }
    } else {
        log::error!("Service is already running.");
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
            return;
        }

        sa.nLength = std::mem::size_of::<Win32Security::SECURITY_ATTRIBUTES>() as u32;
        sa.lpSecurityDescriptor = &mut sd as *mut _ as *mut _;
        sa.bInheritHandle = Win32Foundation::FALSE;

        let mutex = 
            match MutexGuard::new(Some(&sa), true, SERVICE_MUTEX_NAME_WIDE){
                Ok(mutex) => mutex,
                Err(e) => {
                    log::error!("Failed to create mutex: {:?}", e);
                    return;
                }
            };
        
        if mutex.is_holding() {
            service_dispatcher::start(APP_NAME, ffi_service_main)
                .expect("Failed to start service dispatcher");
        } else {
            log::error!("Service is already running.");
        }
    }
}




fn start_service() -> Result<()> {
    ServiceManager::local_computer(
        None::<&str>, 
        ServiceManagerAccess::CONNECT
    )?.open_service(
        APP_NAME, 
        ServiceAccess::START
    )?.start(&[OsStr::new("Started from Rust!")])?;
    Ok(())
}

fn stop_service() -> Result<()> {
    ServiceManager::local_computer(
        None::<&str>, 
        ServiceManagerAccess::CONNECT
    )?.open_service(
        APP_NAME, 
        ServiceAccess::STOP
    )?.stop()?;
    Ok(())
}

fn stop() -> Result<String> {
    let mutex = MutexGuard::new(None, true, SERVICE_MUTEX_NAME_WIDE)?;
    if mutex.is_holding() {
        Err(Error::String("Service is not running.".to_string()))
    } else {
        unsafe {
            Ok(String::from_utf8_unchecked(
                pipe::send(STOPPED_PIPE_NAME_WIDE, b"stop")?
            ))
        }
    }
}

fn install() -> Result<()> {

    let service_info = ServiceInfo {
        name: OsString::from(APP_NAME),
        display_name: OsString::from(APP_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: std::env::current_exe().map_err(
            windows_service::Error::Winapi
        )?,
        launch_arguments: vec![
            OsString::from("setting"),
            OsString::from("--run-as-service")
        ],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };

    ServiceManager::local_computer(
        None::<&str>, 
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE
    )?.create_service(
        &service_info, 
        ServiceAccess::CHANGE_CONFIG
    )?.set_description("A Linux-like service manager")?;

    Ok(())
}

fn uninstall() -> Result<String> {
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

fn register() -> Result<()> {


    Ok(())
}

fn unregister() -> Result<()> {

    Ok(())
}


static STATUS_HDL: OnceCell<ServiceStatusHandle> = OnceCell::new();
define_windows_service!(ffi_service_main, service_main);
fn service_main(arguments: Vec<std::ffi::OsString>) {
    log::info!("Service started with arguments: {:?}", arguments);
    if let Err(e) = run_service() {
        log::error!("Service failed: {}", e);
    }

}
fn run_service() ->  Result<()> {

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
                    }).unwrap_or_else(|e| {
                        log::error!("Failed to set service status: {:?}", e);
                    });
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



    mem::forget(thread::spawn(move || {
        match wait_for_stop_signal() {
            Ok(()) => {
                log::info!("Received stop signal");
            }
            Err(e) => {
                log::error!("Failed to listen on stop signal pipe: {:?}", e);
            }
        }
        if let Some(status_handle) = STATUS_HDL.get() {
            status_handle.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            }).unwrap_or_else(|e| {
                log::error!("Failed to set service status: {:?}", e);
            });
            log::info!("Service stopped successfully.");
        } else {
            log::info!("Service stopped, but status handle is missing.");
        }
    }));


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