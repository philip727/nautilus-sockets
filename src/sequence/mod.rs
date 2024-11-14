use std::ops::{Add, AddAssign};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SequenceNumber(u32);

impl SequenceNumber {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn raw(&self) -> u32 {
        self.0
    }
}

impl Add for SequenceNumber {
    type Output = SequenceNumber;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for SequenceNumber {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self(self.0 + rhs.0)
    }
}
