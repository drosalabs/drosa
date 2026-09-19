# Drosa Hardware Mapping: BRAM Ledger, Precision Bounds, and Silicon Budget

Date: 2026-09-17
Status: Specification of record for the FPGA mapping of Drosa
Sources: hardware verification appendix (section 15) of the audited research
ledger in `~/main`; vendor datasheets [H10], [H11]; every latency, power, and
utilization figure below is a calculation from stated parameters, to be
replaced by measurement at the Phase 3 gates.

---

## 1. Target platforms

| Part | LUTs | BRAM36 | DSP slices | Role in the roadmap |
|---|---:|---:|---:|---|
| Artix-7 XC7A35T | 20,800 | 50 (225 KB) | 90 | aggressive floor; 60 to 80% BRAM before double-buffering choices |
| Artix-7 XC7A100T | 63,400 | 135 (608 KB) | 240 | recommended minimum-comfortable part; first P&R target |
| Kria K26 SOM (Zynq US+ XCZU5EV) | 117,120 | 144 (about 660 KB) + 64 UltraRAM | 1,248 | fits trivially; adds the ARM subsystem the asynchronous supervisor link wants |

One BRAM36 block holds 36 Kb, 4,608 bytes usable. The design occupies 30 to
40 blocks depending on configuration. No external memory controller is
instantiated on any target; on Kria the DDR controller exists on the board
for the ARM telemetry path and is outside the control loop.

Scope of the power claims: Artix-7 class boards measure 1.5 to 2.5 W total
for this design; a Kria KV260 board runs near 5 W with the processor
subsystem and DDR active, so a 5 to 7 W figure is the honest Kria
expectation. The sub-2.5 W claim is scoped to Artix-7 class builds and never
stated unscoped.

## 2. Mapping principles

1. **No floating point in the control path.** Every operation reduces to
   popcount, compare, gated integer addition, fixed-point multiply-accumulate
   on a bounded set of DSP slices, shift, or small-table lookup. No FPU and
   no soft-float library is instantiated. If the Kria ARM subsystem runs
   Linux for telemetry, that software may use floats; the control loop does
   not.
2. **No external DRAM in the control path.** Capacity is not the binding
   argument; bandwidth, energy, and determinism are, in that order
   (section 8).
3. **Zero multipliers on the Mushroom Body path.** The sensory expansion is
   pure LUT logic; the MBON readout is conditional addition. DSP slices are
   spent only on the Central Complex and steering arithmetic.
4. **Determinism is a first-class property.** Same inputs, same seed, same
   outputs on CPU, GPU, and fabric, verified bit-exact through the export
   manifest (section 11). Single-cycle on-chip access latency is treated as
   a design feature worth more than capacity headroom.

## 3. Clocking and pipeline

Three clock domains, aligned to the biological timescale hierarchy:

```text
+-------------------------------------------------------------------+
| Motor reflex loop, 1 to 5 kHz                                     |
| phase increment, gait ROM read, servo update, GF reflex latch      |
+----------------------------------+--------------------------------+
                                  |
+----------------------------------v-------------------------------+
| Spatial guidance and policy loop, 100 to 500 Hz                   |
| KC expansion, MBON readout, compass update, vector integration,   |
| steering arbitration onto v and omega command registers           |
+----------------------------------+--------------------------------+
                                  |
+----------------------------------v-------------------------------+
| Neuromodulatory learning loop, 1 to 10 Hz                         |
| compartment teacher events, trace decay, in-place W_out update    |
+-------------------------------------------------------------------+
```

Cycle budgets at a 100 MHz fabric clock:

| Operation | Cycles | Time | Note |
|---|---:|---:|---|
| Policy tick (expansion + readout + CX + steering) | <= 200 | ~2 us | 0.1% duty at the 500 Hz tick period (2 ms) |
| Motor/CPG tick | <= 50 | ~0.5 us | 1 to 5 kHz loop runs with 200 to 1000x margin |
| Giant fiber reflex (loom feature, threshold, command latch) | 2 to 4 | 20 to 40 ns | end-to-end reflex bounded by sensor cadence, inside the 10 ms budget |

The "sub-microsecond step latency" claim is accurate for the motor increment
and the fabric reflex path; perception is sensor-cadence-limited and never
quoted at fabric latency.

## 4. Sensory expansion microarchitecture (W_in)

Binary connectivity is anatomically faithful: a KC claw is present or
absent. Stored as compressed sparse row, six u8 input indices per KC:
2,048 x 6 = 12,288 bytes, frozen, in bitstream ROM, distributed LUT RAM, or
BRAM.

Two operating points, both zero-DSP:

| Operating point | Per-KC cost | Total for 2,048 KCs | Notes |
|---|---|---|---|
| Fully parallel, binary activations | popcount + compare, ~6-8 LUTs | 12-16K LUTs | minimum latency; INT8-free |
| 256-lane multiplexed, INT8 sums, binary KC outputs | 8-bit adder tree + compare, ~28 LUTs | ~8K LUTs | 8 passes per tick, ~40 cycles at 100 MHz; graded PN rates preserved |

The recommended operating point is the second: projection-neuron output is
graded, and discarding that grading costs odor separability on hard odor
sets for no resource reason (this is the first item in the risk register).
The KC output itself stays binary in both points, which is what keeps the
readout a gated add. APL-style feedback inhibition becomes "keep the top 5%
above threshold", a rank-select over the KC sums.

## 5. MBON readout microarchitecture (W_out inference)

Binary KC activation turns each multiply-accumulate into a gated add:
$y_i = \sum_{j : z_j = 1} W_{ij}$. With ~102 active KCs and 21 output rows,
one policy tick performs ~2,100 conditional byte additions. No multiplier is
instantiated anywhere in the Mushroom Body inference path.

The learning update uses all nonzero historical eligibility, not only the
currently active KCs. Its worst case is 2,048 x 21 = 43,008 logical synapses
per event, with events bounded at ~10 Hz by the caller. The 102 x 21 count
applies only when the eligible set is as sparse as the current input. The
[local-learning specification](3-factor-plasticity-and-cubecl-learning.md)
defines exact integer updates and packed-word ownership for device execution.

## 6. Learning-path microarchitecture

Per policy tick: decay 2,048 16-bit KC eligibility values and refresh active
KC tags. The implemented local-learning variant uses unsigned Q1.15 bounded
replacement tags and consumed signed Q7 teacher pulses, as specified in
[the arithmetic contract](3-factor-plasticity-and-cubecl-learning.md#2-mathematical-formulation-and-fixed-point-arithmetic).
This refines the original accumulating Q8.8 trace sketch without increasing
trace-vector storage.
Per teaching event: for each active row $i$ and each KC $j$ with nonzero
trace, add $-\eta\,(d_c - d_{0,c})\, e_j$ into the INT16 Q8.8 master, then
clamp to the box interval. Because the eligibility factorizes (per-KC trace
times per-compartment route), the update streams KC-major through the same
gated-add datapath as inference; no per-synapse trace memory exists.

## 7. Precision bounds

### 7.1 The quantization stall, and why INT16 masters exist

Store W_out directly at inference precision, INT8 full scale ±127. A
dopamine-gated update small enough for stable online learning (per-event
updates of 0.1 to 1.5 LSB, the regime where one pairing moves behavior
gently) frequently satisfies $|\Delta W| < 1$ LSB. Round-to-nearest maps
every such update to zero: the weight never moves, not slowly, never. That
is the quantization stall.

The fix is the master-weight construction: keep the full-precision master in
INT16 Q8.8 (2,048 x 21 x 2 = 86,016 bytes), accumulate updates in the
master, quantize to INT8 by taking the upper byte on inference read.

Arithmetic that pins the design rule:

- One INT8 inference LSB spans exactly 256 master LSBs.
- Per-event master updates of 32 to 64 LSB move the inference byte every 4
  to 8 reinforced events per synapse.
- A first-event change in the INT8 MBON sum depends on fractional weight
  phases and update size. Population size alone does not ensure that any
  synapse crosses a runtime-byte boundary.
- Updates below one master LSB need a further rounding mechanism. The CPU
  reference uses event-keyed stochastic rounding without a residual matrix;
  INT16 storage alone does not prevent sub-master quantization stalls.

Choose $\eta$ against the master LSB, never the inference LSB. Gate G1.2
catches a mis-tuned $\eta$.

### 7.2 Compass precision: drift arithmetic

Storing the heading pair $(\cos\theta, \sin\theta)$ in Q1.15 and rotating
with an exact 2x2 rotation: every step's four products round to the Q15
grid, $q = 2^{-15} \approx 3.05 \times 10^{-5}$. With per-step phase error
modeled as a zero-mean random walk of step size ~$2q$, over
$N = 3 \times 10^5$ steps (10 minutes of darkness at 500 Hz):

$$\sigma \approx \sqrt{N} \cdot 2q \approx 3.3 \times 10^{-2} \text{ rad} \approx 1.9^\circ$$

Renormalizing to the unit circle each step bounds amplitude drift but not
phase walk. At Q1.7 the step error is 128x larger and the 10-minute walk
reaches ~4.2 radians. INT16 is therefore the minimum for stored
trigonometric state; INT8 is disqualified for heading storage while
remaining fine for readout slices and all MB-side values.

The INT32 phase accumulator ($\theta \leftarrow \theta + \omega_q \Delta t$,
pair produced by CORDIC or quarter-wave LUT) adds zero storage error: all
drift is the once-quantized angular-velocity input, a sensor-calibration
property. It is also cheaper, 4 to 6 DSP slices versus 12 to 16. Decision:
the accumulator is the primary heading representation in RTL; the recurrent
16-wedge bump network is retained in Phase 1 simulation as the biologically
literal comparison condition and earns silicon only if it demonstrates a
functional advantage.

### 7.3 Format contract

| Signal | Format | Storage |
|---|---|---|
| PN/channel rates | INT8 | rate registers, 128 B |
| KC sums and thresholds | INT8 | thresholds ROM 2,048 B |
| KC activations | binary | 256 B bitmap |
| W_out inference read | INT8 (master upper byte) | derived, not stored twice |
| W_out master | INT16 Q8.8 | 86,016 B BRAM |
| KC eligibility traces | unsigned Q1.15, 16 bits | 4,096 B BRAM |
| Teacher pulses and routing | signed Q7 / static table | 64 B target, representation-dependent |
| Heading state | INT32 phase accumulator | ~8 B (+ Q1.15 pair in reference build) |
| Home vector, steering registers | INT16 fixed point | ~200 B total CX state |
| CPG joint trajectories | INT8 ROM | ~1.9 KB with phase accumulators |
| Decay factors $\lambda_t$, $\alpha_t$ | Q1.15 LUT | 16-entry tables |

## 8. On-chip memory ledger

Every line is calculated from the parameter set of record; "frozen" items
can live in bitstream ROM rather than mutable BRAM.

| Item | Size (bytes) | Class |
|---|---:|---|
| W_in connectivity (CSR, 6 x u8 per KC) | 12,288 | frozen |
| KC thresholds (u8) | 2,048 | frozen |
| KC gains (u8, optional) | 2,048 | frozen |
| Sensory channel rate registers (64 x i16) | 128 | mutable |
| KC activation bitmap (2,048 x 1 bit) | 256 | mutable |
| KC eligibility traces (2,048 x i16) | 4,096 | mutable |
| W_out master, INT16 Q8.8 (2,048 x 21) | 86,016 | mutable |
| Compartment teacher traces and routing | 64 | mutable |
| CX state (heading pair, phase accumulator, home vector, 16-wedge bump, steering registers) | ~200 | mutable |
| GDN-2 context state, 4 heads x 16 x 16 i16 (option) | 2,048 | mutable |
| CPG phase accumulators, gait ROM, servo state (6 joints) | ~1,900 | mixed |
| Sensor front-end: FIR taps, nonlinearity LUTs, flow tables | ~4,700 | frozen |
| Buffers, telemetry FIFO, double-buffer slack | 8,192 | mutable |
| **Core total (valence rule)** | **~124,000** | |
| Actor traces 8 x 2,048 x i16 + critic and critic trace (option, reusing MBON rows) | +40,960 | mutable |
| **Full configuration total** | **~165,000** | |

The public headline number is 181 KB: a conservative envelope chosen so that
every configuration, including the optional actor head and double-buffer
slack, sits inside the stated figure with margin. The audited line-item
totals (124 to 165 KB) are the engineering numbers; the envelope is the
marketing number, and both are stated together wherever the budget is
quoted. At 4,608 bytes per BRAM36 block the full configuration occupies 30
to 40 blocks including banking granularity.

### 8.1 Part utilization

| Part | Design need | Utilization |
|---|---|---|
| XC7A35T | 12-19K LUT, 30-40 BRAM36, <= 32 DSP | LUT 58-91% at the low-LUT point (tight), BRAM 60-80%, DSP <= 36% |
| XC7A100T | same | LUT 19-30%, BRAM 22-30%, DSP <= 13% |
| Kria K26 | same | LUT 10-16%, BRAM 21-28%, DSP <= 3% |

The 35T is an aggressive floor rather than a target; the 100T is the first
P&R closure target (gate G3.2).

## 9. The DDR-free argument

Capacity was never the binding constraint. The argument, in order of
importance:

1. **Bandwidth.** At a 500 Hz policy tick, the worst case that reads the
   entire W_out master every tick is 86 KB x 500 = 43 MB/s; the sparse
   active-set read is ~102 x 21 bytes, about 2.1 KB per tick, ~1 MB/s. A
   36 Kb dual-port BRAM block at 100 MHz moves 2 x 36 bits x 10^8 =
   0.9 GB/s; the ~30 blocks this design uses provide on the order of
   24 GB/s aggregate. Worst-case utilization is ~0.2%.
2. **Energy.** At typical on-chip SRAM scales (order 1 to 2 pJ per bit
   read; estimate, not measured here) the 43 MB/s worst case costs on the
   order of 0.5 mW of array dynamic power. A DDR4 controller plus DRAM
   draws 100 to 500 mW before the first useful access (estimate from
   standard controller/DRAM power classes).
3. **Determinism.** With no external memory controller there is no
   arbitration, no refresh, no TLB event anywhere in the control path.
   Fixed single-cycle access latency is worth more to a hard-real-time
   controller than any capacity argument.

## 10. LUT and DSP inventory

DSP slices:

| Block | MAC count |
|---|---|
| Heading rotation or CORDIC (compact formulation) | 4-6 |
| Home-vector rotation | 4 |
| Steering evaluation | 4 |
| Trace/decay factors | 1-2 |
| **Compact total** | **13-16 concurrent multiplies, 16 DSP slices fully parallel** |
| Literal 16-wedge ring attractor (3 neighbor terms per wedge) | 48 MACs: 32 DSPs at 2:1 multiplexing, or 48-64 fully parallel, which exceeds the budget |

This is why the phase accumulator is primary and the literal ring is a
simulation comparison condition. The Mushroom Body contributes zero DSPs on
top of the CX total.

LUTs:

| Block | LUTs |
|---|---|
| KC expansion (parallel binary / 256-lane INT8) | 12-16K / ~8K |
| MBON gather and accumulate | ~1K |
| CX and steering glue | ~1K |
| Sequencing and arbiters | ~2K |
| Sensor front-end | 1-2K |
| **Total** | **12-19K** |

## 11. Frozen image flow and cross-target determinism

Phase 2 pre-training (MuJoCo CPU physics plus CubeCL/WGPU on the local
RX 6900 XT; CUDA-dependent stacks are unavailable on this card) converges
the frozen 99%: sensor calibration (optic flow to velocity, haltere to
angular velocity, landmark anchoring gains) and CPG/motor residuals, under
domain randomization (motor gain ±50%, latency 0-20 ms, friction and mass
±30%, sensor dropout). Known geometry is installed, not learned: the compass
and path-integration mathematics are exact-rotation integrators, and
training them by policy gradient invites drift for no benefit.

Export: frozen weights and tables serialize to a raw binary blob with a
manifest and SHA-256 content hash; the Phase 1 loader consumes it bit-exact
(hash-verified). RTL consumes the same images as initialized ROM/BRAM
contents, so CPU reference, GPU kernels, and fabric evaluate identical
parameters.

## 12. Verification gates

| Gate | Requirement |
|---|---|
| G1.1 Compass | 10 minutes simulated darkness at 500 Hz: INT16 pipeline drift <= 5 degrees (target 2, model of 7.2); INT32 accumulator shows zero storage-induced drift; float64 oracle cross-check |
| G1.2 Conditioning | PI >= 0.5 after one reinforced pairing; >= 0.7 within six; 60-minute retention < 10% loss; extinction in ~10 unreinforced presentations; all control conditions fail |
| G1.3 Quantization parity | fixed-point vs float32 oracle: KC activation agreement >= 99% under noisy inputs; MBON rank-order preserved >= 99.5% over 1,000 random odors |
| G1.4 Determinism | zero heap allocations; bounded cycle-count variance on a pinned core over 10^6 ticks (p99 jitter < 5%); overflow counters exercised and zero |
| G1.5 Co-simulation | 1 kHz motor loop and 500 Hz policy loop for 10^6 ticks with both MBON routes (goal-weight and direct DN bias) exercised |
| G2.1 Batched rollouts | >= 1,024 agents on the RX 6900 XT (10,000 stretch), >= 50x real-time, parity within G1.3 bounds |
| G2.2 Randomization sweep | CPG + sensor-calibration policy retains >= 90% task success across the randomization grid; coverage committed |
| G2.3 Export | frozen images with manifest and SHA-256; Phase 1 loader consumes bit-exact |
| G2.4 Service discipline | every GPU run inside the fleet's bounded training.slice with pre-flight resource estimates |
| G3.1 RTL parity | cycle-accurate simulation vs Phase 1 golden traces: 10^6 random input vectors, 100% agreement within the quantization spec |
| G3.2 P&R closure | 100 MHz, non-negative slack on XC7A100T; LUT < 20K, BRAM <= 40 blocks, DSP <= 32, DDR controller absent |
| G3.3 On-board timing | policy tick <= 100 us measured on GPIO toggle; motor tick <= 1 us; GF reflex <= 1 ms end-to-end |
| G3.4 Soak | 72 hours closed-loop without hang or counter overflow; board power measured and reported honestly per platform class |
| G3.5 Behavior demo | learned-valence goal rotation through the MBON goal-weight path; looming escape inside budget; 1-2 Hz supervisor tasking with expiry semantics on one board |

## 13. Risk register

| Risk | Mitigation |
|---|---|
| Binary KC activations cost separability on hard odor sets | INT8-sum expansion variant of section 4, still LUT-only; gate G1.3 |
| Literal ring attractor exceeds 32 DSPs at full parallelism | Phase accumulator primary; ring demoted to simulation comparison |
| BRAM tightness on XC7A35T | Ship on the 100T; keep the 35T as a stretch target |
| Kria idle power hides the efficiency story | Measure both board classes; report each scoped figure |
| CubeCL-on-AMD backend churn | Pinned CubeCL commit plus the CPU golden model |
| $\eta$ mis-tuned against the wrong LSB | G1.2 catches it; the 7.1 rule documents it |

## 14. Spike transport versus parallel buses

Biology spikes because unmyelinated axons are leaky cables: passive signals
decay over a length constant of order 0.05 to 0.5 mm, so centimeter-scale
transmission requires regenerative all-or-none pulses, and each axon carries
at most a few hundred rate-coded bits per second. An FPGA routing track at
100 MHz carries 10^8 bits per second, and an 8-bit bus is 8 copper traces
moving a full byte per clock edge. Drosa keeps the functional codes (rates
over behaviorally relevant windows) and drops the physical spike transport;
address-event serialization with queues and sort buffers would be pure
overhead for a controller that can afford parallel buses.

One correction carried from the audit, stated so it is never re-made: the
claim "a fly cannot fit a parallel bus inside a nerve" is wrong as stated.
The neck connective is a physical parallel bus of roughly 3,100 axons
(~1,300 descending, ~1,800 ascending). The binding biological constraint is
per-wire bandwidth, noise, and regenerative transmission energy, not wire
count. Rate coding at 100 to 500 Hz preserves the dynamics of the functions
implemented here (valence integration, heading integration, CPG phase) and
does not preserve sub-millisecond spike-timing codes, which this
architecture does not use.
