use drosa::SynapticWeightQ16;
use drosa::plasticity::{DopamineQ7, FractionQ15, PlasticityConfig, rounding_sample};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

const CHANNEL_ACTIVE_DEN: u64 = 10;
const CODING_PERCENT: u64 = 5;
const OUTPUT_ROWS: usize = 21;
const INITIAL_WEIGHT_RAW: i16 = 2304;
const LEARNING_RATE_RAW: u16 = 512;
const DIMENSION_CUES: usize = 1024;
const PAIR_SAMPLES: usize = 64;
const ASSOCIATION_ENSEMBLES: usize = 8;
const PERCEPTRON_EPOCHS: usize = 40;
const FULL_LADDER: [usize; 10] = [2, 4, 8, 16, 24, 32, 48, 64, 96, 128];
const FULL_REPLICATES: u32 = 30;
const FEEDBACK_GAIN_PER_CELL_Q8: i32 = 4;
const FEEDBACK_STEP_CLAMP_Q8: i32 = 64;
const FEEDBACK_INHIBITION_CEILING_Q8: i32 = 384;
const BASELINE_WIDTH: usize = 50;
const BASELINE_KC_COUNT: usize = 2048;
const BASELINE_CLAWS: usize = 6;
const PAIR_MARGIN_RELATIVE_TOLERANCE: f64 = 0.95;
const ASSOCIATION_ACCURACY_TOLERANCE: f64 = 0.02;
const ASSOCIATION_CAPACITY_STEP_TOLERANCE: usize = 8;
const ASSOCIATION_ACCURACY_FLOOR: f64 = 0.90;
const EXPANSION_ACCESS_FJ: f64 = 65.88;
const THRESHOLD_ACCESS_FJ: f64 = 64.8;
const TRACE_ACCESS_FJ: f64 = 69.6777;
const TOPK_ACCESS_FJ: f64 = 88.3281;
const READOUT_ACCESS_FJ: f64 = 87.84;
const THRESHOLD_ACCESSES_PER_KC: f64 = 5.0;
const TRACE_ACCESSES_PER_KC: f64 = 6.0;
const TOPK_ACCESSES_PER_KC: f64 = 4.0;
const READOUT_ACCESSES_PER_ACTIVE: f64 = 66.0;
const EXPANSION_COUNTER_ACCESSES: f64 = 4.0;
const FEEDBACK_ACCESSES_PER_TICK: f64 = 4.0;
const READOUT_FIXED_FJ: f64 = 55_800.0;
const CONTROL_FJ: f64 = 130_500.0;
const PRICED_SKIP_TICK_NJ: f64 = 5.2958;
const PRICED_ACTIVE_PER_TICK: f64 = 98.73;

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn next_below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }

    fn next_channel(&mut self) -> bool {
        self.next_u64().is_multiple_of(CHANNEL_ACTIVE_DEN)
    }
}

fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn stream_seed(width: u64, kc: u64, claws: u64, model: u64, replicate: u64, stream: u64) -> u64 {
    let mut h = 0x243f_6a88_85a3_08d3_u64;
    for field in [width, kc, claws, model, replicate, stream] {
        h = mix(h ^ field);
    }
    h
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InhibitionModel {
    RankSelect,
    FeedbackThreshold,
    Disabled,
}

impl InhibitionModel {
    fn label(self) -> &'static str {
        match self {
            Self::RankSelect => "rank_select",
            Self::FeedbackThreshold => "feedback_threshold",
            Self::Disabled => "disabled",
        }
    }

    fn index(self) -> u64 {
        match self {
            Self::RankSelect => 0,
            Self::FeedbackThreshold => 1,
            Self::Disabled => 2,
        }
    }

    fn all() -> [Self; 3] {
        [Self::RankSelect, Self::FeedbackThreshold, Self::Disabled]
    }
}

struct Calibration {
    threshold: u64,
    gate: u64,
}

fn binomial_mass(claws: usize, hits: usize) -> u128 {
    let mut choose: u128 = 1;
    for i in 0..hits {
        choose = choose * claws.wrapping_sub(i) as u128 / (i + 1) as u128;
    }
    choose * 9u128.pow(claws.wrapping_sub(hits) as u32)
}

fn tail_mass(claws: usize, from_hits: usize) -> u128 {
    (from_hits..=claws)
        .map(|hits| binomial_mass(claws, hits))
        .sum()
}

fn calibrate(claws: usize) -> Calibration {
    let total = 10u128.pow(claws as u32);
    let target = total / 100 * CODING_PERCENT as u128;
    let mut threshold = 0usize;
    for hits in 0..=claws {
        if tail_mass(claws, hits) >= target {
            threshold = hits;
        }
    }
    let above = tail_mass(claws, threshold + 1);
    let boundary = binomial_mass(claws, threshold);
    let gate = ((target - above) * 256 + boundary / 2) / boundary;
    if gate >= 256 {
        Calibration {
            threshold: threshold.saturating_sub(1) as u64,
            gate: 0,
        }
    } else {
        Calibration {
            threshold: threshold as u64,
            gate: gate as u64,
        }
    }
}

fn calibrated_marginal(claws: usize) -> f64 {
    let calibration = calibrate(claws);
    let total = 10u128.pow(claws as u32);
    let above = tail_mass(claws, calibration.threshold as usize + 1);
    let boundary = binomial_mass(claws, calibration.threshold as usize);
    (above + boundary * calibration.gate as u128 / 256) as f64 / total as f64
}

struct Expansion {
    width: usize,
    claw_count: usize,
    kc_count: usize,
    claw_channels: Vec<u8>,
    channel_masks: Vec<u64>,
    jitter: Vec<u8>,
    calibration: Calibration,
}

impl Expansion {
    fn new(width: usize, claw_count: usize, kc_count: usize, seed: u64) -> Self {
        assert!(claw_count <= width);
        assert!(width <= 64);
        let mut rng = SplitMix64::new(seed);
        let mut claw_channels = vec![0u8; kc_count * claw_count];
        let mut channel_masks = vec![0u64; kc_count];
        let mut jitter = vec![0u8; kc_count];
        let mut permutation: Vec<u8> = (0..width as u8).collect();
        for kc in 0..kc_count {
            for slot in 0..claw_count {
                let pick = rng.next_below((width - slot) as u64) as usize;
                permutation.swap(slot, slot + pick);
            }
            let mut mask = 0u64;
            for claw in 0..claw_count {
                let channel = permutation[claw];
                claw_channels[kc * claw_count + claw] = channel;
                mask |= 1u64 << channel;
            }
            channel_masks[kc] = mask;
            permutation.sort_unstable();
            jitter[kc] = 255 - rng.next_below(256) as u8;
        }
        Self {
            width,
            claw_count,
            kc_count,
            claw_channels,
            channel_masks,
            jitter,
            calibration: calibrate(claw_count),
        }
    }

    fn scores(&self, cue: u64, out: &mut Vec<u32>) {
        out.clear();
        out.extend(
            self.channel_masks
                .iter()
                .zip(&self.jitter)
                .map(|(&mask, &jitter)| ((mask & cue).count_ones() << 8) | u32::from(jitter)),
        );
    }
}

fn draw_cue(rng: &mut SplitMix64, width: usize) -> u64 {
    let mut cue = 0u64;
    for channel in 0..width {
        if rng.next_channel() {
            cue |= 1u64 << channel;
        }
    }
    cue
}

fn rank_target(kc_count: usize) -> usize {
    (kc_count * CODING_PERCENT as usize + 50) / 100
}

#[derive(Clone, Copy)]
struct FeedbackController {
    offset_q8: i32,
}

impl FeedbackController {
    fn new() -> Self {
        Self { offset_q8: 0 }
    }

    fn update(&mut self, active: usize, target: usize) {
        let error = active as i32 - target as i32;
        let commanded = (error * FEEDBACK_GAIN_PER_CELL_Q8)
            .clamp(-FEEDBACK_STEP_CLAMP_Q8, FEEDBACK_STEP_CLAMP_Q8);
        self.offset_q8 = commanded.clamp(0, FEEDBACK_INHIBITION_CEILING_Q8);
    }
}

fn activate(
    expansion: &Expansion,
    scores: &mut Vec<u32>,
    cue: u64,
    model: InhibitionModel,
    feedback_offset_q8: i32,
    active_out: &mut Vec<u32>,
) {
    expansion.scores(cue, scores);
    let base = (expansion.calibration.threshold * 256 + (255 - expansion.calibration.gate)) as i32
        + feedback_offset_q8;
    active_out.clear();
    active_out.extend(
        scores
            .iter()
            .enumerate()
            .filter(|&(_, score)| *score as i32 > base)
            .map(|(kc, _)| kc as u32),
    );
    if let InhibitionModel::RankSelect = model {
        let target = rank_target(expansion.kc_count);
        if active_out.len() > target {
            active_out.sort_by(|&a, &b| scores[b as usize].cmp(&scores[a as usize]));
            active_out.truncate(target);
            active_out.sort_unstable();
        }
    }
}

fn participation_ratio(batch: &[Vec<u32>], kc_count: usize) -> (f64, f64) {
    let samples = batch.len();
    if samples == 0 || kc_count == 0 {
        return (0.0, 0.0);
    }
    let words = kc_count.div_ceil(64);
    let mut bitmaps = vec![0u64; samples * words];
    let mut column_counts = vec![0u64; kc_count];
    for (sample, active) in batch.iter().enumerate() {
        for &kc in active {
            bitmaps[sample * words + kc as usize / 64] |= 1u64 << (kc % 64);
            column_counts[kc as usize] += 1;
        }
    }
    let means: Vec<f64> = column_counts
        .iter()
        .map(|&count| count as f64 / samples as f64)
        .collect();
    let mean_square_sum: f64 = means.iter().map(|mean| mean * mean).sum();
    let active_means: Vec<f64> = batch
        .iter()
        .map(|active| active.iter().map(|&kc| means[kc as usize]).sum())
        .collect();
    let mut trace = 0.0f64;
    let mut square_trace = 0.0f64;
    for row in 0..samples {
        let row_words = &bitmaps[row * words..row * words + words];
        for column in 0..=row {
            let column_words = &bitmaps[column * words..column * words + words];
            let overlap: u32 = row_words
                .iter()
                .zip(column_words)
                .map(|(&a, &b)| (a & b).count_ones())
                .sum();
            let centered =
                overlap as f64 - active_means[row] - active_means[column] + mean_square_sum;
            if row == column {
                trace += centered;
                square_trace += centered * centered;
            } else {
                square_trace += 2.0 * centered * centered;
            }
        }
    }
    let mean_active = batch.iter().map(Vec::len).sum::<usize>() as f64 / samples as f64;
    if square_trace <= 0.0 {
        return (0.0, mean_active);
    }
    ((trace * trace) / square_trace, mean_active)
}

fn pair_metrics(a: &[u32], b: &[u32]) -> (usize, f64, f64) {
    let mut overlap = 0usize;
    let mut i = 0usize;
    let mut j = 0usize;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                overlap += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let difference = a.len() + b.len() - 2 * overlap;
    let margin_abs = (difference as f64).sqrt() / 2.0;
    let denominator = (a.len() as f64).sqrt() + (b.len() as f64).sqrt();
    let margin_norm = if denominator == 0.0 {
        0.0
    } else {
        (difference as f64).sqrt() / denominator
    };
    (overlap, margin_abs, margin_norm)
}

fn shuffled_balanced_labels(rng: &mut SplitMix64, cue_count: usize) -> Vec<i8> {
    let mut labels: Vec<i8> = (0..cue_count)
        .map(|i| if i % 2 == 0 { -1 } else { 1 })
        .collect();
    for i in (1..labels.len()).rev() {
        let j = rng.next_below(i as u64 + 1) as usize;
        labels.swap(i, j);
    }
    labels
}

fn association_accuracy(
    expansion: &Expansion,
    model: InhibitionModel,
    seed: u64,
    cue_count: usize,
) -> f64 {
    let config = PlasticityConfig::new(
        FractionQ15::from_raw(0).unwrap(),
        LEARNING_RATE_RAW,
        SynapticWeightQ16::ZERO,
        SynapticWeightQ16::MAX,
    )
    .unwrap();
    let mut rng = SplitMix64::new(seed);
    let cues: Vec<u64> = (0..cue_count)
        .map(|_| draw_cue(&mut rng, expansion.width))
        .collect();
    let labels = shuffled_balanced_labels(&mut rng, cue_count);
    let mut approach_weights = vec![INITIAL_WEIGHT_RAW; expansion.kc_count];
    let mut avoid_weights = vec![INITIAL_WEIGHT_RAW; expansion.kc_count];
    let mut scores = Vec::with_capacity(expansion.kc_count);
    let mut active = Vec::new();
    let mut feedback = FeedbackController::new();
    let target = rank_target(expansion.kc_count);
    for (event, (cue, &label)) in cues.iter().zip(&labels).enumerate() {
        activate(
            expansion,
            &mut scores,
            *cue,
            model,
            feedback.offset_q8,
            &mut active,
        );
        if let InhibitionModel::FeedbackThreshold = model {
            feedback.update(active.len(), target);
        }
        let taught_row = if label < 0 {
            &mut approach_weights
        } else {
            &mut avoid_weights
        };
        let synapse_base = if label < 0 { 0 } else { expansion.kc_count };
        for &kc in &active {
            let index = kc as usize;
            let sample = rounding_sample(seed as u32, event as u32, (synapse_base + index) as u32);
            let updated = config.update_weight(
                SynapticWeightQ16::from_raw(taught_row[index]),
                FractionQ15::ONE,
                DopamineQ7::DEPRESS,
                sample,
            );
            taught_row[index] = updated.raw();
        }
    }
    let mut correct = 0usize;
    for (cue, &label) in cues.iter().zip(&labels) {
        activate(
            expansion,
            &mut scores,
            *cue,
            model,
            feedback.offset_q8,
            &mut active,
        );
        if let InhibitionModel::FeedbackThreshold = model {
            feedback.update(active.len(), target);
        }
        let mut valence = 0i32;
        for &kc in &active {
            let index = kc as usize;
            valence +=
                i32::from(approach_weights[index] >> 8) - i32::from(avoid_weights[index] >> 8);
        }
        if (label < 0 && valence < 0) || (label > 0 && valence > 0) {
            correct += 1;
        }
    }
    correct as f64 / cue_count as f64
}

fn perceptron_separability(batch: &[Vec<u32>], labels: &[i8], dims: usize) -> (f64, f64) {
    let mut weights = vec![0.0f64; dims];
    let mut bias = 0.0f64;
    let mut best_weights = weights.clone();
    let mut best_bias = 0.0f64;
    let mut best_errors = usize::MAX;
    for _epoch in 0..PERCEPTRON_EPOCHS {
        let mut errors = 0usize;
        for (active, &label) in batch.iter().zip(labels) {
            let mut score = bias;
            for &kc in active {
                score += weights[kc as usize];
            }
            if f64::from(label) * score <= 0.0 {
                errors += 1;
                for &kc in active {
                    weights[kc as usize] += f64::from(label);
                }
                bias += f64::from(label);
            }
        }
        if errors < best_errors {
            best_errors = errors;
            best_weights.copy_from_slice(&weights);
            best_bias = bias;
        }
        if errors == 0 {
            break;
        }
    }
    let accuracy = 1.0 - best_errors as f64 / batch.len() as f64;
    let norm: f64 = best_weights
        .iter()
        .map(|weight| weight * weight)
        .sum::<f64>()
        .sqrt();
    let mut margin = f64::INFINITY;
    for (active, &label) in batch.iter().zip(labels) {
        let mut score = best_bias;
        for &kc in active {
            score += best_weights[kc as usize];
        }
        let signed = f64::from(label) * score / norm;
        if signed < margin {
            margin = signed;
        }
    }
    if !margin.is_finite() {
        margin = 0.0;
    }
    (accuracy, margin)
}

#[derive(Clone, Copy)]
struct ConfigSpec {
    width: usize,
    kc_count: usize,
    claws: usize,
    model: InhibitionModel,
}

struct ReplicateOutcome {
    participation: f64,
    mean_active: f64,
    margin_norm_mean: f64,
    margin_abs_mean: f64,
    overlap_mean: f64,
    disjoint_fraction: f64,
    association_accuracy: Vec<f64>,
    perceptron_accuracy: Vec<f64>,
    perceptron_margin: Vec<f64>,
    capacity: usize,
}

fn ladder_capacity(ladder: &[usize], accuracy: &[f64]) -> usize {
    let mut capacity = 0;
    for (rung, &value) in ladder.iter().zip(accuracy) {
        if value >= ASSOCIATION_ACCURACY_FLOOR {
            capacity = *rung;
        }
    }
    capacity
}

fn dimension_efficiency(participation: f64, kc_count: usize, samples: usize) -> f64 {
    let ceiling = kc_count as f64 * samples as f64 / (kc_count + samples) as f64;
    if ceiling <= 0.0 {
        0.0
    } else {
        participation / ceiling
    }
}

fn config_stream_seed(spec: &ConfigSpec, replicate: u64, stream: u64) -> u64 {
    stream_seed(
        spec.width as u64,
        spec.kc_count as u64,
        spec.claws as u64,
        spec.model.index(),
        replicate,
        stream,
    )
}

fn run_replicate(spec: ConfigSpec, replicate: u64, ladder: &[usize]) -> ReplicateOutcome {
    let expansion = Expansion::new(
        spec.width,
        spec.claws,
        spec.kc_count,
        config_stream_seed(&spec, replicate, 1),
    );
    let mut scores = Vec::with_capacity(spec.kc_count);
    let mut active = Vec::new();
    let mut feedback = FeedbackController::new();
    let target = rank_target(spec.kc_count);

    let mut rng = SplitMix64::new(config_stream_seed(&spec, replicate, 2));
    let mut batch: Vec<Vec<u32>> = Vec::with_capacity(DIMENSION_CUES);
    for _ in 0..DIMENSION_CUES {
        let cue = draw_cue(&mut rng, spec.width);
        activate(
            &expansion,
            &mut scores,
            cue,
            spec.model,
            feedback.offset_q8,
            &mut active,
        );
        if let InhibitionModel::FeedbackThreshold = spec.model {
            feedback.update(active.len(), target);
        }
        batch.push(active.clone());
    }
    let (participation, mean_active) = participation_ratio(&batch, spec.kc_count);

    let mut rng = SplitMix64::new(config_stream_seed(&spec, replicate, 3));
    let mut margin_norm_sum = 0.0f64;
    let mut margin_abs_sum = 0.0f64;
    let mut overlap_sum = 0.0f64;
    let mut disjoint_count = 0usize;
    for _ in 0..PAIR_SAMPLES {
        let cue_a = draw_cue(&mut rng, spec.width);
        let cue_b = draw_cue(&mut rng, spec.width);
        activate(
            &expansion,
            &mut scores,
            cue_a,
            spec.model,
            feedback.offset_q8,
            &mut active,
        );
        let code_a = active.clone();
        activate(
            &expansion,
            &mut scores,
            cue_b,
            spec.model,
            feedback.offset_q8,
            &mut active,
        );
        let (overlap, margin_abs, margin_norm) = pair_metrics(&code_a, &active);
        margin_norm_sum += margin_norm;
        margin_abs_sum += margin_abs;
        overlap_sum += overlap as f64;
        if overlap == 0 {
            disjoint_count += 1;
        }
    }

    let mut accuracy_by_rung = Vec::with_capacity(ladder.len());
    let mut perceptron_accuracy_by_rung = Vec::with_capacity(ladder.len());
    let mut perceptron_margin_by_rung = Vec::with_capacity(ladder.len());
    for (rung_index, &cue_count) in ladder.iter().enumerate() {
        let mut accuracy_sum = 0.0f64;
        let mut perceptron_accuracy_sum = 0.0f64;
        let mut perceptron_margin_sum = 0.0f64;
        for ensemble in 0..ASSOCIATION_ENSEMBLES {
            let ensemble_seed = config_stream_seed(&spec, replicate, 100 + rung_index as u64)
                ^ mix(ensemble as u64);
            let mut rng = SplitMix64::new(ensemble_seed);
            let cues: Vec<u64> = (0..cue_count)
                .map(|_| draw_cue(&mut rng, spec.width))
                .collect();
            let labels = shuffled_balanced_labels(&mut rng, cue_count);
            accuracy_sum += association_accuracy(&expansion, spec.model, ensemble_seed, cue_count);
            let mut codes: Vec<Vec<u32>> = Vec::with_capacity(cue_count);
            for &cue in &cues {
                activate(
                    &expansion,
                    &mut scores,
                    cue,
                    spec.model,
                    feedback.offset_q8,
                    &mut active,
                );
                if let InhibitionModel::FeedbackThreshold = spec.model {
                    feedback.update(active.len(), target);
                }
                codes.push(active.clone());
            }
            let (perceptron_acc, perceptron_mar) =
                perceptron_separability(&codes, &labels, spec.kc_count);
            perceptron_accuracy_sum += perceptron_acc;
            perceptron_margin_sum += perceptron_mar;
        }
        let scale = ASSOCIATION_ENSEMBLES as f64;
        accuracy_by_rung.push(accuracy_sum / scale);
        perceptron_accuracy_by_rung.push(perceptron_accuracy_sum / scale);
        perceptron_margin_by_rung.push(perceptron_margin_sum / scale);
    }
    let capacity = ladder_capacity(ladder, &accuracy_by_rung);
    ReplicateOutcome {
        participation,
        mean_active,
        margin_norm_mean: margin_norm_sum / PAIR_SAMPLES as f64,
        margin_abs_mean: margin_abs_sum / PAIR_SAMPLES as f64,
        overlap_mean: overlap_sum / PAIR_SAMPLES as f64,
        disjoint_fraction: disjoint_count as f64 / PAIR_SAMPLES as f64,
        association_accuracy: accuracy_by_rung,
        perceptron_accuracy: perceptron_accuracy_by_rung,
        perceptron_margin: perceptron_margin_by_rung,
        capacity,
    }
}

#[derive(Clone, Copy)]
struct BramFootprint {
    win_bytes: usize,
    threshold_gain_bytes: usize,
    eligibility_bytes: usize,
    wout_bytes: usize,
}

impl BramFootprint {
    fn new(kc_count: usize, claws: usize) -> Self {
        Self {
            win_bytes: kc_count * claws,
            threshold_gain_bytes: 2 * kc_count,
            eligibility_bytes: 2 * kc_count,
            wout_bytes: 2 * OUTPUT_ROWS * kc_count,
        }
    }

    fn total_bytes(&self) -> usize {
        self.win_bytes + self.threshold_gain_bytes + self.eligibility_bytes + self.wout_bytes
    }
}

fn energy_tick_nj(kc_count: usize, claws: usize, model: InhibitionModel, mean_active: f64) -> f64 {
    let expansion = (kc_count as f64 * (3.0 * claws as f64 - 1.0) + EXPANSION_COUNTER_ACCESSES)
        * EXPANSION_ACCESS_FJ;
    let threshold = kc_count as f64 * THRESHOLD_ACCESSES_PER_KC * THRESHOLD_ACCESS_FJ;
    let traces = kc_count as f64 * TRACE_ACCESSES_PER_KC * TRACE_ACCESS_FJ;
    let topk = match model {
        InhibitionModel::RankSelect => kc_count as f64 * TOPK_ACCESSES_PER_KC * TOPK_ACCESS_FJ,
        _ => 0.0,
    };
    let feedback = match model {
        InhibitionModel::FeedbackThreshold => FEEDBACK_ACCESSES_PER_TICK * EXPANSION_ACCESS_FJ,
        _ => 0.0,
    };
    let readout = READOUT_FIXED_FJ + mean_active * READOUT_ACCESSES_PER_ACTIVE * READOUT_ACCESS_FJ;
    (expansion + threshold + traces + topk + feedback + readout + CONTROL_FJ) / 1_000_000.0
}

struct ConfigResult {
    spec: ConfigSpec,
    participation_mean: f64,
    participation_sd: f64,
    participation_efficiency: f64,
    mean_active: f64,
    margin_norm_mean: f64,
    margin_abs_mean: f64,
    overlap_mean: f64,
    disjoint_fraction: f64,
    association_accuracy_mean: Vec<f64>,
    perceptron_accuracy_mean: Vec<f64>,
    perceptron_margin_mean: Vec<f64>,
    capacity_pooled: usize,
    capacity_mean: f64,
    energy_nj: f64,
    bram: BramFootprint,
    admissible: bool,
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn standard_deviation(values: &[f64], average: f64) -> f64 {
    let variance = values
        .iter()
        .map(|value| (value - average) * (value - average))
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn run_config(spec: ConfigSpec, replicates: u32, ladder: &[usize]) -> ConfigResult {
    let outcomes: Vec<ReplicateOutcome> = (0..replicates)
        .map(|replicate| run_replicate(spec, u64::from(replicate), ladder))
        .collect();
    let participation_values: Vec<f64> = outcomes
        .iter()
        .map(|outcome| outcome.participation)
        .collect();
    let participation_mean = mean(&participation_values);
    let mean_active = mean(&outcomes.iter().map(|o| o.mean_active).collect::<Vec<_>>());
    let margin_norm_mean = mean(
        &outcomes
            .iter()
            .map(|o| o.margin_norm_mean)
            .collect::<Vec<_>>(),
    );
    let margin_abs_mean = mean(
        &outcomes
            .iter()
            .map(|o| o.margin_abs_mean)
            .collect::<Vec<_>>(),
    );
    let overlap_mean = mean(&outcomes.iter().map(|o| o.overlap_mean).collect::<Vec<_>>());
    let disjoint_fraction = mean(
        &outcomes
            .iter()
            .map(|o| o.disjoint_fraction)
            .collect::<Vec<_>>(),
    );
    let mut association_accuracy_mean = Vec::with_capacity(ladder.len());
    let mut perceptron_accuracy_mean = Vec::with_capacity(ladder.len());
    let mut perceptron_margin_mean = Vec::with_capacity(ladder.len());
    for rung_index in 0..ladder.len() {
        association_accuracy_mean.push(mean(
            &outcomes
                .iter()
                .map(|o| o.association_accuracy[rung_index])
                .collect::<Vec<_>>(),
        ));
        perceptron_accuracy_mean.push(mean(
            &outcomes
                .iter()
                .map(|o| o.perceptron_accuracy[rung_index])
                .collect::<Vec<_>>(),
        ));
        perceptron_margin_mean.push(mean(
            &outcomes
                .iter()
                .map(|o| o.perceptron_margin[rung_index])
                .collect::<Vec<_>>(),
        ));
    }
    let capacity_pooled = ladder_capacity(ladder, &association_accuracy_mean);
    let capacity_mean = mean(
        &outcomes
            .iter()
            .map(|o| o.capacity as f64)
            .collect::<Vec<_>>(),
    );
    let energy_nj = energy_tick_nj(spec.kc_count, spec.claws, spec.model, mean_active);
    ConfigResult {
        spec,
        participation_mean,
        participation_sd: standard_deviation(&participation_values, participation_mean),
        participation_efficiency: dimension_efficiency(
            participation_mean,
            spec.kc_count,
            DIMENSION_CUES,
        ),
        mean_active,
        margin_norm_mean,
        margin_abs_mean,
        overlap_mean,
        disjoint_fraction,
        association_accuracy_mean,
        perceptron_accuracy_mean,
        perceptron_margin_mean,
        capacity_pooled,
        capacity_mean,
        energy_nj,
        bram: BramFootprint::new(spec.kc_count, spec.claws),
        admissible: false,
    }
}

fn is_baseline(result: &ConfigResult) -> bool {
    result.spec.width == BASELINE_WIDTH
        && result.spec.kc_count == BASELINE_KC_COUNT
        && result.spec.claws == BASELINE_CLAWS
        && result.spec.model == InhibitionModel::RankSelect
}

fn mark_admissible(results: &mut [ConfigResult], ladder: &[usize]) {
    let baseline_index = results
        .iter()
        .position(is_baseline)
        .expect("baseline present");
    let baseline_margin = results[baseline_index].margin_norm_mean;
    let baseline_capacity = results[baseline_index].capacity_pooled;
    let baseline_accuracy = results[baseline_index].association_accuracy_mean.clone();
    let baseline_rungs: Vec<usize> = ladder
        .iter()
        .enumerate()
        .filter(|(_, rung)| **rung <= baseline_capacity)
        .map(|(index, _)| index)
        .collect();
    for result in results.iter_mut() {
        let margin_ok = result.margin_norm_mean >= PAIR_MARGIN_RELATIVE_TOLERANCE * baseline_margin;
        let capacity_ok = result
            .capacity_pooled
            .ge(&baseline_capacity.saturating_sub(ASSOCIATION_CAPACITY_STEP_TOLERANCE));
        let accuracy_ok = baseline_rungs.iter().all(|&index| {
            result.association_accuracy_mean[index]
                >= baseline_accuracy[index] - ASSOCIATION_ACCURACY_TOLERANCE
        });
        result.admissible = margin_ok && capacity_ok && accuracy_ok;
    }
}

fn full_spec_ladder() -> Vec<ConfigSpec> {
    let mut specs = Vec::new();
    for width in [50usize, 64] {
        for kc_count in [512usize, 1024, 2048, 4096] {
            for claws in 2..=12usize {
                for model in InhibitionModel::all() {
                    specs.push(ConfigSpec {
                        width,
                        kc_count,
                        claws,
                        model,
                    });
                }
            }
        }
    }
    specs
}

fn run_sweep_parallel(
    specs: &[ConfigSpec],
    replicates: u32,
    ladder: &[usize],
) -> Vec<ConfigResult> {
    let workers = thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let progress = AtomicUsize::new(0);
    let total = specs.len();
    let mut indexed: Vec<(usize, ConfigResult)> = Vec::with_capacity(total);
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for worker in 0..workers {
            let chunk: Vec<(usize, &ConfigSpec)> = specs
                .iter()
                .enumerate()
                .filter(|(index, _)| index % workers == worker)
                .collect();
            let progress = &progress;
            handles.push(scope.spawn(move || {
                let mut results = Vec::with_capacity(chunk.len());
                for (index, spec) in chunk {
                    let result = run_config(*spec, replicates, ladder);
                    let done = progress.fetch_add(1, Ordering::Relaxed) + 1;
                    if done.is_multiple_of(16) || done == total {
                        eprintln!("e1 sweep progress {done}/{total}");
                    }
                    results.push((index, result));
                }
                results
            }));
        }
        for handle in handles {
            indexed.extend(handle.join().expect("sweep worker"));
        }
    });
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, result)| result).collect()
}

fn write_results_csv(results: &[ConfigResult], ladder: &[usize], path: &std::path::Path) {
    let mut header = String::from(
        "width,kc_count,claws,model,participation_mean,participation_sd,participation_efficiency,mean_active,margin_norm_mean,margin_abs_mean,overlap_mean,disjoint_fraction",
    );
    for rung in ladder {
        let _ = write!(header, ",assoc_acc_{rung}");
    }
    for rung in ladder {
        let _ = write!(header, ",perceptron_acc_{rung}");
    }
    for rung in ladder {
        let _ = write!(header, ",perceptron_margin_{rung}");
    }
    header.push_str(
        ",capacity_pooled,capacity_seed_mean,energy_nj,bram_win,bram_thresholds,bram_eligibility,bram_wout,bram_total,admissible",
    );
    let mut body = String::new();
    for result in results {
        let spec = result.spec;
        let _ = write!(
            body,
            "{},{},{},{},{:.4},{:.4},{:.4},{:.2},{:.5},{:.4},{:.3},{:.4}",
            spec.width,
            spec.kc_count,
            spec.claws,
            spec.model.label(),
            result.participation_mean,
            result.participation_sd,
            result.participation_efficiency,
            result.mean_active,
            result.margin_norm_mean,
            result.margin_abs_mean,
            result.overlap_mean,
            result.disjoint_fraction
        );
        for value in &result.association_accuracy_mean {
            let _ = write!(body, ",{value:.5}");
        }
        for value in &result.perceptron_accuracy_mean {
            let _ = write!(body, ",{value:.5}");
        }
        for value in &result.perceptron_margin_mean {
            let _ = write!(body, ",{value:.5}");
        }
        let _ = writeln!(
            body,
            ",{},{:.2},{:.4},{},{},{},{},{},{}",
            result.capacity_pooled,
            result.capacity_mean,
            result.energy_nj,
            result.bram.win_bytes,
            result.bram.threshold_gain_bytes,
            result.bram.eligibility_bytes,
            result.bram.wout_bytes,
            result.bram.total_bytes(),
            result.admissible
        );
    }
    fs::write(path, format!("{header}\n{body}")).expect("write csv");
}

fn summary_text(results: &[ConfigResult], ladder: &[usize], elapsed_seconds: f64) -> String {
    let mut text = String::new();
    let baseline = results
        .iter()
        .find(|result| is_baseline(result))
        .expect("baseline");
    let _ = writeln!(
        text,
        "baseline (w{} n{} k{} {}): pr {:.1} (eff {:.3}), margin_norm {:.4}, capacity_pooled {}, capacity_seed_mean {:.1}, active {:.1}, energy {:.4} nJ, bram {} B",
        baseline.spec.width,
        baseline.spec.kc_count,
        baseline.spec.claws,
        baseline.spec.model.label(),
        baseline.participation_mean,
        baseline.participation_efficiency,
        baseline.margin_norm_mean,
        baseline.capacity_pooled,
        baseline.capacity_mean,
        baseline.mean_active,
        baseline.energy_nj,
        baseline.bram.total_bytes()
    );
    let _ = writeln!(
        text,
        "configurations: {} over {} replicates, ladder {:?}, elapsed {:.1} s",
        results.len(),
        FULL_REPLICATES,
        ladder,
        elapsed_seconds
    );
    let mut ranked: Vec<&ConfigResult> =
        results.iter().filter(|result| result.admissible).collect();
    let _ = writeln!(text, "admissible configurations: {}", ranked.len());
    ranked.sort_by(|a, b| {
        a.energy_nj
            .partial_cmp(&b.energy_nj)
            .unwrap()
            .then(a.bram.total_bytes().cmp(&b.bram.total_bytes()))
    });
    for result in ranked.iter().take(12) {
        let _ = writeln!(
            text,
            "  w{} n{} k{} {}: energy {:.4} nJ, bram {} B, pr {:.1} (eff {:.3}), margin_norm {:.4}, capacity_pooled {}, active {:.1}",
            result.spec.width,
            result.spec.kc_count,
            result.spec.claws,
            result.spec.model.label(),
            result.energy_nj,
            result.bram.total_bytes(),
            result.participation_mean,
            result.participation_efficiency,
            result.margin_norm_mean,
            result.capacity_pooled,
            result.mean_active
        );
    }
    let mut by_bram: Vec<&ConfigResult> =
        results.iter().filter(|result| result.admissible).collect();
    by_bram.sort_by_key(|result| result.bram.total_bytes());
    if let Some(result) = by_bram.first() {
        let _ = writeln!(
            text,
            "smallest BRAM admissible: w{} n{} k{} {} at {} B, energy {:.4} nJ",
            result.spec.width,
            result.spec.kc_count,
            result.spec.claws,
            result.spec.model.label(),
            result.bram.total_bytes(),
            result.energy_nj
        );
    }
    text
}

#[test]
fn mini_sweep_end_to_end_and_decision_marks_baseline() {
    let mut specs = Vec::new();
    for claws in [2usize, 6] {
        for model in InhibitionModel::all() {
            specs.push(ConfigSpec {
                width: 50,
                kc_count: 512,
                claws,
                model,
            });
        }
    }
    specs.push(ConfigSpec {
        width: BASELINE_WIDTH,
        kc_count: BASELINE_KC_COUNT,
        claws: BASELINE_CLAWS,
        model: InhibitionModel::RankSelect,
    });
    let ladder = [2usize, 8, 32];
    let mut results: Vec<ConfigResult> = specs
        .iter()
        .map(|spec| run_config(*spec, 2, &ladder))
        .collect();
    mark_admissible(&mut results, &ladder);
    let baseline = results.iter().position(is_baseline).expect("baseline");
    assert!(results[baseline].admissible);
    let cheapest = results
        .iter()
        .filter(|result| result.admissible)
        .min_by(|a, b| {
            a.energy_nj
                .partial_cmp(&b.energy_nj)
                .unwrap()
                .then(a.bram.total_bytes().cmp(&b.bram.total_bytes()))
        })
        .expect("cheapest admissible");
    assert!(cheapest.energy_nj <= results[baseline].energy_nj);
}

#[test]
#[ignore]
fn full_e1_sweep() {
    let specs = full_spec_ladder();
    let start = std::time::Instant::now();
    let mut results = run_sweep_parallel(&specs, FULL_REPLICATES, &FULL_LADDER);
    let elapsed = start.elapsed().as_secs_f64();
    mark_admissible(&mut results, &FULL_LADDER);
    let out_dir = env::var("E1_OUT_DIR").unwrap_or_else(|_| "target/e1-sweep".to_string());
    let out_path = std::path::Path::new(&out_dir);
    fs::create_dir_all(out_path).expect("create output directory");
    write_results_csv(&results, &FULL_LADDER, &out_path.join("config_results.csv"));
    let summary = summary_text(&results, &FULL_LADDER, elapsed);
    fs::write(out_path.join("summary.txt"), &summary).expect("write summary");
    println!("{summary}");
    println!("results written under {}", out_path.display());
    assert_eq!(results.len(), 264);
    assert!(results.iter().any(|result| result.admissible));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_sequences_are_deterministic_and_seeded() {
        let mut a = SplitMix64::new(7);
        let mut b = SplitMix64::new(7);
        let mut c = SplitMix64::new(8);
        for _ in 0..8 {
            let x = a.next_u64();
            assert_eq!(x, b.next_u64());
            assert_ne!(x, c.next_u64());
        }
        let mut r = SplitMix64::new(99);
        assert_eq!(r.next_below(1), 0);
        for _ in 0..64 {
            assert!(r.next_below(50) < 50);
        }
    }

    #[test]
    fn calibration_hits_five_percent_for_all_claw_counts() {
        for claws in 2..=12usize {
            let marginal = calibrated_marginal(claws);
            assert!(
                (0.045..=0.055).contains(&marginal),
                "claws {claws} marginal {marginal}"
            );
        }
    }

    #[test]
    fn measured_marginal_matches_calibration_for_current_shape() {
        let expansion = Expansion::new(50, 6, 1024, 12345);
        let mut rng = SplitMix64::new(999);
        let mut scores = Vec::new();
        let mut active = Vec::new();
        let mut total_active = 0usize;
        let ticks = 2000usize;
        for _ in 0..ticks {
            let cue = draw_cue(&mut rng, 50);
            activate(
                &expansion,
                &mut scores,
                cue,
                InhibitionModel::Disabled,
                0,
                &mut active,
            );
            total_active += active.len();
        }
        let measured = total_active as f64 / (ticks as f64 * 1024.0);
        assert!((0.045..=0.055).contains(&measured), "measured {measured}");
    }

    #[test]
    fn expansion_wiring_is_distinct_and_complete() {
        let expansion = Expansion::new(50, 6, 64, 20260106);
        assert_eq!(
            expansion.claw_channels.len(),
            expansion.kc_count * expansion.claw_count
        );
        for kc in 0..expansion.kc_count {
            let claws = &expansion.claw_channels
                [kc * expansion.claw_count..(kc + 1) * expansion.claw_count];
            let mut sorted = claws.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), expansion.claw_count);
            assert!(
                claws
                    .iter()
                    .all(|&channel| (channel as usize) < expansion.width)
            );
        }
        assert_eq!(
            expansion
                .channel_masks
                .iter()
                .map(|mask| mask.count_ones())
                .sum::<u32>() as usize,
            expansion.kc_count * expansion.claw_count
        );
    }

    #[test]
    fn rank_select_caps_and_keeps_the_top_scores() {
        let expansion = Expansion::new(50, 6, 1024, 4242);
        let mut scores = Vec::new();
        let mut active = Vec::new();
        let cue = (1u64 << 50) - 1;
        activate(
            &expansion,
            &mut scores,
            cue,
            InhibitionModel::Disabled,
            0,
            &mut active,
        );
        assert_eq!(active.len(), 1024);
        let unconstrained: std::collections::HashSet<u32> = active.iter().copied().collect();
        activate(
            &expansion,
            &mut scores,
            cue,
            InhibitionModel::RankSelect,
            0,
            &mut active,
        );
        let target = rank_target(1024);
        assert_eq!(target, 51);
        assert_eq!(active.len(), target);
        assert!(active.iter().all(|kc| unconstrained.contains(kc)));
        let chosen: std::collections::HashSet<u32> = active.iter().copied().collect();
        let minimum_kept = active.iter().map(|kc| scores[*kc as usize]).min().unwrap();
        let excluded_max = scores
            .iter()
            .enumerate()
            .filter(|(kc, _)| !chosen.contains(&(*kc as u32)))
            .map(|(_, score)| *score)
            .max()
            .unwrap();
        assert!(minimum_kept >= excluded_max);
    }

    #[test]
    fn feedback_threshold_settles_near_the_target_count() {
        let expansion = Expansion::new(50, 6, 1024, 777);
        let mut rng = SplitMix64::new(555);
        let mut scores = Vec::new();
        let mut active = Vec::new();
        let mut feedback = FeedbackController::new();
        let target = rank_target(1024);
        let mut recent_total = 0usize;
        let recent_ticks = 400usize;
        let total_ticks = 1200usize;
        for tick in 0..total_ticks {
            let cue = draw_cue(&mut rng, 50);
            activate(
                &expansion,
                &mut scores,
                cue,
                InhibitionModel::FeedbackThreshold,
                feedback.offset_q8,
                &mut active,
            );
            feedback.update(active.len(), target);
            if tick >= total_ticks - recent_ticks {
                recent_total += active.len();
            }
        }
        let settled = recent_total as f64 / recent_ticks as f64;
        assert!(
            settled > 0.6 * target as f64 && settled < 1.2 * target as f64,
            "feedback settled {settled} target {target}"
        );
    }

    #[test]
    fn participation_ratio_separates_independent_and_duplicated_codes() {
        let mut rng = SplitMix64::new(31337);
        let dims = 512usize;
        let samples = 256usize;
        let independent: Vec<Vec<u32>> = (0..samples)
            .map(|_| {
                let mut code: Vec<u32> = (0..dims as u32).filter(|_| rng.next_channel()).collect();
                if code.is_empty() {
                    code.push(rng.next_below(dims as u64) as u32);
                }
                code
            })
            .collect();
        let (pr_independent, _) = participation_ratio(&independent, dims);
        let isotropic = dims as f64 * samples as f64 / (dims + samples) as f64;
        assert!(
            pr_independent > 0.85 * isotropic && pr_independent < 1.15 * isotropic,
            "pr {pr_independent} isotropic {isotropic}"
        );
        assert!(
            (dimension_efficiency(pr_independent, dims, samples) - 1.0).abs() < 0.15,
            "efficiency {}",
            dimension_efficiency(pr_independent, dims, samples)
        );
        let base = independent[0].clone();
        let duplicated = vec![base; samples];
        let (pr_duplicated, _) = participation_ratio(&duplicated, dims);
        assert!(pr_duplicated < 2.0, "pr {pr_duplicated}");
        assert!(dimension_efficiency(pr_duplicated, dims, samples) < 0.01);
    }

    #[test]
    fn pair_margin_bounds_for_identical_and_disjoint_codes() {
        let a: Vec<u32> = (0..26).collect();
        let b: Vec<u32> = (26..52).collect();
        let (overlap, margin_abs, margin_norm) = pair_metrics(&a, &b);
        assert_eq!(overlap, 0);
        assert!((margin_abs - (52.0f64).sqrt() / 2.0).abs() < 1e-12);
        assert!((margin_norm - 1.0 / 2.0f64.sqrt()).abs() < 1e-12);
        let copy = a.clone();
        let (overlap, margin_abs, margin_norm) = pair_metrics(&a, &copy);
        assert_eq!(overlap, 26);
        assert_eq!(margin_abs, 0.0);
        assert_eq!(margin_norm, 0.0);
    }

    #[test]
    fn one_shot_association_separates_two_opponent_cues() {
        let expansion = Expansion::new(50, 6, 512, 20260106);
        let accuracy = association_accuracy(&expansion, InhibitionModel::RankSelect, 424242, 2);
        assert_eq!(accuracy, 1.0);
    }

    #[test]
    fn perceptron_separates_disjoint_support_sets() {
        let a: Vec<u32> = (0..20).collect();
        let b: Vec<u32> = (20..40).collect();
        let batch = vec![a, b];
        let labels = [-1i8, 1];
        let (accuracy, margin) = perceptron_separability(&batch, &labels, 40);
        assert_eq!(accuracy, 1.0);
        assert!(margin > 0.0);
    }

    #[test]
    fn energy_model_reproduces_the_priced_baseline_tick() {
        let energy = energy_tick_nj(2048, 6, InhibitionModel::RankSelect, PRICED_ACTIVE_PER_TICK);
        assert!(
            (energy - PRICED_SKIP_TICK_NJ).abs() < 0.001,
            "energy {energy}"
        );
    }

    #[test]
    fn energy_model_orders_models_and_shapes() {
        let rank = energy_tick_nj(2048, 6, InhibitionModel::RankSelect, 102.0);
        let disabled = energy_tick_nj(2048, 6, InhibitionModel::Disabled, 102.0);
        let small = energy_tick_nj(512, 6, InhibitionModel::Disabled, 26.0);
        let wide = energy_tick_nj(2048, 12, InhibitionModel::RankSelect, 102.0);
        assert!(disabled < rank);
        assert!(small < disabled);
        assert!(wide > rank);
    }

    #[test]
    fn bram_footprint_matches_the_ledger_anchors() {
        let bram = BramFootprint::new(2048, 6);
        assert_eq!(bram.win_bytes, 12288);
        assert_eq!(bram.threshold_gain_bytes, 4096);
        assert_eq!(bram.eligibility_bytes, 4096);
        assert_eq!(bram.wout_bytes, 86016);
        assert_eq!(bram.total_bytes(), 106496);
    }
}
