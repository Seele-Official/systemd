use std::ffi::c_void;
use windows::core::PCWSTR;
use windows::Win32::Foundation as Win32Foundation;
use windows::Win32::Storage::FileSystem as Win32FileSystem;
use windows::Win32::System::Pipes as Win32Pipes;

use crate::{Result, Error};


pub fn listen<F>(pipe_name: PCWSTR, handler: F) -> Result<()>
where
    F: Fn(&[u8]) -> Vec<u8>
{
unsafe {
    let pipe_hdl = Win32Pipes::CreateNamedPipeW(
        pipe_name,
        Win32FileSystem::PIPE_ACCESS_DUPLEX,
        Win32Pipes::PIPE_TYPE_MESSAGE | Win32Pipes::PIPE_READMODE_MESSAGE | Win32Pipes::PIPE_WAIT,
        Win32Pipes::PIPE_UNLIMITED_INSTANCES,
        1024,
        1024,
        0,
        None,
    );

    if pipe_hdl.is_invalid() {
        return Err(Error::Win32(windows::core::Error::from_win32()));
    }

    // Start listening for incoming connections.
    Win32Pipes::ConnectNamedPipe(pipe_hdl, None)?;

    let mut read_buffer = [0u8; 1024];
    let mut bytes_read: u32 = 0;
    
    Win32FileSystem::ReadFile(
        pipe_hdl, 
        Some(&mut read_buffer), 
        Some(&mut bytes_read), 
        None
    )?;

    let mut bytes_written: u32 = 0;

    Win32FileSystem::WriteFile(
        pipe_hdl,
        Some(&handler(&read_buffer[..bytes_read as usize])),
        Some(&mut bytes_written), 
        None
    )?;
    
    Win32FileSystem::FlushFileBuffers(pipe_hdl)?;
    
    Win32Pipes::DisconnectNamedPipe(pipe_hdl)?;
    Win32Foundation::CloseHandle(pipe_hdl)?;
    Ok(())
}
}


pub fn send(pipe_name: PCWSTR, data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        
        let mut read_buffer = [0u8; 1024];
        let mut bytes_read: u32 = 0;

        let result = Win32Pipes::CallNamedPipeW(
            pipe_name,
            Some(data.as_ptr() as *const c_void),
            data.len() as u32,
            Some(read_buffer.as_mut_ptr() as *mut c_void),
            1024,
            &mut bytes_read,
            Win32Pipes::NMPWAIT_USE_DEFAULT_WAIT,
        );

        if result.as_bool() {
            if bytes_read > 0 {
                Ok(read_buffer[..bytes_read as usize].to_vec())
            } else {
                Err(Error::Win32(windows::core::Error::from_win32()))
            }
        } else {
            Err(Error::Win32(windows::core::Error::from_win32()))
        }
    }
}