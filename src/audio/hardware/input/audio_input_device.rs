pub trait AudioInputDevice {
    fn get_device_name(&self) -> String;
    fn get_device_sample_rate(&self) -> u32;
    fn get_device_sample_format(&self) -> cpal::SampleFormat;
    fn get_default_input_config(&self) -> cpal::SupportedStreamConfig;
    fn start_recording(&self, output_path: String) -> ();
}