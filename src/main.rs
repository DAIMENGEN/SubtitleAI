use std::env;
use std::path::PathBuf;

use log::{error, info, LevelFilter};
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use crate::audio::hardware::input::audio_input_device::AudioInputDevice;
use crate::audio::hardware::input::audio_input_device_manager::AudioInputDeviceManager;
use crate::audio::wav_writer;

mod audio;
mod proto {
    include!("proto_gen/chat.service.rs");
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto).unwrap();
    let target_file_path: PathBuf = env::current_dir().unwrap().join("temp");
    let device_manager = AudioInputDeviceManager::new();
    let microphones = device_manager.get_available_microphones();
    info!("Available microphones:");
    for (index, microphone) in microphones.iter().enumerate() {
        info!("{}: {}", index, microphone.get_device_name());
    }
    let microphone = &microphones[0];
    microphone.start_recording(target_file_path.to_str().unwrap().to_string());
    Ok(())
}
