use std::{
    fmt::Display,
    iter::Sum,
    marker::PhantomData,
    ops::{Add, Div, Mul, Neg, Rem, Sub},
};

use serde::{Deserialize, Serialize};

pub trait Unit: Into<f64> {
    fn inner(self) -> f64 {
        self.into()
    }
}
macro_rules! Quantity {
    ($name: ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
        pub struct $name(f64);

        impl From<f64> for $name {
            fn from(value: f64) -> Self {
                $name(value)
            }
        }

        impl From<$name> for f64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl Unit for $name {}

        impl Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl Add for $name {
            type Output = $name;

            fn add(self, rhs: Self) -> Self::Output {
                $name(self.0 + rhs.0)
            }
        }

        impl Sum for $name {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold($name(0.0), |a, b| a + b)
            }
        }

        impl Sub for $name {
            type Output = $name;

            fn sub(self, rhs: Self) -> Self::Output {
                $name(self.0 - rhs.0)
            }
        }

        impl Neg for $name {
            type Output = $name;

            fn neg(self) -> Self::Output {
                $name(-self.0)
            }
        }

        impl Div for $name {
            type Output = f64;

            fn div(self, rhs: Self) -> Self::Output {
                self.0 / rhs.0
            }
        }

        impl Div<f64> for $name {
            type Output = $name;

            fn div(self, rhs: f64) -> Self::Output {
                $name(self.0 / rhs)
            }
        }

        impl Mul<f64> for $name {
            type Output = $name;

            fn mul(self, rhs: f64) -> Self::Output {
                $name(self.0 * rhs)
            }
        }

        impl Mul<$name> for f64 {
            type Output = $name;

            fn mul(self, rhs: $name) -> Self::Output {
                $name(self * rhs.0)
            }
        }

        impl Rem for $name {
            type Output = $name;

            fn rem(self, rhs: Self) -> Self::Output {
                $name(self.0 % rhs.0)
            }
        }

        impl $name {
            #[inline]
            pub fn map<F>(self, f: F) -> Self
            where
                F: FnOnce(f64) -> f64,
            {
                Self(f(self.0))
            }

            /// Same as calling powi on the underlying float.
            /// Strictly this should change the unit but doesn't.  
            pub fn powi(self, exp: i32) -> Self {
                Self(self.0.powi(exp))
            }

            /// Strictly this should change the unit but doesn't.  
            pub fn sqrt(self) -> Self{
                Self(self.0.sqrt())
            }

            pub fn min(self, other: Self) -> Self {
                Self(self.0.min(other.0))
            }

            pub fn max(self, other: Self) -> Self {
                Self(self.0.max(other.0))
            }
        }
    };
}

macro_rules! DivRelation {
    ($top:ident, $bottom:ident, $result:ident) => {
        impl Div<$bottom> for $top {
            type Output = $result;

            fn div(self, rhs: $bottom) -> Self::Output {
                $result(Into::<f64>::into(self) / rhs.0)
            }
        }

        impl Mul<$result> for $bottom {
            type Output = $top;

            fn mul(self, rhs: $result) -> Self::Output {
                (self.0 * rhs.0).into()
            }
        }

        impl Mul<$bottom> for $result {
            type Output = $top;

            fn mul(self, rhs: $bottom) -> Self::Output {
                (self.0 * rhs.0).into()
            }
        }

        impl Div<$result> for $top {
            type Output = $bottom;

            fn div(self, rhs: $result) -> Self::Output {
                $bottom(Into::<f64>::into(self) / rhs.0)
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! MulRelation {
    ($left: ident, $right: ident, $result: ident) => {
        DivRelation!($result, $left, $right);
    };
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Db<T>(f64,  #[serde(skip)] PhantomData<T>);

impl<T, A> Add<Db<A>> for Db<T>
where
    T: Mul<A>,
{
    type Output = Db<T::Output>;

    fn add(self, rhs: Db<A>) -> Self::Output {
        Db::<T::Output>::from(self.0 + rhs.0)
    }
}

impl<T, A> Sub<Db<A>> for Db<T>
where
    T: Div<A>,
{
    type Output = Db<T::Output>;

    fn sub(self, rhs: Db<A>) -> Self::Output {
        Db::<T::Output>::from(self.0 - rhs.0)
    }
}

impl<T> From<f64> for Db<T> {
    fn from(value: f64) -> Self {
        Self(value, PhantomData)
    }
}

impl<T> From<Db<T>> for f64 {
    fn from(value: Db<T>) -> Self {
        value.0
    }
}

impl<T> From<T> for Db<T>
where
    T: Unit,
{
    fn from(value: T) -> Self {
        let log = 10.0 * value.inner().log10();
        Self(log, PhantomData)
    }
}

impl<T> Db<T>
where
    T: From<f64> + Into<f64>,
{
    pub fn as_linear(self) -> T {
        T::from(10f64.powf(self.0 / 10.0))
    }

    pub fn as_db_float(self) -> f64 {
        self.0
    }

    fn from_linear(val: f64) -> Self {
        let log = 10.0 * val.log10();
        Db::from(log)
    }

    /// From the equivalent non-logarithmic unit.
    /// This will apply the `10 * log(value)` transform.
    pub fn from_unit(val: T) -> Self {
        Self::from_linear(val.into())
    }

    const fn from_db(val: f64) -> Self {
        Self(val, PhantomData)
    }

    #[inline]
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(f64) -> f64,
    {
        Self::from(f(self.0))
    }
}

impl<T> Mul<f64> for Db<T> {
    type Output = Db<T>;

    fn mul(self, rhs: f64) -> Self::Output {
        Db::from(self.0 * rhs)
    }
}

impl<T> Mul<Db<T>> for f64 {
    type Output = Db<T>;

    fn mul(self, rhs: Db<T>) -> Self::Output {
        Db::from(self * rhs.0)
    }
}

Quantity!(Length);
pub const METRES : Length = Length::from_metres(1.0);
pub const KM : Length = Length::from_metres(1000.0);
impl Length {
    pub const fn from_metres(n: f64) -> Self {
        Length(n)
    }

    pub fn metres(self) -> f64 {
        self.0
    }
}

Quantity!(Time);
pub const HOURS: Time = Time::from_seconds(60.0 * 60.0);
pub const MINS : Time = Time::from_seconds(60.0);
pub const SECONDS : Time = Time::from_seconds(1.0);
impl Time {
    pub const fn from_seconds(n: f64) -> Self {
        Time(n)
    }

    pub const fn from_milis(n: f64) -> Self {
        Time(n / 1000.0)
    }

    pub const fn from_imilis(n: i32) -> Self {
        Time((n as f64) / 1000.0)
    }

    pub fn seconds(self) -> f64 {
        self.0
    }

    pub fn milis(self) -> f64 {
        self.0 * 1000.0
    }
}

Quantity!(Mass);
Quantity!(Temperature);

impl Temperature {
    pub const fn from_celsius(n: f64) -> Self {
        Temperature(n + 273.15)
    }

    pub const fn from_kelvin(n: f64) -> Self {
        Temperature(n)
    }

    pub fn kelvin(self) -> f64 {
        self.0
    }

    pub fn celsius(self) -> f64 {
        self.0 - 273.15
    }

    pub const ROOM_TEMP: Self = Temperature(293.0);
}

Quantity!(Power);
Quantity!(Frequency);
impl Frequency {

    #[allow(non_snake_case)]
    pub const fn from_MHz(n: f64) -> Self {
        Frequency(n * 1000.0 * 1000.0)
    }

    #[allow(non_snake_case)]
    pub const fn from_kHz(n: f64) -> Self {
        Frequency(n * 1000.0)
    }

    #[allow(non_snake_case)]
    pub fn kHz(self) -> f64 {
        self.0 / 1000.0
    }

    /// If this is the frequency of a light wave in air, this function returns the wavelength.
    pub fn light_wavelength(self) -> Length {
        Speed::LIGHTSPEED_AIR / self
    }
}

// Internally this is dB Watts not milli-watts
pub type Dbm = Db<Power>;
impl Dbm {
    pub const fn from_dbm(n: f64) -> Self {
        Self::from_db(n - 30.0)
    }

    pub const fn dbm(self) -> f64 {
        self.0 + 30.0
    }
}

pub type Dbf = Db<f64>;
impl Dbf {
    /// Specifically allowed because this is unitless
    pub const fn from_db_value(n: f64) -> Self {
        Db::from_db(n)
    }
}

Quantity!(DbPerLength);

impl DbPerLength {
    pub fn from_db_per_metre(n: f64) -> Self {
        Db::from_db(n) / Length::from_metres(1.0)
    }
}

Quantity!(Energy);
Quantity!(EnergyPerTemprature);
Quantity!(Speed);

/// Metres per second
pub const MPS : Speed = Speed::from_metres_per_second(1.0);

impl Speed {
    const LIGHTSPEED_AIR : Speed = Speed(299702547.0);

    pub const fn from_metres_per_second(n: f64) -> Self {
        Speed(n)
    }
}

DivRelation!(Speed, Frequency, Length);
MulRelation!(Speed, Time, Length);

pub const BOLTZMANN: EnergyPerTemprature = EnergyPerTemprature(1.380649E-23);

MulRelation!(EnergyPerTemprature, Temperature, Energy);
MulRelation!(Energy, Frequency, Power);
DivRelation!(f64, Frequency, Time);
DivRelation!(Dbf, Length, DbPerLength);
