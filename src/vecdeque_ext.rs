use std::collections::VecDeque;

pub trait VecDequeExt<T> {
    fn to_vec(&self) -> Vec<T>;
}

impl<T: Clone> VecDequeExt<T> for VecDeque<T> {
    fn to_vec(&self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.len());

        for ctr in 0..self.len() {
            vec.push(self[ctr].clone());
        }

        vec
    }
}
