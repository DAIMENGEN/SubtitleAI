use std::collections::VecDeque;
use std::{path, thread};
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, InputCallbackInfo, SampleFormat, SizedSample, Stream, StreamError, SupportedStreamConfig};
use log::{error, info};

use crate::audio::hardware::input::audio_input_device::AudioInputDevice;
use crate::audio::wav_writer;
use crate::audio::wav_writer::SharedWavWriter;

#[derive(Clone)]
pub struct Microphone {
    pub device: Device,
}

impl Microphone {
    pub fn new(device: Device) -> Self {
        Microphone { device }
    }
}

impl AudioInputDevice for Microphone {
    fn get_device_name(&self) -> String {
        self.device.name().unwrap_or_else(|_| {
            error!("Failed to get device name");
            panic!("Failed to get device name");
        })
    }
    fn get_device_sample_rate(&self) -> u32 {
        self.get_default_input_config().sample_rate().0
    }
    fn get_device_sample_format(&self) -> SampleFormat {
        self.get_default_input_config().sample_format()
    }
    fn get_default_input_config(&self) -> SupportedStreamConfig {
        self.device.default_input_config().unwrap_or_else(|err| {
            error!("Failed to get default input config: {}", err);
            panic!("Failed to get default input config: {}", err)
        })
    }
    fn start_recording(&self, output_path: String) -> () {
        let output_path: Arc<dyn AsRef<path::Path> + Send + Sync> = Arc::new(output_path);
        let sample_format = self.get_device_sample_format();
        let input_config = self.get_default_input_config();

        let mut stream: Option<Stream> = None;
        let mut wav_writer: Option<SharedWavWriter> = None;

        match sample_format {
            SampleFormat::F32 => {
                let mut data_buf = VecDeque::<Vec<f32>>::new();
                stream = Some(self.device.build_input_stream(
                    &input_config.clone().into(),
                    move |data: &[f32], _: &InputCallbackInfo| {
                        let data = Vec::from(data);
                        data_buf.push_front(data);

                        if data_buf.len() > 10 && wav_writer.is_none() {
                            let local: chrono::DateTime<Local> = Local::now();
                            let filename = format!("{}.wav", local.format("%Y%m%d %H_%M_%S.%3f"));
                            let wav_spec = wav_writer::get_wav_spec(&input_config);
                            wav_writer = Some(wav_writer::get_wav_writer(output_path.as_ref().as_ref().join(filename), wav_spec));
                        }

                        if data_buf.len() > 500 {
                            if let Some(writer) = &wav_writer {
                                while !data_buf.is_empty() {
                                    let it = data_buf.pop_back().unwrap();
                                    wav_writer::write_audio_data_to_wav::<f32, f32>(writer, &it);
                                }
                                wav_writer = None;
                                data_buf.clear();
                            }
                        }
                    },
                    |error| {
                        error!("Error: {}", error);
                    },
                    None,
                ).unwrap())
            }
            _ => {
                error!("Unsupported sample format: {:?}", sample_format);
                panic!("Unsupported sample format: {:?}", sample_format)
            }
        }

        match stream {
            Some(stream) => {
                stream.play().unwrap();
                thread::sleep(Duration::from_secs(100000));
            }
            None => panic!("Failed to create stream")
        }
    }
}