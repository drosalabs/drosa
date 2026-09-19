use crate::SynapticWeightQ16;

pub const MAX_KENYON_CELLS: usize = 2048;
pub const MAX_OUTPUT_ROWS: usize = 21;
pub const MAX_LEARNING_RATE_RAW: u16 = 512;
pub const UPDATE_FRACTIONAL_BITS: u32 = 22;
const UPDATE_FRACTION_MASK: u32 = (1 << UPDATE_FRACTIONAL_BITS) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlasticityError {
    InvalidFraction,
    InvalidDopamine,
    InvalidLearningRate,
    InvalidWeightBounds,
    InvalidDimensions,
    InitialWeightOutOfBounds,
    InvalidActiveSet,
    EventCounterExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FractionQ15(u16);

impl FractionQ15 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(32768);

    pub const fn from_raw(raw: u16) -> Result<Self, PlasticityError> {
        if raw > Self::ONE.0 {
            return Err(PlasticityError::InvalidFraction);
        }
        Ok(Self(raw))
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    fn decay(self, retention: Self) -> Self {
        Self(((u32::from(self.0) * u32::from(retention.0)) >> 15) as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DopamineQ7(i16);

impl DopamineQ7 {
    pub const ZERO: Self = Self(0);
    pub const DEPRESS: Self = Self(128);
    pub const POTENTIATE: Self = Self(-128);

    pub const fn from_raw(raw: i16) -> Result<Self, PlasticityError> {
        if raw < -128 || raw > 128 {
            return Err(PlasticityError::InvalidDopamine);
        }
        Ok(Self(raw))
    }

    pub const fn raw(self) -> i16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlasticityConfig {
    retention: FractionQ15,
    learning_rate_raw: u16,
    minimum: SynapticWeightQ16,
    maximum: SynapticWeightQ16,
}

impl PlasticityConfig {
    pub fn new(
        retention: FractionQ15,
        learning_rate_raw: u16,
        minimum: SynapticWeightQ16,
        maximum: SynapticWeightQ16,
    ) -> Result<Self, PlasticityError> {
        if learning_rate_raw > MAX_LEARNING_RATE_RAW {
            return Err(PlasticityError::InvalidLearningRate);
        }
        if minimum > maximum {
            return Err(PlasticityError::InvalidWeightBounds);
        }
        Ok(Self {
            retention,
            learning_rate_raw,
            minimum,
            maximum,
        })
    }

    pub fn update_weight(
        self,
        weight: SynapticWeightQ16,
        eligibility: FractionQ15,
        dopamine: DopamineQ7,
        rounding_sample: u32,
    ) -> SynapticWeightQ16 {
        let product = u32::from(self.learning_rate_raw)
            * u32::from(eligibility.raw())
            * u32::from(dopamine.raw().unsigned_abs());
        let fractional = product & UPDATE_FRACTION_MASK;
        let magnitude = (product >> UPDATE_FRACTIONAL_BITS)
            + u32::from((rounding_sample & UPDATE_FRACTION_MASK) < fractional);
        let delta = (magnitude as i32) * i32::from(dopamine.raw().signum());
        let updated = i32::from(weight.raw()) - delta;
        SynapticWeightQ16::from_raw(
            updated.clamp(i32::from(self.minimum.raw()), i32::from(self.maximum.raw())) as i16,
        )
    }
}

impl Default for PlasticityConfig {
    fn default() -> Self {
        Self {
            retention: FractionQ15(32703),
            learning_rate_raw: 128,
            minimum: SynapticWeightQ16::ZERO,
            maximum: SynapticWeightQ16::MAX,
        }
    }
}

pub fn rounding_sample(seed: u32, event: u32, synapse: u32) -> u32 {
    let mut value = seed
        .wrapping_add(event.wrapping_mul(0x9e37_79b9))
        .wrapping_add(synapse.wrapping_mul(0x85eb_ca6b));
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb_352d);
    value = (value ^ (value >> 15)).wrapping_mul(0x846c_a68b);
    (value ^ (value >> 16)) & UPDATE_FRACTION_MASK
}

#[derive(Debug, PartialEq, Eq)]
pub struct Plasticity<const K: usize, const M: usize> {
    weights: [[SynapticWeightQ16; K]; M],
    eligibility: [FractionQ15; K],
    config: PlasticityConfig,
    seed: u32,
    reinforcement_events: u32,
}

impl<const K: usize, const M: usize> Plasticity<K, M> {
    pub fn new(
        config: PlasticityConfig,
        initial_weight: SynapticWeightQ16,
        seed: u32,
    ) -> Result<Self, PlasticityError> {
        if K == 0 || K > MAX_KENYON_CELLS || M == 0 || M > MAX_OUTPUT_ROWS {
            return Err(PlasticityError::InvalidDimensions);
        }
        if initial_weight < config.minimum || initial_weight > config.maximum {
            return Err(PlasticityError::InitialWeightOutOfBounds);
        }
        Ok(Self {
            weights: [[initial_weight; K]; M],
            eligibility: [FractionQ15::ZERO; K],
            config,
            seed,
            reinforcement_events: 0,
        })
    }

    pub fn weights(&self) -> &[[SynapticWeightQ16; K]; M] {
        &self.weights
    }

    pub fn eligibility(&self) -> &[FractionQ15; K] {
        &self.eligibility
    }

    pub fn reinforcement_events(&self) -> u32 {
        self.reinforcement_events
    }

    pub fn forward(&self, active: &[usize]) -> Result<[i32; M], PlasticityError> {
        Self::validate_active(active)?;
        Ok(self.forward_validated(active))
    }

    pub fn step(
        &mut self,
        active: &[usize],
        dopamine: &[DopamineQ7; M],
    ) -> Result<[i32; M], PlasticityError> {
        Self::validate_active(active)?;
        let reinforced = dopamine.iter().any(|pulse| *pulse != DopamineQ7::ZERO);
        if reinforced && self.reinforcement_events == u32::MAX {
            return Err(PlasticityError::EventCounterExhausted);
        }
        let output = self.forward_validated(active);
        for trace in &mut self.eligibility {
            *trace = trace.decay(self.config.retention);
        }
        for &kc in active {
            self.eligibility[kc] = FractionQ15::ONE;
        }
        if reinforced {
            for (row_index, (row, pulse)) in self.weights.iter_mut().zip(dopamine).enumerate() {
                if *pulse == DopamineQ7::ZERO {
                    continue;
                }
                for (kc, (weight, trace)) in row.iter_mut().zip(&self.eligibility).enumerate() {
                    let sample = rounding_sample(
                        self.seed,
                        self.reinforcement_events,
                        (row_index * K + kc) as u32,
                    );
                    *weight = self.config.update_weight(*weight, *trace, *pulse, sample);
                }
            }
            self.reinforcement_events += 1;
        }
        Ok(output)
    }

    fn validate_active(active: &[usize]) -> Result<(), PlasticityError> {
        if active.iter().any(|&kc| kc >= K) || active.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PlasticityError::InvalidActiveSet);
        }
        Ok(())
    }

    fn forward_validated(&self, active: &[usize]) -> [i32; M] {
        let mut output = [0; M];
        for (sum, row) in output.iter_mut().zip(&self.weights) {
            for &kc in active {
                *sum += i32::from(row[kc].inference_byte());
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(retention: u16, learning_rate: u16) -> PlasticityConfig {
        PlasticityConfig::new(
            FractionQ15::from_raw(retention).unwrap(),
            learning_rate,
            SynapticWeightQ16::ZERO,
            SynapticWeightQ16::MAX,
        )
        .unwrap()
    }

    fn model<const K: usize, const M: usize>(
        retention: u16,
        learning_rate: u16,
    ) -> Plasticity<K, M> {
        Plasticity::new(
            config(retention, learning_rate),
            SynapticWeightQ16::from_raw(2303),
            7,
        )
        .unwrap()
    }

    #[test]
    fn one_shot_changes_valence_only_for_the_paired_cue_and_row() {
        let mut learning = model::<2000, 2>(0, 512);
        let paired: [usize; 100] = core::array::from_fn(|kc| kc);
        let control: [usize; 100] = core::array::from_fn(|kc| kc + 100);
        assert_eq!(learning.forward(&paired).unwrap(), [800, 800]);
        let before_update = learning
            .step(&paired, &[DopamineQ7::DEPRESS, DopamineQ7::ZERO])
            .unwrap();
        assert_eq!(before_update, [800, 800]);
        assert_eq!(learning.forward(&paired).unwrap(), [600, 800]);
        assert_eq!(learning.forward(&control).unwrap(), [800, 800]);
        assert_eq!(learning.weights()[0][0].raw(), 1791);
        assert_eq!(learning.weights()[0][100].raw(), 2303);
        assert_eq!(learning.reinforcement_events(), 1);
    }

    #[test]
    fn omission_teacher_extinction_adds_an_opposing_memory_without_erasure() {
        let mut learning = model::<4, 2>(0, 512);
        learning
            .step(&[0], &[DopamineQ7::DEPRESS, DopamineQ7::ZERO])
            .unwrap();
        let aversive_memory = learning.weights()[0][0];
        assert_eq!(learning.forward(&[0]).unwrap(), [6, 8]);
        learning
            .step(&[0], &[DopamineQ7::ZERO, DopamineQ7::DEPRESS])
            .unwrap();
        assert_eq!(learning.forward(&[0]).unwrap(), [6, 6]);
        assert_eq!(learning.weights()[0][0], aversive_memory);
        assert_eq!(learning.forward(&[1]).unwrap(), [8, 8]);
    }

    #[test]
    fn cue_alone_and_elapsed_time_do_not_erase_memory() {
        let mut learning = model::<4, 1>(16384, 512);
        learning.step(&[0], &[DopamineQ7::DEPRESS]).unwrap();
        let learned = *learning.weights();
        for _ in 0..20 {
            learning.step(&[0], &[DopamineQ7::ZERO]).unwrap();
        }
        for _ in 0..32 {
            learning.step(&[], &[DopamineQ7::ZERO]).unwrap();
        }
        assert_eq!(learning.weights(), &learned);
        assert_eq!(learning.eligibility(), &[FractionQ15::ZERO; 4]);
        assert_eq!(learning.reinforcement_events(), 1);
    }

    #[test]
    fn delayed_reward_uses_recently_active_not_currently_active_inputs() {
        let mut learning = model::<2, 1>(16384, 128);
        learning.step(&[0], &[DopamineQ7::ZERO]).unwrap();
        assert_eq!(learning.eligibility()[0], FractionQ15::ONE);
        learning.step(&[], &[DopamineQ7::DEPRESS]).unwrap();
        assert_eq!(learning.eligibility()[0].raw(), 16384);
        assert_eq!(learning.weights()[0][0].raw(), 2303 - 64);
        assert_eq!(learning.weights()[0][1].raw(), 2303);
    }

    #[test]
    fn backward_and_expired_pairings_do_not_learn() {
        let mut backward = model::<2, 1>(16384, 128);
        backward.step(&[], &[DopamineQ7::DEPRESS]).unwrap();
        backward.step(&[0], &[DopamineQ7::ZERO]).unwrap();
        assert_eq!(backward.weights()[0][0].raw(), 2303);
        let mut expired = model::<2, 1>(16384, 128);
        expired.step(&[0], &[DopamineQ7::ZERO]).unwrap();
        for _ in 0..16 {
            expired.step(&[], &[DopamineQ7::ZERO]).unwrap();
        }
        expired.step(&[], &[DopamineQ7::DEPRESS]).unwrap();
        assert_eq!(expired.weights()[0][0].raw(), 2303);
    }

    #[test]
    fn repeated_activity_replaces_rather_than_accumulates_eligibility() {
        let mut learning = model::<2, 1>(32768, 128);
        for _ in 0..100 {
            learning.step(&[0], &[DopamineQ7::ZERO]).unwrap();
        }
        assert_eq!(
            learning.eligibility(),
            &[FractionQ15::ONE, FractionQ15::ZERO]
        );
    }

    #[test]
    fn default_trace_retains_seconds_scale_credit_and_reaches_zero() {
        let mut trace = FractionQ15::ONE;
        for _ in 0..500 {
            trace = trace.decay(PlasticityConfig::default().retention);
        }
        assert!((11900..=12200).contains(&trace.raw()));
        for _ in 0..5000 {
            trace = trace.decay(PlasticityConfig::default().retention);
        }
        assert_eq!(trace, FractionQ15::ZERO);
    }

    #[test]
    fn integer_master_accumulates_sub_byte_updates_without_stalling() {
        let mut learning = model::<1, 1>(0, 1);
        for _ in 0..255 {
            learning.step(&[0], &[DopamineQ7::DEPRESS]).unwrap();
        }
        assert_eq!(learning.weights()[0][0].raw(), 2048);
        assert_eq!(learning.forward(&[0]).unwrap(), [8]);
        learning.step(&[0], &[DopamineQ7::DEPRESS]).unwrap();
        assert_eq!(learning.weights()[0][0].raw(), 2047);
        assert_eq!(learning.forward(&[0]).unwrap(), [7]);
    }

    #[test]
    fn fractional_master_update_uses_exact_rounding_threshold() {
        let settings = config(32768, 1);
        let weight = SynapticWeightQ16::from_raw(2303);
        let half = FractionQ15::from_raw(16384).unwrap();
        let threshold = 1 << (UPDATE_FRACTIONAL_BITS - 1);
        assert_eq!(
            settings
                .update_weight(weight, half, DopamineQ7::DEPRESS, threshold - 1)
                .raw(),
            2302
        );
        assert_eq!(
            settings
                .update_weight(weight, half, DopamineQ7::DEPRESS, threshold)
                .raw(),
            2303
        );
        assert_eq!(
            settings
                .update_weight(weight, half, DopamineQ7::POTENTIATE, threshold - 1)
                .raw(),
            2304
        );
        assert_eq!(
            settings
                .update_weight(weight, half, DopamineQ7::POTENTIATE, threshold)
                .raw(),
            2303
        );
        assert_eq!(
            settings.update_weight(weight, FractionQ15::ZERO, DopamineQ7::DEPRESS, 0),
            weight
        );
    }

    #[test]
    fn seeded_sub_master_updates_accumulate_and_replay_exactly() {
        let mut first = model::<1, 1>(32768, 1);
        let mut replay = model::<1, 1>(32768, 1);
        let pulse = [DopamineQ7::from_raw(64).unwrap()];
        for _ in 0..4096 {
            first.step(&[0], &pulse).unwrap();
            replay.step(&[0], &pulse).unwrap();
        }
        assert_eq!(first, replay);
        let total_depression = 2303 - first.weights()[0][0].raw();
        assert!((1900..=2200).contains(&total_depression));
        assert!(first.forward(&[0]).unwrap()[0] < 8);
    }

    #[test]
    fn full_range_updates_clamp_before_narrowing() {
        let settings = PlasticityConfig::new(
            FractionQ15::ONE,
            MAX_LEARNING_RATE_RAW,
            SynapticWeightQ16::MIN,
            SynapticWeightQ16::MAX,
        )
        .unwrap();
        assert_eq!(
            settings.update_weight(
                SynapticWeightQ16::MIN,
                FractionQ15::ONE,
                DopamineQ7::DEPRESS,
                0
            ),
            SynapticWeightQ16::MIN
        );
        assert_eq!(
            settings.update_weight(
                SynapticWeightQ16::MAX,
                FractionQ15::ONE,
                DopamineQ7::POTENTIATE,
                0
            ),
            SynapticWeightQ16::MAX
        );
        assert_eq!(
            settings
                .update_weight(
                    SynapticWeightQ16::ZERO,
                    FractionQ15::ONE,
                    DopamineQ7::DEPRESS,
                    0
                )
                .raw(),
            -512
        );
        assert_eq!(
            settings
                .update_weight(
                    SynapticWeightQ16::ZERO,
                    FractionQ15::ONE,
                    DopamineQ7::POTENTIATE,
                    0
                )
                .raw(),
            512
        );
    }

    #[test]
    fn default_depression_never_makes_excitatory_weights_negative() {
        let mut learning = model::<1, 1>(0, 512);
        for _ in 0..10 {
            learning.step(&[0], &[DopamineQ7::DEPRESS]).unwrap();
        }
        assert_eq!(learning.weights()[0][0], SynapticWeightQ16::ZERO);
        learning.step(&[0], &[DopamineQ7::POTENTIATE]).unwrap();
        assert_eq!(learning.weights()[0][0].raw(), 512);
    }

    #[test]
    fn invalid_inputs_are_rejected_before_state_mutation() {
        for active in [&[2][..], &[0, 0][..], &[1, 0][..]] {
            let mut learning = model::<2, 1>(32768, 128);
            let original = model::<2, 1>(32768, 128);
            assert_eq!(
                learning.step(active, &[DopamineQ7::DEPRESS]),
                Err(PlasticityError::InvalidActiveSet)
            );
            assert_eq!(
                learning.forward(active),
                Err(PlasticityError::InvalidActiveSet)
            );
            assert_eq!(learning, original);
        }
    }

    #[test]
    fn configuration_and_signal_bounds_are_checked() {
        assert_eq!(
            FractionQ15::from_raw(32769),
            Err(PlasticityError::InvalidFraction)
        );
        assert_eq!(
            DopamineQ7::from_raw(i16::MIN),
            Err(PlasticityError::InvalidDopamine)
        );
        assert_eq!(
            DopamineQ7::from_raw(129),
            Err(PlasticityError::InvalidDopamine)
        );
        assert_eq!(
            PlasticityConfig::new(
                FractionQ15::ONE,
                513,
                SynapticWeightQ16::MIN,
                SynapticWeightQ16::MAX
            ),
            Err(PlasticityError::InvalidLearningRate)
        );
        assert_eq!(
            PlasticityConfig::new(
                FractionQ15::ONE,
                1,
                SynapticWeightQ16::MAX,
                SynapticWeightQ16::MIN
            ),
            Err(PlasticityError::InvalidWeightBounds)
        );
        assert_eq!(
            Plasticity::<0, 1>::new(config(0, 1), SynapticWeightQ16::ZERO, 0),
            Err(PlasticityError::InvalidDimensions)
        );
        assert_eq!(
            Plasticity::<1, 0>::new(config(0, 1), SynapticWeightQ16::ZERO, 0),
            Err(PlasticityError::InvalidDimensions)
        );
        assert_eq!(
            Plasticity::<1, 1>::new(config(0, 1), SynapticWeightQ16::MIN, 0),
            Err(PlasticityError::InitialWeightOutOfBounds)
        );
    }

    #[test]
    fn zero_learning_rate_freezes_weights() {
        let mut learning = model::<2, 1>(32768, 0);
        learning.step(&[0], &[DopamineQ7::DEPRESS]).unwrap();
        assert_eq!(learning.weights()[0][0].raw(), 2303);
        assert_eq!(learning.eligibility()[0], FractionQ15::ONE);
    }

    #[test]
    fn exhausted_event_counter_rejects_reinforcement_atomically() {
        let mut learning = model::<2, 1>(32768, 128);
        learning.reinforcement_events = u32::MAX;
        assert_eq!(
            learning.step(&[0], &[DopamineQ7::DEPRESS]),
            Err(PlasticityError::EventCounterExhausted)
        );
        assert_eq!(learning.eligibility(), &[FractionQ15::ZERO; 2]);
        assert_eq!(learning.weights()[0][0].raw(), 2303);
        learning.step(&[0], &[DopamineQ7::ZERO]).unwrap();
        assert_eq!(learning.reinforcement_events(), u32::MAX);
    }

    #[test]
    fn unsigned_update_matches_independent_i64_rational_oracle() {
        for rate in [0, 1, 127, 128, 511, 512] {
            let settings = PlasticityConfig::new(
                FractionQ15::ONE,
                rate,
                SynapticWeightQ16::MIN,
                SynapticWeightQ16::MAX,
            )
            .unwrap();
            for trace in [0, 1, 16383, 16384, 32766, 32767, 32768] {
                for dose in [-128, -127, -1, 0, 1, 127, 128] {
                    for weight in [i16::MIN, -32767, -1, 0, 1, 32766, i16::MAX] {
                        for sample in [0, 1, 2097152, 4194303] {
                            let product = i64::from(rate) * i64::from(trace) * i64::from(dose);
                            let magnitude = product.abs();
                            let step = magnitude / 4194304
                                + i64::from(i64::from(sample) < magnitude % 4194304);
                            let expected = (i64::from(weight) - product.signum() * step)
                                .clamp(i64::from(i16::MIN), i64::from(i16::MAX));
                            let actual = settings.update_weight(
                                SynapticWeightQ16::from_raw(weight),
                                FractionQ15::from_raw(trace).unwrap(),
                                DopamineQ7::from_raw(dose).unwrap(),
                                sample,
                            );
                            assert_eq!(i64::from(actual.raw()), expected);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn factorized_eligibility_uses_two_bytes_per_kenyon_cell() {
        assert_eq!(core::mem::size_of::<[FractionQ15; 2000]>(), 4000);
        assert_eq!(core::mem::size_of::<[FractionQ15; 2048]>(), 4096);
        assert_eq!(core::mem::size_of::<[[FractionQ15; 2048]; 21]>(), 86016);
    }

    #[test]
    fn sparse_forward_matches_dense_oracle_at_signed_extremes() {
        let settings = PlasticityConfig::new(
            FractionQ15::ONE,
            0,
            SynapticWeightQ16::MIN,
            SynapticWeightQ16::MAX,
        )
        .unwrap();
        for initial in [SynapticWeightQ16::MIN, SynapticWeightQ16::MAX] {
            let learning = Plasticity::<2048, 1>::new(settings, initial, 0).unwrap();
            let active: [usize; 2048] = core::array::from_fn(|kc| kc);
            let dense: i32 = learning.weights()[0]
                .iter()
                .map(|weight| i32::from(weight.inference_byte()))
                .sum();
            assert_eq!(learning.forward(&active).unwrap(), [dense]);
            assert_eq!(dense, i32::from(initial.inference_byte()) * 2048);
            assert_eq!(learning.forward(&[]).unwrap(), [0]);
        }
    }
}
