use std::f32::consts::{PI, TAU};

use crate::structs::ThreadState;

pub trait Matrix {
    // Widening is currently disabled because it results in poor audio quality, and favors too
    // much steering to the rear
    //fn widen(&self, back_to_front: &mut f32, left_to_right: &mut f32);

    fn phase_shift(
        &self,
        thread_state: &ThreadState,
        left_front_phase: &mut f32,
        right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    );
}

pub struct DefaultMatrix {
    _left_front_volume: f32,
    _right_front_volume: f32,
    _center_volume: f32,
    _left_rear_volume: f32,
    _right_rear_volume: f32,
    left_rear_shift: f32,
    right_rear_shift: f32,
}

// Note that it is intended that PhaseMatrix can be configured to support the old quad matrixes
impl DefaultMatrix {
    pub fn new() -> DefaultMatrix {
        DefaultMatrix {
            _left_front_volume: 1.0,
            _right_front_volume: 1.0,
            _center_volume: 1.0,
            _left_rear_volume: 1.0,
            _right_rear_volume: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
        }
    }

    pub fn sq() -> DefaultMatrix {
        /*
        Matrix {
            left_front_volume: 1.0,
            right_front_volume: 1.0,
            center_volume: 1.0,
            left_rear_volume: 1.0,
            right_rear_volume: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
        }*/
        panic!("Currently unimplemented");
    }
}

impl Matrix for DefaultMatrix {
    // Widening is currently disabled because it results in poor audio quality, and favors too
    // much steering to the rear
    /*
    fn widen(&self, back_to_front: &mut f32, left_to_right: &mut f32) {
        *left_to_right *= 10.0;
        *left_to_right = left_to_right.min(1.0).max(-1.0);

        if *back_to_front > 0.0 {
            *back_to_front *= 10.0;
            *back_to_front = back_to_front.min(1.0);
        }
    }
    */

    fn phase_shift(
        &self,
        _thread_state: &ThreadState,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        shift(left_rear_phase, self.left_rear_shift);
        shift(right_rear_phase, self.right_rear_shift);
    }
}

fn shift(phase: &mut f32, shift: f32) {
    *phase += shift;

    if *phase > PI {
        *phase -= TAU;
    } else if *phase < -PI {
        *phase += TAU;
    }
}
