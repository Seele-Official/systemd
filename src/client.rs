use crate::{SERVICE_PIPE_NAME_WIDE, Cli, pipe};

pub fn run(cli: &Cli) -> String {

    let msg = match serde_json::to_string(&cli) {
        Ok(msg) => msg,
        Err(e) => {
            return format!("Failed to serialize CLI message: {}", e);
        }
    };

    match pipe::send(SERVICE_PIPE_NAME_WIDE, msg.as_bytes()) {
        Ok(response) => {
            unsafe {
                String::from_utf8_unchecked(response)
            }
        }
        Err(e) => {
            return format!("Failed to send message to pipe: {}", e);
        }
    }
}
