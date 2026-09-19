#![no_std]
#![forbid(unsafe_code)]

pub mod plasticity;

use core::ops::{Add, Mul, Sub};

pub const AL_PN_COUNT: usize = 50;
pub const MB_KC_COUNT: usize = 2000;
pub const KC_ACTIVE_SPARSITY_PERCENT: usize = 5;
pub const EB_RING_COLUMNS: usize = 16;
pub const DN_COUNT: usize = 1300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Neuromodulator {
    Dopamine,
    Octopamine,
    Serotonin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Valence {
    Aversive,
    Appetitive,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CircuitModule {
    AntennalLobe,
    MushroomBody,
    CentralComplex,
    DescendingBottleneck,
}

impl CircuitModule {
    pub const fn population(self) -> usize {
        match self {
            Self::AntennalLobe => AL_PN_COUNT,
            Self::MushroomBody => MB_KC_COUNT,
            Self::CentralComplex => EB_RING_COLUMNS,
            Self::DescendingBottleneck => DN_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HeadingQ8(u8);

impl HeadingQ8 {
    pub const UNITS_PER_TURN: u32 = 256;

    pub const fn from_units(units: u8) -> Self {
        Self(units)
    }

    pub const fn units(self) -> u8 {
        self.0
    }

    pub const fn from_degrees(degrees: u16) -> Self {
        Self((((degrees as u32 % 360) * Self::UNITS_PER_TURN) / 360) as u8)
    }

    pub const fn degrees(self) -> u16 {
        ((self.0 as u32 * 360) / Self::UNITS_PER_TURN) as u16
    }

    pub const fn to_angle_q16(self) -> AngleQ16 {
        AngleQ16((self.0 as u16) << 8)
    }
}

impl Add for HeadingQ8 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for HeadingQ8 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AngleQ16(u16);

impl AngleQ16 {
    pub const UNITS_PER_TURN: u32 = 65536;

    pub const fn from_units(units: u16) -> Self {
        Self(units)
    }

    pub const fn units(self) -> u16 {
        self.0
    }

    pub const fn from_degrees(degrees: u16) -> Self {
        Self((((degrees as u32 % 360) * Self::UNITS_PER_TURN) / 360) as u16)
    }

    pub const fn degrees(self) -> u16 {
        ((self.0 as u32 * 360) / Self::UNITS_PER_TURN) as u16
    }

    pub const fn to_heading_q8(self) -> HeadingQ8 {
        HeadingQ8((self.0 >> 8) as u8)
    }
}

impl Add for AngleQ16 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for AngleQ16 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SynapticWeightQ16(i16);

impl SynapticWeightQ16 {
    pub const FRACTIONAL_BITS: u32 = 8;
    pub const MAX: Self = Self(i16::MAX);
    pub const MIN: Self = Self(i16::MIN);
    pub const ZERO: Self = Self(0);

    pub const fn from_raw(raw: i16) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> i16 {
        self.0
    }

    pub fn from_millis(millis: i32) -> Self {
        let scaled = millis.saturating_mul(1 << Self::FRACTIONAL_BITS) / 1000;
        Self(scaled.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
    }

    pub const fn from_inference_byte(byte: i8) -> Self {
        Self((byte as i16) << Self::FRACTIONAL_BITS)
    }

    pub const fn inference_byte(self) -> i8 {
        (self.0 >> Self::FRACTIONAL_BITS) as i8
    }
}

impl Add for SynapticWeightQ16 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl Sub for SynapticWeightQ16 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl Mul for SynapticWeightQ16 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let product = ((self.0 as i32) * (rhs.0 as i32)) >> Self::FRACTIONAL_BITS;
        Self(product.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn population_constants_match_biology_anchors() {
        assert_eq!(AL_PN_COUNT, 50);
        assert_eq!(MB_KC_COUNT, 2000);
        assert_eq!(KC_ACTIVE_SPARSITY_PERCENT, 5);
        assert_eq!(EB_RING_COLUMNS, 16);
        assert_eq!(DN_COUNT, 1300);
        assert_eq!(MB_KC_COUNT * KC_ACTIVE_SPARSITY_PERCENT / 100, 100);
    }

    #[test]
    fn circuit_modules_map_to_their_populations() {
        assert_eq!(CircuitModule::AntennalLobe.population(), AL_PN_COUNT);
        assert_eq!(CircuitModule::MushroomBody.population(), MB_KC_COUNT);
        assert_eq!(CircuitModule::CentralComplex.population(), EB_RING_COLUMNS);
        assert_eq!(CircuitModule::DescendingBottleneck.population(), DN_COUNT);
    }

    #[test]
    fn domain_enum_variants_are_distinct() {
        assert_ne!(Neuromodulator::Dopamine, Neuromodulator::Octopamine);
        assert_ne!(Neuromodulator::Dopamine, Neuromodulator::Serotonin);
        assert_ne!(Neuromodulator::Octopamine, Neuromodulator::Serotonin);
        assert_ne!(Valence::Aversive, Valence::Appetitive);
        assert_ne!(Valence::Aversive, Valence::Neutral);
        assert_ne!(Valence::Appetitive, Valence::Neutral);
        assert_ne!(CircuitModule::AntennalLobe, CircuitModule::MushroomBody);
        assert_ne!(CircuitModule::MushroomBody, CircuitModule::CentralComplex);
        assert_ne!(
            CircuitModule::CentralComplex,
            CircuitModule::DescendingBottleneck
        );
    }

    #[test]
    fn heading_q8_maps_degree_anchors() {
        assert_eq!(HeadingQ8::from_degrees(0).units(), 0);
        assert_eq!(HeadingQ8::from_degrees(90).units(), 64);
        assert_eq!(HeadingQ8::from_degrees(180).units(), 128);
        assert_eq!(HeadingQ8::from_degrees(270).units(), 192);
        assert_eq!(HeadingQ8::from_degrees(359).degrees(), 358);
    }

    #[test]
    fn heading_q8_wraps_past_full_turn() {
        assert_eq!(HeadingQ8::from_degrees(360), HeadingQ8::from_degrees(0));
        assert_eq!(HeadingQ8::from_degrees(721), HeadingQ8::from_degrees(1));
        assert_eq!(
            HeadingQ8::from_degrees(u16::MAX),
            HeadingQ8::from_degrees(15)
        );
    }

    #[test]
    fn heading_q8_addition_wraps_at_boundary() {
        let two_hundred = HeadingQ8::from_units(200);
        let one_hundred = HeadingQ8::from_units(100);
        assert_eq!(two_hundred + one_hundred, HeadingQ8::from_units(44));
        assert_eq!(
            HeadingQ8::from_units(255) + HeadingQ8::from_units(1),
            HeadingQ8::from_units(0)
        );
        assert_eq!(
            HeadingQ8::from_units(128) + HeadingQ8::from_units(128),
            HeadingQ8::from_units(0)
        );
    }

    #[test]
    fn heading_q8_subtraction_wraps_across_zero() {
        assert_eq!(
            HeadingQ8::from_units(0) - HeadingQ8::from_units(1),
            HeadingQ8::from_units(255)
        );
        assert_eq!(
            HeadingQ8::from_units(10) - HeadingQ8::from_units(20),
            HeadingQ8::from_units(246)
        );
        assert_eq!(
            HeadingQ8::from_units(7) - HeadingQ8::from_units(7),
            HeadingQ8::from_units(0)
        );
    }

    #[test]
    fn heading_q8_degree_truncation_stays_in_bounds() {
        for units in 0..=u8::MAX {
            let heading = HeadingQ8::from_units(units);
            let degrees = heading.degrees();
            assert!(degrees < 360);
            let back = HeadingQ8::from_degrees(degrees).units();
            assert!(u32::from(units) - u32::from(back) <= 1);
        }
    }

    #[test]
    fn heading_q8_promotes_losslessly_to_angle_q16() {
        for units in 0..=u8::MAX {
            let promoted = HeadingQ8::from_units(units).to_angle_q16();
            assert_eq!(promoted.units(), u16::from(units) << 8);
            assert_eq!(promoted.to_heading_q8().units(), units);
        }
        assert_eq!(
            HeadingQ8::from_units(128).to_angle_q16(),
            AngleQ16::from_units(32768)
        );
    }

    #[test]
    fn angle_q16_maps_degree_anchors() {
        assert_eq!(AngleQ16::from_degrees(0).units(), 0);
        assert_eq!(AngleQ16::from_degrees(90).units(), 16384);
        assert_eq!(AngleQ16::from_degrees(180).units(), 32768);
        assert_eq!(AngleQ16::from_degrees(270).units(), 49152);
        assert_eq!(AngleQ16::from_degrees(359).units(), 65353);
    }

    #[test]
    fn angle_q16_wraps_past_full_turn() {
        assert_eq!(AngleQ16::from_degrees(360), AngleQ16::from_degrees(0));
        assert_eq!(AngleQ16::from_degrees(721), AngleQ16::from_degrees(1));
        assert_eq!(AngleQ16::from_degrees(u16::MAX), AngleQ16::from_degrees(15));
    }

    #[test]
    fn angle_q16_addition_wraps_at_boundary() {
        assert_eq!(
            AngleQ16::from_units(60000) + AngleQ16::from_units(10000),
            AngleQ16::from_units(4464)
        );
        assert_eq!(
            AngleQ16::from_units(32768) + AngleQ16::from_units(32768),
            AngleQ16::from_units(0)
        );
        assert_eq!(
            AngleQ16::from_units(65535) + AngleQ16::from_units(1),
            AngleQ16::from_units(0)
        );
    }

    #[test]
    fn angle_q16_subtraction_wraps_across_zero() {
        assert_eq!(
            AngleQ16::from_units(0) - AngleQ16::from_units(1),
            AngleQ16::from_units(65535)
        );
        assert_eq!(
            AngleQ16::from_units(1000) - AngleQ16::from_units(2000),
            AngleQ16::from_units(64536)
        );
        assert_eq!(
            AngleQ16::from_units(9) - AngleQ16::from_units(9),
            AngleQ16::from_units(0)
        );
    }

    #[test]
    fn angle_q16_degree_truncation_stays_in_bounds() {
        for degrees in 0..360u16 {
            let angle = AngleQ16::from_degrees(degrees);
            let back = angle.degrees();
            assert!(back <= degrees);
            assert!(degrees - back <= 1);
        }
    }

    #[test]
    fn angle_q16_demotion_keeps_the_high_byte() {
        for units in 0..=u16::MAX {
            let demoted = AngleQ16::from_units(units).to_heading_q8();
            assert_eq!(demoted.units(), (units >> 8) as u8);
            assert_eq!(demoted.to_angle_q16().units(), units & 0xFF00);
        }
        assert_eq!(AngleQ16::from_units(0x1234).to_heading_q8().units(), 0x12);
    }

    #[test]
    fn angle_q16_resolves_steps_below_heading_q8_resolution() {
        let one_step = AngleQ16::from_units(0) + AngleQ16::from_units(1);
        assert_eq!(one_step.units(), 1);
        assert_eq!(one_step.to_heading_q8(), HeadingQ8::from_units(0));
        let full_circle_in_steps = AngleQ16::from_units(65535) + AngleQ16::from_units(1);
        assert_eq!(full_circle_in_steps, AngleQ16::from_units(0));
    }

    #[test]
    fn synaptic_weight_q8_8_scale_anchors() {
        assert_eq!(SynapticWeightQ16::FRACTIONAL_BITS, 8);
        assert_eq!(SynapticWeightQ16::ZERO.raw(), 0);
        assert_eq!(SynapticWeightQ16::MAX.raw(), i16::MAX);
        assert_eq!(SynapticWeightQ16::MIN.raw(), i16::MIN);
        assert!(SynapticWeightQ16::MIN < SynapticWeightQ16::ZERO);
        assert!(SynapticWeightQ16::ZERO < SynapticWeightQ16::MAX);
    }

    #[test]
    fn synaptic_weight_from_millis_quantizes_to_q8_8() {
        assert_eq!(SynapticWeightQ16::from_millis(1000).raw(), 256);
        assert_eq!(SynapticWeightQ16::from_millis(500).raw(), 128);
        assert_eq!(SynapticWeightQ16::from_millis(250).raw(), 64);
        assert_eq!(SynapticWeightQ16::from_millis(-500).raw(), -128);
        assert_eq!(SynapticWeightQ16::from_millis(1).raw(), 0);
        assert_eq!(SynapticWeightQ16::from_millis(-1).raw(), 0);
    }

    #[test]
    fn synaptic_weight_from_millis_saturates_far_range() {
        assert_eq!(
            SynapticWeightQ16::from_millis(200000),
            SynapticWeightQ16::MAX
        );
        assert_eq!(
            SynapticWeightQ16::from_millis(-200000),
            SynapticWeightQ16::MIN
        );
        assert_eq!(
            SynapticWeightQ16::from_millis(i32::MAX),
            SynapticWeightQ16::MAX
        );
        assert_eq!(
            SynapticWeightQ16::from_millis(i32::MIN),
            SynapticWeightQ16::MIN
        );
    }

    #[test]
    fn synaptic_weight_arithmetic_saturates_at_the_box_interval() {
        let one_lsb = SynapticWeightQ16::from_raw(1);
        assert_eq!(SynapticWeightQ16::MAX + one_lsb, SynapticWeightQ16::MAX);
        assert_eq!(
            SynapticWeightQ16::MAX + SynapticWeightQ16::MAX,
            SynapticWeightQ16::MAX
        );
        assert_eq!(SynapticWeightQ16::MIN - one_lsb, SynapticWeightQ16::MIN);
        assert_eq!(
            SynapticWeightQ16::MIN - SynapticWeightQ16::MIN,
            SynapticWeightQ16::ZERO
        );
        assert_eq!(
            SynapticWeightQ16::from_raw(128) + SynapticWeightQ16::from_raw(128),
            SynapticWeightQ16::from_raw(256)
        );
        assert_eq!(
            SynapticWeightQ16::from_raw(300) - SynapticWeightQ16::from_raw(100),
            SynapticWeightQ16::from_raw(200)
        );
    }

    #[test]
    fn synaptic_weight_products_shift_back_to_q8_8() {
        let half = SynapticWeightQ16::from_raw(128);
        assert_eq!(half * half, SynapticWeightQ16::from_raw(64));
        assert_eq!(
            SynapticWeightQ16::from_raw(512) * SynapticWeightQ16::from_raw(64),
            SynapticWeightQ16::from_raw(128)
        );
        assert_eq!(
            SynapticWeightQ16::from_raw(-512) * SynapticWeightQ16::from_raw(128),
            SynapticWeightQ16::from_raw(-256)
        );
    }

    #[test]
    fn synaptic_weight_products_saturate_on_overflow() {
        assert_eq!(
            SynapticWeightQ16::from_raw(512) * SynapticWeightQ16::from_raw(16384),
            SynapticWeightQ16::MAX
        );
        assert_eq!(
            SynapticWeightQ16::from_raw(-513) * SynapticWeightQ16::from_raw(16384),
            SynapticWeightQ16::MIN
        );
        assert_eq!(
            SynapticWeightQ16::MIN * SynapticWeightQ16::MIN,
            SynapticWeightQ16::MAX
        );
        assert_eq!(
            SynapticWeightQ16::MAX * SynapticWeightQ16::MIN,
            SynapticWeightQ16::MIN
        );
    }

    #[test]
    fn synaptic_weight_inference_byte_spans_256_master_lsbs() {
        assert_eq!(SynapticWeightQ16::from_raw(0).inference_byte(), 0);
        assert_eq!(SynapticWeightQ16::from_raw(255).inference_byte(), 0);
        assert_eq!(SynapticWeightQ16::from_raw(256).inference_byte(), 1);
        assert_eq!(SynapticWeightQ16::from_raw(-1).inference_byte(), -1);
        assert_eq!(SynapticWeightQ16::from_raw(-256).inference_byte(), -1);
        for byte in -128..=127i32 {
            let master = SynapticWeightQ16::from_raw((byte * 256) as i16);
            assert_eq!(i32::from(master.inference_byte()), byte);
        }
    }

    #[test]
    fn synaptic_weight_master_rebuilds_from_inference_byte() {
        assert_eq!(
            SynapticWeightQ16::from_inference_byte(0),
            SynapticWeightQ16::ZERO
        );
        assert_eq!(SynapticWeightQ16::from_inference_byte(1).raw(), 256);
        assert_eq!(
            SynapticWeightQ16::from_inference_byte(-128),
            SynapticWeightQ16::MIN
        );
        assert_eq!(SynapticWeightQ16::from_inference_byte(127).raw(), 32512);
        for byte in -128..=127i32 {
            let master = SynapticWeightQ16::from_inference_byte(byte as i8);
            assert_eq!(i32::from(master.inference_byte()), byte);
        }
    }
}
