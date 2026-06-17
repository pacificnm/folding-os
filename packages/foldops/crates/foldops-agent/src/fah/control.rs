//! FAH WebSocket control commands (pause / fold / finish).

use super::websocket::send_fah_control_command;

pub async fn send_fah_pause(host: &str, port: u16) -> Result<(), String> {
    send_fah_control_command("pause", host, port).await
}

pub async fn send_fah_resume(host: &str, port: u16) -> Result<(), String> {
    send_fah_control_command("fold", host, port).await
}

pub async fn send_fah_finish(host: &str, port: u16) -> Result<(), String> {
    send_fah_control_command("finish", host, port).await
}
