use biquad::{Coefficients, DirectForm2Transposed, ToHertz, Type};

pub struct BandpassFilter {
    pub filter: DirectForm2Transposed<f32>,
}

impl BandpassFilter {
    pub fn new(sample_rate: u32, high_freq: f32, low_freq: f32) -> Self
    {
        let nyquist = sample_rate as f32 / 2.0;
        let normalized_high_freq = (high_freq / nyquist).hz();
        let normalized_low_freq = (low_freq / nyquist).hz();
        let cuffs = Coefficients::<f32>::from_params(
            Type::BandPass,       // Bandpass filter type
            normalized_high_freq,
            normalized_low_freq,
            std::f32::consts::FRAC_1_SQRT_2, // Q factor
        ).unwrap();
        let filter = DirectForm2Transposed::<f32>::new(cuffs);
        Self { filter }
    }
}