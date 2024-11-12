pub struct AudioSegment {
    pub audio_data: Vec<f32>,
    pub speech_probability: f32,
}

impl AudioSegment {
    pub fn new(audio_data: Vec<f32>, speech_probability: f32) -> Self {
        AudioSegment {
            audio_data,
            speech_probability,
        }
    }
    pub fn is_pause(probabilities: &Vec<f32>) -> bool {
        let frame_rate = 3;
        let len = probabilities.len() as f32;
        let sum = probabilities.iter().skip(frame_rate).sum::<f32>();
        let average = sum / (len - frame_rate as f32);
        probabilities[0..frame_rate].iter().all(|&x| x <= average * 0.75f32)
    }
}