use rustfft::num_complex::Complex;

// An upmixed window, in the time domain
#[derive(Debug)]
pub struct UpmixedWindow {
    pub sample_ctr: i32,
    pub left_front: Vec<Complex<f32>>,
    pub right_front: Vec<Complex<f32>>,
    pub left_rear: Vec<Complex<f32>>,
    pub right_rear: Vec<Complex<f32>>,
}
