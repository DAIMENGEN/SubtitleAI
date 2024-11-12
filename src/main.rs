use std::error::Error;

use log::{info, LevelFilter};
use simplelog::{ColorChoice, Config, TerminalMode, TermLogger};

use crate::audio::hardware::input::audio_input_device_manager::AudioInputDeviceManager;

mod audio;
mod proto {
    include!("proto_gen/chat.service.rs");
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
    let device_manager = AudioInputDeviceManager::new();
    let microphones = device_manager.get_available_microphones();
    info!("Available microphones:");
    for (index, microphone) in microphones.iter().enumerate() {
        info!("{}: {}", index, microphone.get_device_name());
    }
    microphones[0].start_record().await;
    Ok(())
}
