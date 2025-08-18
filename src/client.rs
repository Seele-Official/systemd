
use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation as Win32Foundation;
use windows::Win32::System::Pipes as Win32Pipes;

use crate::{PIPE_NAME_WIDE, Cli};

pub fn run(cli: &Cli) -> String {

    let msg = serde_json::to_string(&cli).unwrap();
    let in_buffer = msg.as_bytes();

    let mut response_buffer = [0u8; 1024];
    let mut bytes_read: u32 = 0;

    unsafe {
        let result = Win32Pipes::CallNamedPipeW(
            PCWSTR::from_raw(PIPE_NAME_WIDE.as_ptr()),
            Some(in_buffer.as_ptr() as *const c_void),
            in_buffer.len() as u32,
            Some(response_buffer.as_mut_ptr() as *mut c_void),
            response_buffer.len() as u32,
            &mut bytes_read,
            Win32Pipes::NMPWAIT_USE_DEFAULT_WAIT,
        );


        if result.as_bool() {
            if bytes_read > 0 {
                format!("{}", String::from_utf8_unchecked(response_buffer[..bytes_read as usize].to_vec()))
            } else {
                format!("Connected to the pipe server, but received no response.")
            }
        } else {
            format!(
                "CallNamedPipeW failed. Win32 Error: {:?}",
                Win32Foundation::GetLastError()
            )
        }
    }
}
