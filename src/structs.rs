use rustfft::num_complex::Complex;

#[derive(Debug)]
pub struct FrequenciesAndPositions {
    // The left channel, transformed (frequences and phases)
    pub left_frequences: Vec<Complex<f32>>,

    // The right channel, transformed (frequences and phases)
    pub right_frequences: Vec<Complex<f32>>,

    // Right to left measurements, 0 is right, 1 is left
    pub right_to_lefts: Vec<f32>,

    // Phase ratios, 0 is in phase, 1 is out of phase
    pub phase_ratios: Vec<f32>,
}
