use std::env;
use std::path::PathBuf;

use log::{error, info};

use crate::audio::hardware::input::audio_input_device::AudioInputDevice;
use crate::audio::hardware::input::audio_input_device_manager::AudioInputDeviceManager;
use crate::audio::wav_writer;

mod audio;
mod proto {
    include!("proto_gen/chat.service.rs");
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_file_path: PathBuf = env::current_dir().unwrap().join("recorded_microphone.wav");
    let device_manager = AudioInputDeviceManager::new();
    let microphones = device_manager.get_available_microphones();
    info!("Available microphones:");
    for (index, microphone) in microphones.iter().enumerate() {
        info!("{}: {}", index, microphone.get_device_name());
    }
    let microphone = &microphones[0];
    let input_config = microphone.get_default_input_config();
    let sample_format = microphone.get_device_sample_format();
    let wav_spec = wav_writer::get_wav_spec(&input_config);
    let wav_writer = wav_writer::get_wav_writer(target_file_path, wav_spec);

    // match sample_format {
    //     cpal::SampleFormat::F32 => {
    //         let stream = microphone.start_recording(
    //             move |data: &[f32], _: &cpal::InputCallbackInfo| {
    //                 wav_writer::write_audio_data_to_wav::<f32, f32>(&wav_writer, data);
    //             },
    //             move |err| {
    //                 error!("Error in recording: {:?}", err);
    //             },
    //             None,
    //         );
    //     }
    //     _ => panic!("Unsupported sample format: {:?}", sample_format)
    // }

    Ok(())
}
