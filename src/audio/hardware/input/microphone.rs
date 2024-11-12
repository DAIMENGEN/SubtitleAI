use std::collections::VecDeque;
use std::{env, fs};

use biquad::Biquad;
use chrono::Local;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, InputCallbackInfo, SampleFormat, StreamConfig};
use dasp::{interpolate::linear::Linear, signal, Signal};
use log::{error, info};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

use crate::audio::filter::bandpass_filter::BandpassFilter;
use crate::audio::processing::audio_segment::AudioSegment;
use crate::audio::wav_writer;

#[derive(Clone)]
pub struct Microphone {
    pub device: Device,
}

impl Microphone {
    pub const TARGET_SAMPLE_RATE: u32 = 16000;
    pub fn new(device: Device) -> Self {
        Microphone { device }
    }
    pub fn get_device_name(&self) -> String {
        self.device.name().unwrap_or_else(|_| {
            error!("Failed to get device name");
            panic!("Failed to get device name");
        })
    }
    pub fn get_device_sample_format(&self) -> SampleFormat {
        self.device.default_input_config().unwrap_or_else(|error| {
            error!("Failed to get default input config: {}", error);
            panic!("Failed to get default input config: {}", error)
        }).sample_format()
    }
    pub fn get_device_config(&self) -> StreamConfig {
        match self.device.default_input_config() {
            Ok(config) => StreamConfig {
                channels: config.channels(),
                sample_rate: config.sample_rate(),
                buffer_size: cpal::BufferSize::Fixed(3072u32 * (config.channels() as u32)),
            },
            Err(error) => {
                error!("Failed to get default input config: {}", error);
                panic!("Failed to get default input config: {}", error)
            }
        }
    }
    pub fn get_device_channels(&self) -> u16 {
        self.device.default_input_config().unwrap_or_else(|error| {
            error!("Failed to get default input config: {}", error);
            panic!("Failed to get default input config: {}", error)
        }).channels()
    }
    pub fn get_device_sample_rate(&self) -> u32 {
        self.device.default_input_config().unwrap_or_else(|error| {
            error!("Failed to get default input config: {}", error);
            panic!("Failed to get default input config: {}", error)
        }).sample_rate().0
    }
    pub async fn start_record(&self) -> () {
        let config = self.get_device_config();
        let microphone_name = self.get_device_name();
        info!("Start recording sound using microphone : {}, stream config is: {:#?}", microphone_name, config);
        let (tx, rx) = mpsc::channel(100);
        let channels = self.get_device_channels();
        let sample_rate = self.get_device_sample_rate();
        let target_sample_rate = Self::TARGET_SAMPLE_RATE;
        let sample_format = self.get_device_sample_format();
        let mut audio_data_buffer: Vec<f32> = Vec::new();
        let stream = match sample_format {
            SampleFormat::F32 => {
                self.device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &InputCallbackInfo| {
                        if channels == 1 {
                            audio_data_buffer.extend_from_slice(data);
                        } else {
                            let mono_data = data.chunks(channels as usize).map(|pair| {
                                pair.iter().sum::<f32>() / (pair.len() as f32)
                            });
                            audio_data_buffer.extend(mono_data);
                        }
                        if audio_data_buffer.len() >= 3072usize {
                            let mut resampled = if sample_rate != target_sample_rate {
                                let interpolator = Linear::new(audio_data_buffer[0], audio_data_buffer[1]);
                                let source = signal::from_iter(audio_data_buffer.iter().cloned());
                                let resampled = source.from_hz_to_hz(
                                    interpolator,
                                    sample_rate as f64,
                                    target_sample_rate as f64,
                                );
                                let number = audio_data_buffer.len() * target_sample_rate as usize / sample_rate as usize;
                                resampled.take(number).collect()
                            } else {
                                audio_data_buffer.to_vec()
                            };
                            let mut bandpass_filter = BandpassFilter::new(target_sample_rate, 3000f32, 300f32);
                            for sample in resampled.iter_mut() {
                                *sample = bandpass_filter.filter.run(*sample) * 2.0
                            }
                            tx.blocking_send(resampled).unwrap_or_else(|error| {
                                error!("Error sending data to channel: {:?}", error);
                                panic!("Error sending data to channel: {:?}", error)
                            });
                            audio_data_buffer.clear();
                        }
                    },
                    move |err| {
                        error!("Error in input stream: {:?}", err);
                    },
                    None,
                ).unwrap_or_else(|error| {
                    error!("Error building input stream: {:?}", error);
                    panic!("Error building input stream: {:?}", error)
                })
            }
            _ => {
                error!("Unsupported sample format: {:?}", sample_format);
                panic!("Unsupported sample format: {:?}", sample_format)
            }
        };
        match stream.play() {
            Ok(_) => {
                let (async_handle, _) = self.process_audio_data(rx);
                let (voice_result, ) = tokio::join!(async_handle);
                if let Err(e) = voice_result {
                    error!("Voice record task failed: {:?}", e);
                }
            }
            Err(error) => {
                error!("Error playing stream: {:?}", error);
            }
        }
    }

    fn process_audio_data(&self, mut audio_data_rx: Receiver<Vec<f32>>) -> (JoinHandle<()>, Receiver<Vec<f32>>) {
        let (tx, rx) = mpsc::channel::<Vec<f32>>(100);
        let predict_gate = 0.75f32;
        let sample_length = 16usize;
        let sample_length_half = sample_length / 2;
        let target_sample_rate = Self::TARGET_SAMPLE_RATE;
        let sample_format = self.get_device_sample_format();
        let mut output_temp_dir = env::current_dir().unwrap_or_else(|error| {
            error!("Error getting current directory: {:?}", error);
            panic!("Error getting current directory: {:?}", error)
        });
        output_temp_dir.push("recordings");
        fs::create_dir_all(&output_temp_dir).unwrap_or_else(|error| {
            error!("Error creating directory: {:?}", error);
            panic!("Error creating directory: {:?}", error)
        });
        let mut vad_detector = self.voice_activity_detector();
        (tokio::spawn(async move {
            let mut audio_data_buffer = VecDeque::<AudioSegment>::new();
            loop {
                if let Some(audio_data) = audio_data_rx.recv().await {
                    let predict = vad_detector.predict(audio_data.clone());
                    let audio_segment = AudioSegment::new(audio_data.clone(), predict);
                    audio_data_buffer.push_front(audio_segment);
                    while audio_data_buffer.len() >= sample_length
                        && audio_data_buffer.iter()
                        .skip(audio_data_buffer.len() - sample_length).filter(|&segment| segment.speech_probability < predict_gate)
                        .count() >= sample_length_half {
                        audio_data_buffer.pop_back();
                    }
                    let probabilities = &audio_data_buffer.iter().take(sample_length).map(|segment| segment.speech_probability).collect::<Vec<_>>();
                    if audio_data_buffer.len() > sample_length && AudioSegment::is_pause(probabilities) {
                        let mut file_data: Vec<f32> = Vec::new();
                        let local: chrono::DateTime<Local> = Local::now();
                        let filename = format!("{}.wav", local.format("%Y%m%d %H_%M_%S.%3f"));
                        let wav_spec = wav_writer::get_wav_spec(target_sample_rate, sample_format);
                        let writer = wav_writer::get_wav_writer(output_temp_dir.join(filename.clone()), wav_spec);
                        while !audio_data_buffer.is_empty() {
                            let audio_data: Vec<f32> = audio_data_buffer.pop_back().unwrap().audio_data;
                            file_data.extend(audio_data);
                        }
                        wav_writer::write_audio_data_to_wav::<f32, f32>(writer, &file_data);
                        audio_data_buffer.clear();
                        tx.send(file_data).await.unwrap_or_else(|error| {
                            error!("Error sending filename to channel: {:?}", error);
                        });
                    }
                }
            }
        }), rx)
    }

    fn voice_activity_detector(&self) -> voice_activity_detector::VoiceActivityDetector {
        voice_activity_detector::VoiceActivityDetector::builder()
            .sample_rate(Self::TARGET_SAMPLE_RATE)
            .chunk_size(1024usize).build().unwrap()
    }
}