use std::collections::VecDeque;
use std::path;
use std::sync::Arc;
use std::time::Duration;

use chrono::Local;
use cpal::{Device, InputCallbackInfo, SampleFormat, SizedSample, Stream, StreamError, SupportedStreamConfig};
use cpal::traits::{DeviceTrait, StreamTrait};
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
    fn start_recording<P, E>(&self, output_path: P, error_callback: E)
    where
        P: AsRef<path::Path> + Send + Sync + 'static,
        E: FnMut(StreamError) + Send + 'static,
    {
        let output_path: Arc<dyn AsRef<path::Path> + Send + Sync> = Arc::new(output_path);
        let time_len = 16usize;
        let time_len_half = 8usize;
        let predict_gate = 0.75f32;
        let sample_rate = self.get_device_sample_rate();
        let sample_format = self.get_device_sample_format();
        let input_config = self.get_default_input_config();

        let mut vad = voice_activity_detector::VoiceActivityDetector::builder()
            .sample_rate(sample_rate).chunk_size(1024usize).build().unwrap();

        let mut stream: Option<Stream> = None;
        let mut wav_writer: Option<SharedWavWriter> = None;

        match sample_format {
            SampleFormat::F32 => {
                let mut data_buf = VecDeque::<(f32, Vec<f32>)>::new();
                stream = Some(self.device.build_input_stream(
                    &input_config.clone().into(),
                    move |data: &[f32], _: &InputCallbackInfo| {
                        let data = Vec::from(data);
                        let predict = vad.predict(data.clone());
                        info!("Predict: {}", predict);
                        data_buf.push_front((predict, data));

                        if data_buf.len() > time_len_half && predict > predict_gate && wav_writer.is_none() {
                            let local: chrono::DateTime<Local> = Local::now();
                            let filename = format!("{}.wav", local.format("%Y%m%d %H_%M_%S.%3f"));
                            let wav_spec = wav_writer::get_wav_spec(&input_config);
                            wav_writer = Some(wav_writer::get_wav_writer(output_path.as_ref().as_ref().join(filename), wav_spec));
                        }

                        while data_buf.len() >= time_len && data_buf.iter().skip(data_buf.len() - time_len).filter(|(a, _)| *a < predict_gate).count() >= time_len_half {
                            data_buf.pop_back();
                        }

                        if data_buf.len() > time_len && avg_compare(data_buf.iter().take(time_len).map(|t| t.0).collect()) {
                            if let Some(writer) = &wav_writer {
                                while !data_buf.is_empty() {
                                    let it = data_buf.pop_back().unwrap().1;
                                    wav_writer::write_audio_data_to_wav::<f32, f32>(writer, &it);
                                }
                                wav_writer = None;
                            }
                        }
                        todo!()
                    },
                    error_callback,
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
            }
            None => panic!("Failed to create stream")
        }
    }
}

fn avg_compare(input: Vec<f32>) -> bool {
    let n = 3;
    let sum: f32 = input.iter().skip(n).sum(); // 计算所有元素的总和
    let avg: f32 = sum / (input.len() as f32 - n as f32); // 计算平均值
    // 这里假设需要比较平均值是否大于0，具体逻辑可以根据需求调整
    input[0..n].iter().all(|&x| x <= avg * 0.75f32)
}