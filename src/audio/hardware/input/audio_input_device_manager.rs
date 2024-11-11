use cpal::traits::HostTrait;
use log::error;

use crate::audio::hardware::input::microphone::Microphone;

pub struct AudioInputDeviceManager;

impl AudioInputDeviceManager {
    pub fn new() -> Self {
        AudioInputDeviceManager
    }

    pub fn get_available_microphones(&self) -> Vec<Microphone> {
        let host = cpal::default_host();
        let mut microphones: Vec<Microphone> = Vec::new();
        let input_devices = host.input_devices().unwrap_or_else(|err| {
            error!("Failed to get input devices: {}", err);
            panic!("Failed to get input devices: {}", err)
        });
        for device in input_devices {
            let microphone = Microphone::new(device);
            microphones.push(microphone);
        }
        microphones
    }
}