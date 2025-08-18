use once_cell::sync::Lazy;
use core::fmt;
use std::sync::Mutex;
use std::collections::HashMap;
use std::process::{Command, Child};
use std::{fs, io};
use std::result::Result;
use crate::config::Service;

static PROCESS_MAP: Lazy<Mutex<HashMap<String, Child>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
pub enum Error {
    ProcessNotFound,
    ProcessAlreadyRunning,
    ProcessExited(u32),
    IoError(io::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
    
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IoError(e) => write!(f, "IoError({})", e),
            Error::ProcessNotFound => write!(f, "ProcessNotFound"),
            Error::ProcessExited(code) => write!(f, "ProcessExited({})", code),
            Error::ProcessAlreadyRunning => write!(f, "ProcessAlreadyRunning"),
        }
    }
}
pub fn spawn(name: &str, service: &Service) -> Result<(), Error> {
    if check(name).is_ok() {
        return Err(Error::ProcessAlreadyRunning);
    }

    let mut command = Command::new(&service.path);
    if let Some(args) = &service.args {
        command.args(args);
    }
    if let Some(env) = &service.env {
        command.envs(env);
    }

    let log_path = std::env::current_exe()?
        .parent()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Could not find parent directory")
        })?
        .join("log");


    command.stdout(match &service.stdout_path {
        Some(stdout_path) => fs::File::create(stdout_path)?,
        None => fs::File::create(
            log_path.join(format!("{}-stdout.log", name))
        )?
    });

    command.stderr(match &service.stderr_path {
        Some(stderr_path) => fs::File::create(stderr_path)?,
        None => fs::File::create(
            log_path.join(format!("{}-stderr.log", name))
        )?
    });

    PROCESS_MAP.lock().unwrap().insert(name.to_string(), command.spawn()?);
    
    Ok(())
}


pub fn get<F, R>(name: &str, f: F) -> Result<R, Error>
where
    F: FnOnce(&Child) -> R,
{
    let process_map = PROCESS_MAP.lock().unwrap();

    match process_map.get(name) {
        Some(child) => Ok(f(child)),
        None => Err(Error::ProcessNotFound),
    }
}

pub fn get_mut<F, R>(name: &str, f: F) -> Result<R, Error>
where
    F: FnOnce(&mut Child) -> R,
{
    let mut process_map = PROCESS_MAP.lock().unwrap();

    match process_map.get_mut(name) {
        Some(child) => Ok(f(child)),
        None => Err(Error::ProcessNotFound),
    }
}

pub fn check(name: &str) -> Result<(), Error> {
    get_mut(name, |child| {
        match child.try_wait() {
            Ok(Some(status)) => {
                Err(Error::ProcessExited(status.code().unwrap_or(1) as u32))
            }
            Ok(None) => {
                Ok(())
            }
            Err(e) => {
                Err(Error::IoError(e))
            }
        }

    })?
}



pub fn stop(name: &str) -> Result<(), Error> {
    let mut process_map = PROCESS_MAP.lock().unwrap();

    match process_map.remove(name) {
        Some(mut child) => {
            child.kill()?;
            child.wait()?;
            Ok(())
        }
        None => Err(Error::ProcessNotFound),
    }
}