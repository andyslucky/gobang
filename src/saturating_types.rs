/// Module for types whose operations saturate at bounds instead of panic.
use std::cmp::Ordering;
use std::ops;

#[derive(Clone, Copy)]
pub struct SaturatingU16(pub u16);

impl Into<SaturatingU16> for u16 {
    fn into(self) -> SaturatingU16 {
        return SaturatingU16(self);
    }
}

impl Into<u16> for SaturatingU16 {
    fn into(self) -> u16 {
        return self.0;
    }
}

impl Into<usize> for SaturatingU16 {
    fn into(self) -> usize {
        return self.0 as usize;
    }
}

impl PartialEq<u16> for SaturatingU16 {
    fn eq(&self, other: &u16) -> bool {
        return self.0 == *other;
    }

    fn ne(&self, other: &u16) -> bool {
        return self.0 != *other;
    }
}
impl PartialOrd<u16> for SaturatingU16 {
    fn partial_cmp(&self, other: &u16) -> Option<Ordering> {
        if self.0 < *other {
            Some(Ordering::Less)
        } else if self.0 > *other {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }

    fn lt(&self, other: &u16) -> bool {
        return self.0 < *other;
    }

    fn le(&self, other: &u16) -> bool {
        return self.0 <= *other;
    }

    fn gt(&self, other: &u16) -> bool {
        return self.0 > *other;
    }

    fn ge(&self, other: &u16) -> bool {
        return self.0 >= *other;
    }
}
impl ops::AddAssign<u16> for SaturatingU16 {
    fn add_assign(&mut self, rhs: u16) {
        self.0 = self.0.saturating_add(rhs);
    }
}

impl ops::SubAssign<u16> for SaturatingU16 {
    fn sub_assign(&mut self, rhs: u16) {
        self.0 = self.0.saturating_sub(rhs);
    }
}

impl ops::Add<u16> for SaturatingU16 {
    type Output = SaturatingU16;
    fn add(self, rhs: u16) -> Self::Output {
        return SaturatingU16(self.0.saturating_add(rhs));
    }
}

impl ops::Sub<u16> for SaturatingU16 {
    type Output = SaturatingU16;
    fn sub(self, rhs: u16) -> Self::Output {
        return SaturatingU16(self.0.saturating_sub(rhs));
    }
}
