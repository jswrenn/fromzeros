#![feature(const_generics)]

pub use fromzeros_derive::*;

pub unsafe trait FromZeros {
  #[inline(always)]
  fn zeroed() -> Self
  where Self: Sized
  {
    unsafe { std::mem::zeroed() }
  }
}

pub fn zeroed<T>() -> T
where
  T: FromZeros + Sized
{
    unsafe { std::mem::zeroed() }
}

macro_rules! impl_fromzeros{
  ($($ty : ty)*) => {$(unsafe impl FromZeros for $ty {})*}
}

impl_fromzeros!{
  ()
  bool
  char
  i8
  i16
  i32
  i64
  i128
  isize
  f32
  f64
  u8
  u16
  u32
  u64
  u128
  usize
}

unsafe impl<T: FromZeros> FromZeros for *const T {}
unsafe impl<T: FromZeros> FromZeros for *mut T {}

unsafe impl<T> FromZeros for [T] {}
unsafe impl<T: FromZeros, const N: usize> FromZeros for [T; {N}] {}
