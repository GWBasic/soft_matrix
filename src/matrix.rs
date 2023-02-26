use std::f32::consts::{PI, TAU};

use crate::structs::ThreadState;

pub trait Matrix {
    fn phase_shift(
        &self,
        thread_state: &ThreadState,
        left_front_phase: &mut f32,
        right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    );
}

pub struct PhaseMatrix {
    left_rear_shift: f32,
    right_rear_shift: f32,
}

impl PhaseMatrix {
    pub fn default() -> PhaseMatrix {
        Self::new(-0.5 * PI, 0.5 * PI)
    }

    // Note that it is intended that PhaseMatrix can be configured to support the old quad matrixes
    fn new(left_rear_shift: f32, right_rear_shift: f32) -> PhaseMatrix {
        PhaseMatrix {
            left_rear_shift,
            right_rear_shift,
        }
    }
}

impl Matrix for PhaseMatrix {
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
