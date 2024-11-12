use std::{env, fs};
use std::collections::VecDeque;

use biquad::Biquad;
use chrono::Local;
use cpal::{Device, InputCallbackInfo, SampleFormat, SampleRate, StreamConfig, SupportedStreamConfig};
use cpal::traits::{DeviceTrait, StreamTrait};
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
        self.get_device_config().sample_format()
    }
    pub fn get_device_config(&self) -> SupportedStreamConfig {
        self.device.default_input_config().unwrap_or_else(|err| {
            error!("Failed to get default input config: {}", err);
            panic!("Failed to get default input config: {}", err)
        })
    }
    pub fn get_device_channels(&self) -> u16 {
        self.get_device_config().channels()
    }
    pub fn get_device_sample_rate(&self) -> u32 {
        self.get_device_config().sample_rate().0
    }
    pub fn start_recording(&self) -> () {
        let (tx, rx) = mpsc::channel(100);
        let name = self.get_device_name();
        let channels = self.get_device_channels();
        let sample_rate = self.get_device_sample_rate();
        let target_sample_rate = Self::TARGET_SAMPLE_RATE;
        let sample_format = self.get_device_sample_format();
        let config = StreamConfig {
            channels,
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(3072u32 * (channels as u32)),
        };
        info!("Start recording on device: {}", name);
        info!("Device supported stream config: {:#?}", &config);
        let mut shared_data: Vec<f32> = Vec::new();
        let stream = match sample_format {
            SampleFormat::F32 => {
                self.device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &InputCallbackInfo| {
                        if channels == 1 {
                            shared_data.extend_from_slice(data);
                        } else {
                            let mono_data = data.chunks(channels as usize).map(|pair| {
                                pair.iter().sum::<f32>() / (pair.len() as f32)
                            });
                            shared_data.extend(mono_data);
                        }
                        let shared_data_len = shared_data.len();
                        if shared_data_len >= 3072usize {
                            let mut resampled = if sample_rate != target_sample_rate {
                                let interpolator = Linear::new(shared_data[0], shared_data[1]);
                                let source = signal::from_iter(shared_data.iter().cloned());
                                let resampled = source.from_hz_to_hz(
                                    interpolator,
                                    sample_rate as f64,
                                    target_sample_rate as f64,
                                );
                                resampled.take(shared_data_len * (target_sample_rate / sample_rate) as usize).collect()
                            } else {
                                shared_data.to_vec()
                            };
                            shared_data.clear();
                            let mut bandpass_filter = BandpassFilter::new(target_sample_rate, 3000f32, 300f32);
                            for sample in resampled.iter_mut() {
                                *sample = bandpass_filter.filter.run(*sample) * 2.0
                            }
                            tx.blocking_send(resampled).unwrap_or_else(|error| {
                                error!("Error sending data to channel: {:?}", error);
                                panic!("Error sending data to channel: {:?}", error)
                            });
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
        stream.play().unwrap_or_else(|error| {
            error!("Error playing stream: {:?}", error);
            panic!("Error playing stream: {:?}", error)
        });
        self.save_audio_data_to_wav(rx);
    }

    fn save_audio_data_to_wav(&self, mut audio_data_rx: Receiver<Vec<f32>>) -> (JoinHandle<()>, Receiver<String>) {
        let (tx, rx) = mpsc::channel::<String>(100);
        let predict_gate = 0.75f32;
        let sample_length = 16usize;
        let sample_length_half = sample_length / 2;
        let mut output_temp_dir = env::current_dir().unwrap_or_else(|error| {
            error!("Error getting current directory: {:?}", error);
            panic!("Error getting current directory: {:?}", error)
        });
        output_temp_dir.push("recordings");
        fs::create_dir_all(&output_temp_dir).unwrap_or_else(|error| {
            error!("Error creating directory: {:?}", error);
            panic!("Error creating directory: {:?}", error)
        });
        let input_config = self.get_device_config();
        let mut vad_detector = self.voice_activity_detector();
        (tokio::spawn(async move {
            let mut audio_data_buffer = VecDeque::<AudioSegment>::new();
            loop {
                if let Some(audio_data) = audio_data_rx.recv().await {
                    let predict = vad_detector.predict(audio_data.clone());
                    let audio_segment = AudioSegment::new(audio_data.clone(), predict);
                    audio_data_buffer.push_front(audio_segment);
                    while audio_data_buffer.len() > sample_length
                        && audio_data_buffer.iter()
                        .skip(audio_data_buffer.len() - sample_length).filter(|&segment| segment.speech_probability < predict_gate)
                        .count() >= sample_length_half {
                        audio_data_buffer.pop_back();
                    }
                    let probabilities = &audio_data_buffer.iter().take(sample_length).map(|segment| segment.speech_probability).collect::<Vec<_>>();
                    if audio_data_buffer.len() >= sample_length && AudioSegment::is_pause(probabilities) {
                        let local: chrono::DateTime<Local> = Local::now();
                        let filename = format!("{}.wav", local.format("%Y%m%d %H_%M_%S.%3f"));
                        let wav_spec = wav_writer::get_wav_spec(&input_config);
                        let writer = wav_writer::get_wav_writer(output_temp_dir.join(filename.clone()), wav_spec);
                        let audio_data: Vec<f32> = audio_data_buffer.iter().flat_map(|segment| segment.audio_data.clone()).collect();
                        wav_writer::write_audio_data_to_wav::<f32, f32>(writer, &audio_data);
                        audio_data_buffer.clear();
                        tx.send(filename).await.unwrap_or_else(|error| {
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