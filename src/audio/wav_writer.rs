use std::{fs, path};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

use cpal::FromSample;
use hound::WavWriter;
use log::{error, warn};

pub type SharedWavWriter = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;
fn get_sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}
pub fn get_wav_spec(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        sample_format: get_sample_format(config.sample_format()),
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
    }
}
pub fn get_wav_writer<P: AsRef<path::Path>>(filepath: P, spec: hound::WavSpec) -> SharedWavWriter {
    let filepath_ref = filepath.as_ref();
    let filepath_str = filepath_ref.to_str().unwrap();
    if filepath_ref.exists() {
        warn!("File already exists, preparing to delete an existing file. filepath: {:?}", filepath_str);
        fs::remove_file(filepath_ref).unwrap_or_else(|error| {
            error!("Error deleting file: {:?}, file path: {:?}", error, filepath_str);
            panic!("Error deleting file: {:?}, file path: {:?}", error, filepath_str);
        });
    }
    match hound::WavWriter::create(filepath_ref, spec) {
        Ok(writer) => Arc::new(Mutex::new(Some(writer))),
        Err(e) => {
            error!("Error creating WAV writer: {:?}, file path: {:?}", e, filepath_str);
            panic!("Error creating WAV writer: {:?}, file path: {:?}", e, filepath_str)
        }
    }
}

pub fn write_audio_data_to_wav<T, U>(writer: SharedWavWriter, data: &[T])
where
    T: cpal::Sample,
    U: cpal::Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in data.iter() {
                let sample = U::from_sample(sample);
                writer.write_sample(sample).ok();
            }
            writer.flush().unwrap_or_else(|error| {
                error!("Error flushing WAV writer: {:?}", error);
                panic!("Error flushing WAV writer: {:?}", error)
            });
        }
    }
}