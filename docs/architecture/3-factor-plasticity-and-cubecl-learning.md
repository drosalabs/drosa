# Three-factor heterosynaptic plasticity and local online learning

Date: 2026-09-19
Status: implemented CPU arithmetic reference and proposed CubeCL execution contract.
Scope: the plastic Kenyon cell to mushroom body output neuron readout in
Drosa. This document advances Phase 2 preparation; it does not close the
Phase 1 conditioning gates or the Phase 2 device, rollout, and export gates.

The executable reference is [`src/plasticity.rs`](../../src/plasticity.rs),
exported by the dependency-free, `no_std` `drosa` crate. CubeCL kernels, WIT
bindings, browser execution, FPGA synthesis, and allocation instrumentation
remain implementation work. Kernel descriptions below are normative
algorithms, not compiled CubeCL examples.

Evidence classes: **A** denotes measured biology, **B** an engineering
choice or derivation under stated assumptions, and **C** an unsupported
inference that is explicitly excluded. Equations, formats, runtime budgets,
and interfaces are class B unless marked otherwise. References are linked
in section 6 so that this specification is independently usable outside the
internal research workspace.

## 1. Biological derivation and heterosynaptic dynamics

### 1.1 What the experiments establish

**A: Hige et al. (2015) [H15].** The relevant paper is in *Neuron* 88(5),
985-998, not *Nature*. In MBON-gamma1pedc, odor paired with optogenetic
PPL1-gamma1pedc activation induces odor-specific long-term depression of
excitatory input. Figure 3 records depression with action potentials
completely suppressed by intracellular QX-314. Figure 4 compares spiking
and synaptic currents while pairing under voltage clamp. Postsynaptic
spiking is not necessary for this experimentally characterized LTD.

This result excludes a mandatory MBON spike factor in Drosa's baseline.
The implementation does not accept MBON spiking, membrane voltage, or
postsynaptic activity as an update argument. It does not require
postsynaptic depolarization. **C:** complete independence from every local
postsynaptic biochemical or voltage process in every compartment is not
established by these experiments. Somatic voltage clamp does not establish
perfect control of all distal membrane voltages. Hige et al. also leave
open contributions from other inputs presynaptic to the recorded MBON.

**A: Aso et al. (2014) [A14].** KC axons, MBON dendrites, and DAN terminals
form compartmentalized learning circuits. The anatomical scheme identifies
21 MBON types and 15 lobe compartments; these are different counts. Drosa's
21 output rows are a reduced engineering layout, not 21 measured
compartments and not 21 individual neurons. Multiple rows may share a
teacher compartment. A row-to-compartment routing table is required.

**A: Li et al. (2020) [L20].** The adult MB connectome describes KC, DAN,
MBON, feedback, and cross-compartment connectivity. It constrains routing
and shows that the MB is more than an independent bank of feedforward
readouts. It does not determine synaptic efficacy, dopamine dose, receptor
kinetics, or a unique learning equation. Drosa retains a compartment-routed
plastic readout and leaves those recurrent mechanisms outside this module.

The following distinctions govern use of the evidence:

| Statement | Class and scope |
|---|---|
| KC and DAN pairing can depress KC to MBON transmission without MBON spikes | A, Hige's measured preparations, especially gamma1pedc |
| The same rule and time constant apply to every row | B, not measured; Hige Figure 6 shows compartment differences |
| Both reward and punishment can change behavioral preference through depression | A-motivated B reduction: the targeted MBON's downstream valence determines the sign of the behavioral effect |
| All dopamine is a global signed reward or exact TD error | C, not claimed |
| A scalar shared eligibility trace per KC is sufficient | B, conditional on the factorization assumptions in section 3 |
| One to five pairings is a universal biological or controller guarantee | C, not claimed; rapid conditioning is a design target with separate behavioral gates |

### 1.2 Why the name does not imply a postsynaptic multiplier

The biological circuit has three participating elements: KC input, the
modifiable KC to MBON synapse, and heterosynaptic DAN modulation. The
compressed update has two activity-dependent multiplicative terms:

\[
\Delta W_{ji}=-\eta e_i D_{c(j)}.
\]

Here `i` is a presynaptic KC, `j` an output row, and `c(j)` the compartment
routing function. `e_i` encodes recent presynaptic activity. `D_c` is a
bounded teacher pulse. The modifiable synapse and its compartment determine
where the update acts; there is no hidden multiplication by output `y_j`.
The term "three-factor" therefore names the circuit and modulatory learning
family, not a claim that this reduction contains `pre * post * dopamine`.
It is not conventional Hebbian STDP and is not an exact REINFORCE estimator.

For nonnegative weights, a positive pulse depresses eligible inputs to its
target row. Punishment routed to an approach-promoting row reduces approach;
reward routed to an avoidance-promoting row reduces avoidance. The negative
pulse supported by the arithmetic produces potentiation and represents an
optional baseline-centered engineering teacher, not negative dopamine
concentration. The conservative biological baseline uses positive pulses
with opposing output routes.

### 1.3 Eligibility and dopamine timing

**A:** Hige Figure 1 pairs a one-second odor with four one-millisecond light
pulses at 2 Hz, beginning 0.2 s after odor onset. A backward protocol places
odor onset 0.5 s after the last light pulse and does not produce the same
response depression. Figure 6 shows that a short protocol effective in
gamma1pedc is insufficient in alpha2, where a longer pairing is effective.
These are protocol-specific observations, not a fitted universal temporal
window. Handler et al. [H19] further establish receptor-dependent temporal
sensitivity; a single exponential is not a complete account of that biology.

**B:** the CPU reference uses a causal, bounded replacement trace. At policy
tick `t`, with binary active set `A_t` and fixed period `dt`:

\[
e_i(t)=\begin{cases}
1,&i\in A_t,\\
\lambda e_i(t-1),&i\notin A_t,
\end{cases}
\qquad \lambda=\exp(-dt/\tau_e).
\]

A replacement tag is chosen instead of an additive rate trace. Sustained activation renews eligibility to one rather than
accumulating an odor-duration-dependent amplitude. This refines the earlier
`lambda * e + x` sketch and must be recorded as a distinct model variant.

With the engineering default `dt = 2 ms` and nominal `tau_e = 1 s`, a
continuous isolated tag would retain 0.3679 after one second and 0.006738
after five seconds, from `exp(-1)` and `exp(-5)`. Section 2 gives the actual
integer recurrence and its error. No floating-point exponential is evaluated
in the control loop.

A teacher pulse is the bounded, normalized integrated reinforcement dose
assigned to one policy tick. It is consumed once and never held until the
next cue. There is no independent lingering dopamine trace in this baseline:
adding one would allow dopamine-before-cue overlap and change the backward
control. A multi-tick physical burst is represented by explicit per-tick
doses with a declared total area, not repeated copies of the total dose.
No pulse means exactly zero update, even if eligibility remains nonzero.

The caller samples sensory activity and routes the teacher before a tick.
The tick returns the forward response under the previous weights, decays
and refreshes eligibility, then integrates the current dose into weights.
Updated weights affect the next forward evaluation. Delayed reinforcement
can therefore update a currently silent KC with a nonzero recent trace.
A reward sent before any KC activity produces no association. Multiple
independent cues within the trace window can interfere, as expected from
shared presynaptic credit; the rule does not identify a unique cause.

Simulation time, not wall-clock render time, drives decay. A pause performs
no ticks. A timestep change requires a separately quantized retention
coefficient and a recorded model configuration. Training-event admission at
at most 10 events/s is a caller policy, not an internal clock or an
experimentally established biological ceiling.

### 1.4 Acquisition, retention, and extinction

An eligibility trace is not the retained memory: eligibility decays, while
master weights remain fixed when the teacher is zero. Consequently,
unreinforced cue presentations alone cannot undo a depression-only memory.
A test that silently decays weights toward their initial values would not
validate the stated rule.

**A:** Felsenberg et al. [F18] describe extinction through integration of
parallel opposing memories. **B:** the reference's extinction fixture uses
that principle in two reduced output rows. Punishment depresses the
approach row for the trained cue. A subsequent explicit omission teacher,
supplied by the fixture, depresses the opposing avoidance row on cue
re-exposure. The output contrast returns to zero while the original weight
change remains. The fixture does not implement the recurrent circuit that
detects omitted expected punishment, nor claim spontaneous recovery,
renewal, or a fitted ten-presentation extinction time.

Actual one-shot performance index, five-shot learning, retention over an
hour, overlapping-cue generalization, and omission detection remain the
conditioning battery in the connectome specification. A successful
synthetic weight/readout unit test is not a behavioral replication or proof
of the `PI >= 0.5` Phase 1 gate.

## 2. Mathematical formulation and fixed-point arithmetic

### 2.1 State, signal ranges, and forward response

| Quantity | Stored representation | Exact range or scale |
|---|---|---|
| Master weight `w_ji` | signed `i16`, Q8.8 | real value `raw / 256`; full type range `[-128, 127.99609375]` |
| Runtime weight `q_ji` | signed INT8 value derived on read | arithmetic `w_raw >> 8`; no second weight matrix |
| Eligibility `E_i` | `FractionQ15(u16)`, unsigned Q1.15 | `0..=32768`, real value `E / 32768` |
| Retention `L` | same bounded `FractionQ15` | `0..=32768`, real value `L / 32768` |
| Teacher `D_j` | `DopamineQ7(i16)` | `-128..=128`, real value `D / 128` |
| Learning rate `H` | validated `u16`, Q8.8 | `0..=512`, real value `H / 256` |
| Forward accumulator `y_j` | `i32` | sum of derived INT8 weights for unique active KCs |
| Reinforcement event ordinal | `u32` | no wrapping; exhaustion rejects the next nonzero pulse tick |
| Rounding seed | `u32` | explicit, fixed per learner instance |

The eligibility format is 16-bit **unsigned**, unlike signed master
weights. It preserves the 4 KiB vector budget while representing exact zero
and one and enough fractional precision for seconds-scale decay. It
supersedes the earlier signed Q8.8 eligibility sketch for this model.
Teacher format and learning-rate limits are explicit engineering choices.
A rate above two runtime weight units per full pulse is rejected rather
than silently risking overflow; stronger trials require a separately
specified sequence of bounded doses.

The default configuration is `L = 32703`, `H = 128`, with nonnegative weight
bounds `[0, 32767]` in master units. This preserves excitatory KC efficacy;
downstream valence is encoded in output routing, not by letting those
weights become inhibitory. The general numeric API also accepts a signed
box for testing or separately declared models.

For `1 <= K <= 2048`, `1 <= M <= 21`:

\[
q_{ji}=\left\lfloor w_{ji}/256\right\rfloor,
\qquad y_j=\sum_{i\in A_t}q_{ji}.
\]

The floor is significant for negative values: master `-1` reads as `-1`,
not zero. For the dense safety envelope `|A_t| <= 2048`, the sum lies in
`[-262144, 260096]`, derived from `2048 * [-128, 127]`. An `i32` accumulator
therefore cannot overflow. For 100 active cells the bound is
`[-12800, 12700]`. The API validates sorted, strictly increasing in-range
indices before reading or modifying any state. Empty activity is valid;
5 percent sparsity is a producer policy, not a demand to invent activity
when all inputs are absent.

### 2.2 Exact eligibility recurrence and error bound

Every policy tick applies:

\[
E_i^-=(E_i L)\mathbin{\mathrm{>>}}15,
\qquad
E_i'=\begin{cases}32768&i\in A_t\\E_i^-&\text{otherwise.}\end{cases}
\]

The product is at most `32768^2 = 1073741824`, safely representable in
`u32`. Truncation is intentional: for `L < 32768` and positive `E`, the next
inactive value is strictly smaller. There is no round-to-nearest fixed
point that leaves a small eligibility tag alive forever. `L = 32768`
explicitly holds eligibility; `L = 0` clears inactive traces each tick.

Relative to exact repeated multiplication by `lambda_q = L / 32768`, an
isolated full tag with `0 <= lambda_q < 1` has the following floor error in
raw units after `n >= 1` inactive ticks:

\[
0\le 32768\lambda_q^n-E_n
<\sum_{r=0}^{n-1}\lambda_q^r
=\frac{1-\lambda_q^n}{1-\lambda_q}.
\]

For the default, `1 / (1-lambda_q) = 32768 / 65`, so the asymptotic
normalized absolute error is less than `1/65`, approximately 0.01539.
This is an absolute bound, not a small relative-error promise in the tail.
The chosen `32703` is nearest to `32768 * exp(-0.002)`; the coefficient
error is less than half a Q15 unit. The tests check approximately 36 to
37 percent retention at 500 ticks and exact eventual extinction of the tag.
They do not equate tag decay with synaptic forgetting.

### 2.3 Exact depression, stochastic rounding, and overflow proof

The ideal change in raw master units for one pulse is

\[
\Delta w_{ji}^{*}=-\frac{H E_i D_j}{2^{22}},
\qquad 2^{22}=32768\cdot128.
\]

Define the unsigned magnitude product and quotient/remainder:

\[
P=H E_i |D_j|,\quad Q=P\mathbin{\mathrm{>>}}22,
\quad R=P\mathbin{\&}(2^{22}-1).
\]

For a rounding sample `U` in `[0, 2^22)`:

\[
B=Q+\mathbf{1}[U<R],\quad
w'_{ji}=\operatorname{clamp}
\bigl(w_{ji}-\operatorname{sgn}(D_j)B,w_{\min},w_{\max}\bigr).
\]

The exact threshold comparison is strict. A zero remainder never rounds up,
including when `U = 0`. Symmetric magnitude rounding avoids a sign-dependent
bias from shifting a negative fractional product.

All intermediate bounds are derived from validated public constructors:

- `P <= 512 * 32768 * 128 = 2147483648`, which fits `u32`, but exceeds
  `i32::MAX` by one. The product must remain unsigned on CPU and device.
- `0 <= B <= 512`. Widened signed subtraction lies in `[-33280, 33279]`.
- Box projection occurs in `i32` before narrowing to `i16`. No signed
  overflow, wrapping subtraction, `i16::MIN.abs()`, or 64-bit device integer
  is needed. Dopamine's validated minimum is `-128`, not `i16::MIN`.
- A reinforcement ordinal increments only once for a tick containing at
  least one nonzero row pulse. At `u32::MAX`, such a tick is rejected before
  forward/trace/weight mutation; zero-pulse ticks remain legal.

The standalone `PlasticityConfig::update_weight` also projects an input
outside the configured box. `Plasticity` initializes inside the box and
never exposes mutable weights, so a zero dose preserves every reachable
weight exactly. Failed constructors and invalid active sets return typed
errors rather than partially updating state.

### 2.4 Why quantization does not freeze local learning

There are two quantization boundaries, and they require different remedies.

**INT8 boundary:** one runtime LSB equals 256 master LSBs. A full-eligibility,
full-dose pulse with `H = 1` changes the master by exactly one unit. Starting
at master `2303`, 255 such depressions reach `2048` while the runtime byte
remains 8; the 256th reaches `2047` and the runtime byte becomes 7. The low
byte is the accumulation state. Rounding every pulse back to INT8 would
lose these changes. No separate per-synapse optimizer accumulator is needed.

**INT16 boundary:** an update of half a master LSB would still stall if
rounded deterministically to zero. With uniform `U`, the threshold rule has
`Pr(round up) = R / 2^22` and `E[B] = P / 2^22` before clipping. For an
ideal constant sub-LSB update with probability `p`, the probability of no
step in `n` independent trials is `(1-p)^n`. Stochastic rounding avoids a
systematic zero-update region; it cannot guarantee motion on every finite
trial sequence. Clamping at a weight bound deliberately breaks unbiasedness.

The implementation's `rounding_sample(seed, event, synapse)` is a stateless
32-bit integer mixer, keyed by `synapse = j*K+i`. It uses explicit wrapping
arithmetic and the constants in `src/plasticity.rs`; only its low 22 output
bits are compared. The mixer is a reproducible engineering pseudorandom
source, not a cryptographic or experimentally validated independent random
process. The expectation above assumes uniform samples. Exact threshold
boundary tests and a fixed-seed repeated half-LSB test verify the
implementation separately from that assumption.

The same key tuple is used regardless of launch geometry, inactive-row
skips, or packed storage padding. Per-agent seeds must be exported; GPU
thread scheduling never supplies randomness. Weights, eligibility,
configuration, seed, and next event ordinal together determine a restart.
Resetting the ordinal alone changes the rounding trajectory.

A deterministic residual accumulator is a valid alternative if exact
fractional error feedback is required, but a general per-synapse residual
matrix adds `O(KM)` memory and invalidates the current BRAM ledger. It is
not silently added. Full-dose integer-master events need no randomness and
are bit-exact regardless of the seed.

One-shot changes in an INT8 readout are not guaranteed merely by having
100 coactive cells: identical fractional phases can all miss the same
quantization boundary. The unit fixture uses `H = 512`, an exact two-INT8-LSB
change per fully eligible synapse at full dose, avoiding that ambiguity. Smaller
rates need a behavioral or quantization-phase distribution argument, not a
population-size assertion.

## 3. Eligibility trace factorization

### 3.1 Conditional proof of the O(K) dynamic trace state

Assume the following reduced-model properties:

1. Every synapse receiving KC `i` uses the same presynaptic activity stream,
   replacement rule, decay coefficient, and initial eligibility.
2. Eligibility does not depend on target-row spiking, local voltage,
   synaptic weight, branch-specific activity, or row-specific biochemical
   history.
3. Compartment differences affect the routed teacher and static synaptic
   support, not eligibility recurrence. Each row receives one effective
   teacher per tick, with deterministic preprocessing of multiple DAN inputs.

If a hypothetical matrix `E_ji(0)` has identical rows, then applying the same
scalar recurrence to each `E_ji` at tick `t` produces identical values for
all `j` at tick `t+1`. Induction gives `E_ji(t) = E_i(t)` for every tick.
In matrix notation the redundant trace matrix is `1_M e^T`, of rank at
most one, and the unclipped ideal update is

\[
\Delta W=-\eta\,\mathbf D\,\mathbf e^{\mathsf T}.
\]

Clipping and independent rounding can make the realized update matrix
higher rank; neither introduces a need to store a trace matrix. A static
connectivity mask can restrict which weights exist without becoming a
dynamic per-synapse trace. The implemented reduction uses a dense KC-to-row
weight bank with sparse current input.

The dynamic trace memory is `2K` bytes, plus `O(C)` teacher input and `O(M)`
static routing outside the CPU learner. This is `O(K+C+M)`, conventionally
`O(K)` for fixed compartment and output counts. The weight matrix itself
remains `O(KM)`; factorization does not compress retained associations.

| Layout | K = 2,000 software profile | K = 2,048 FPGA profile |
|---|---:|---:|
| Per-KC 16-bit trace vector | 4,000 B, 3.90625 KiB | 4,096 B, 4 KiB |
| Redundant 21-row 16-bit trace matrix | 84,000 B, 82.03125 KiB | 86,016 B, 84 KiB |
| Trace bytes avoided | 80,000 B | 81,920 B, 80 KiB |
| Actual 21-row INT16 master bank | 84,000 B | 86,016 B, 84 KiB |

Thus "4 KB rather than 84 KB" refers precisely to 4 KiB versus 84 KiB
for the 2,048-cell profile. The Rust biological anchor remains 2,000 cells;
no public population constant is changed. Tests verify the trace byte counts
using `size_of`. The hardware totals remain the previously audited 124 to
165 KB within the conservative 181 KB envelope; those are FPGA configuration
budgets, not a GPU allocator footprint or a new placement result.

### 3.2 Limits of the proof and compute cost

Hige's absence of a required postsynaptic spike term motivates, but does
not prove, these sharing assumptions. Compartment-specific eligibility
kinetics can require `O(KC)` traces even without postsynaptic spiking.
Synapse-specific biochemical state can require `O(KM)`. An actor score
trace contains an action-dependent factor and generally does not factorize.
Those model variants need a revised memory ledger.

Current activation sparsity and historical eligibility sparsity are not
identical. At 5 percent activity, forward evaluation uses about 100 of
2,000 cells. A sequence of different cues can leave all 2,000 traces
nonzero. Trace maintenance is `O(K)` each policy tick; reinforcement is
worst-case `O(KM)` per event, or 42,000 logical synapses for the software
profile. It is not safely budgeted as `100 * 21` updates. An optional
eligible-index compaction must reserve capacity `K`, not `0.05K`, and prove
that its overhead is useful before adoption. The baseline scans all KCs,
skips zero-teacher rows, and leaves zero-eligibility weights unchanged
without allocation.

## 4. CubeCL kernel architecture

### 4.1 Target and dependency discipline

The compute language is Rust CubeCL, with WGPU as the native/device runtime
and WGSL as the browser shader backend [CUBE]. Native RX 6900 XT execution
uses a selected WGPU adapter, ordinarily Vulkan on Linux. No CUDA, ROCm,
subgroup width, native 16-bit integer arithmetic, or GPU int64 support is
assumed. WGSL's portable integer scalar types are `i32` and `u32` [WGSL].
Packed 8/16-bit values are decoded explicitly into those types.

Upstream source inspected for this specification is immutable revision
`2a3c997dafc43081d58c1417111c35c726dc46d6`, reporting CubeCL
`0.11.0-pre.4`, WGPU `^30.0.0`, and workspace Rust minimum `1.95` [PIN].
Drosa currently pins Rust `1.94.0`. This revision is a **research reference,
not an adopted dependency pin or a verified compatible build**. The future
device crate must select and lock a compatible commit and transitive
versions, or obtain a separate toolchain-change decision, before kernel
implementation is admitted. The root crate gains no device dependencies in
this change. Unpinned `main`, wildcard versions, and incidental upstream API
names are not an executable build specification.

CPU/WebAssembly is the authoritative single-agent browser baseline, as in
the existing visualizer architecture. CubeCL browser execution is an
optional parity-gated backend or batched simulation mode, not a second
independent dynamics implementation in handwritten renderer WGSL. Only one
backend owns a learner's mutable state at a time.

### 4.2 Buffer layout and ownership

All sizes are fixed at initialization. Let `B` be the batch size,
`P = ceil(K/2)`, and `A_cap = 128` for either default sparse profile.
The 5 percent producer caps are 100 for K=2,000 and 102 for K=2,048,
computed by flooring `0.05K`; the extra capacity is padding, not extra
activation. Dense validation uses a distinct initialization profile.

| Buffer | Device representation and indexing | Single-agent bytes at K=2,048, M=21 |
|---|---|---:|
| `masters` | `u32[B,M,P]`, two signed 16-bit masters per word | 86,016 |
| `eligibility` | `u32[B,P]`, two unsigned 16-bit Q15 traces per word | 4,096 |
| `active_ids` | `u32[B,A_cap]`, sorted unique indices, stale tail ignored | 512 |
| `active_bits` | `u32[B,ceil(K/32)]`, full bitmap refreshed each tick | 256 |
| `teachers` | `i32[B,M]`, validated sign-extended Q7 pulses | 84 |
| `outputs` | `i32[B,M]`, pre-update responses | 84 |
| `parameters` | fixed 64-byte block per agent: count, seed, ordinal, gain, decay, bounds, stamps and flags | 64 |
| `diagnostics` | four `u32` words per agent: clipped count, changed-row bits, status, committed ordinal | 16 |

The listed device payload is `91,128 B` per agent. At B=1,024 it is
`93,315,072 B`, approximately 89 MiB, before staging, snapshots, alignment,
compiler/runtime pools, sensory expansion, environment state, and graphics.
This is an arithmetic lower-level payload estimate, not a preflight VRAM
request for a complete rollout. Default WebGPU per-binding size limits
must be checked against the largest bank and any requested batch; larger
batches are partitioned into predeclared chunks at initialization.

Rows are padded independently to an even number of KCs. The final unused
halfword for an odd K is zero and never becomes a logical synapse. Low
16 bits hold KC `2p`, high 16 bits hold KC `2p+1`. Signed unpack uses
`v < 32768 ? i32(v) : i32(v)-65536`; eligibility remains unsigned.
Serialization uses explicit little-endian words, not host struct layout.
Rust's logical `[[SynapticWeightQ16; K]; M]` and `[FractionQ15; K]` are not
cast to device bytes through unsafe code.

There is no baseline INT8 cache: sparse forward reads the high byte of each
unpacked master. A later cache would add `K*M` bytes and require its own
word-owner packing dispatch after updates. Two lanes must never race to
write separate bytes or halfwords of the same `u32`.

### 4.3 Sparse KC to MBON forward kernel

**Entry contract:** validated inputs; immutable masters and active indices;
one complete output per `(agent,row)`. A workgroup has 64 units and 256 B
of workgroup `i32` scratch, independent of hardware subgroup size.

```text
cube = (agent, row)
unit = 0..63
sum = 0
for slot = unit; slot < active_count[agent]; slot += 64
    kc = active_ids[agent, slot]
    word = masters[agent, row, kc / 2]
    weight = signed_halfword(word, kc % 2)
    sum += arithmetic_shift_right(weight, 8)
scratch[unit] = sum
workgroup_barrier
for distance = 32, 16, 8, 4, 2, 1
    if unit < distance
        scratch[unit] += scratch[unit + distance]
    workgroup_barrier
if unit == 0
    outputs[agent, row] = scratch[0]
```

No unit exits early before a barrier, including empty activity and padded
slots. Grid dimensions are exactly the validated batch and row counts.
The shared reduction is fixed and overflow-free by section 2.1. At
K=2,000, M=21, 100 active KCs imply 2,100 logical gathers/adds per agent per
tick, rather than 42,000 dense contributions. Packed-word loads are nominally
8,400 B before caching, not 2,100 physical bytes. Irregular gather latency
and launch overhead require measurement; 5 percent sparsity is not a
measured 20-fold wall-clock speedup. A one-unit-per-row sequential gather is
a legitimate precompiled alternate for very small batches, selected only
by bounded initialization benchmarking.

### 4.4 Eligibility maintenance kernel

One unit owns one packed eligibility word `(agent,p)`. It reads both old
halfwords, multiplies each by the validated retention in `u32`, shifts by
15, and replaces either result by 32768 if the corresponding active bitmap
bit is set. It writes one complete packed word. Tail halfwords are zero.
The grid covers `B*P` words with a fixed 64-unit cube and a bounds predicate.
There are no cross-unit barriers, atomics, or per-synapse trace writes.

The active bitmap and list must describe the same activity set. The CPU
adapter builds both from a validated list into reused storage. A device
producer must perform deterministic bounded selection and emit both
representations consistently. Bitmap words are overwritten even on an
empty tick to prevent stale eligibility refreshes.

### 4.5 Parallel reinforcement kernel

One unit owns `(agent,row,p)`, a packed pair of weights. It reads the row's
teacher, the updated eligibility word, and its old master word. Both
logical weights are processed using the exact unsigned product, threshold,
sign, and signed clamp in section 2.3, with independent logical
`j*K+i` rounding keys. The unit then writes one complete master word.
An all-zero row teacher can return immediately because this kernel has no
workgroup barrier. A zero trace leaves its in-box weight unchanged.

With cube dimensions `(64,1,1)`, the grid is `(ceil(P/64),M,B)` and
`p = 64*cube_x + unit_x`. Units with `p >= P` return before reading data.
All grid axes and flattened buffer indices must pass adapter-limit and
`u32` range checks at initialization; oversized batches use fixed chunks.
This avoids flattening a large batch into an illegal one-dimensional grid.
The dispatch scans `B*M*P` words, even if only about 5 percent of KCs are
active now. With K=2,048 and M=21, there are 21,504 pair-owning units per
agent, or 43,008 potential logical updates. No floating-point atomics,
weight atomics, scatter collisions, gradient buffers, optimizer state,
autodifferentiation, or master-to-runtime requantization allocation exists.

Optional diagnostics use integer `atomicOr` into one changed-row bitmask
and `atomicAdd` for clipped synapses. Their fixed words are cleared before
the event dispatch. A per-event clipped count is at most `K*M = 43,008`,
so it cannot wrap; cumulative telemetry counters are separately saturating
host values. Diagnostic races must not feed back into learning. Doses for
a shared compartment are routed before dispatch; row expansion changes
neither the shared eligibility nor the memory proof.

### 4.6 Dispatch ordering and publication

A single ordered queue owns a batch. Each tick follows:

```text
validate and stage tick input into an available fixed slot
reset per-tick diagnostic words
forward using W(t)
advance eligibility from activity at t
apply current teacher using the updated eligibility
commit event ordinal and snapshot stamp
publish completed pre-update output and post-update learning state
```

Separate dispatches provide the required device-wide ordering. A
workgroup barrier cannot order different kernels or independent workgroups.
The event ordinal commits once per reinforced agent, not once per pair;
a one-owner finalize dispatch or serialized host metadata commit provides
that ownership. GPU errors fail the tick and prevent snapshot publication.
A device-loss recovery restores a complete checkpoint outside the sealed
loop; a half-updated bank is never accepted as a valid checkpoint.

No forward pass, renderer, export, or other stream reads the master bank
concurrently with mutation. Queue-ordered copies write a free snapshot
slot, and only completed slots become visible to consumers. The renderer
cannot retain a mutable master binding. Snapshot headers explicitly label
`response_tick`, `weights_after_tick`, and `reinforcement_ordinal` to
avoid presenting pre-update output as a post-update evaluation. A new
forward response requires a real subsequent evaluation, not interpolation
of weight deltas in the renderer.

### 4.7 Pure CubeCL memory-sealed allocation standard

"Memory sealed" means that Drosa's steady-state simulation has a fixed
storage plan. It is a lifecycle contract, not a property conferred merely
by writing `#[cube]` functions.

1. **Initialize:** validate shapes, adapter features and buffer limits;
   allocate all master, trace, input, output, diagnostic, staging, and
   snapshot buffers through the selected CubeCL runtime. Create all
   permitted shape/kernel specializations and bindings. No Burn tensor,
   handwritten learning WGSL, external optimizer, or hidden fallback tensor
   allocation belongs on the learning path.
2. **Warm and seal:** exercise all permitted event masks and empty/full
   input cases; compile every selected variant; fix launch dimensions and
   specialization choices; disable hot-loop autotuning and lazy variant
   compilation. Preallocate the worst-case eligible population and a fixed
   number of in-flight slots. Record handles, capacity, and allocation
   counters. Capture/reuse a launch graph only if the pinned backend supports
   it and its ownership obligations are met.
3. **Run:** use only existing handles and fixed-capacity storage. No
   `create`, `empty`, resize, collection growth, per-event shader compilation,
   allocating synchronous readback, or new device buffer is permitted.
   Reuse staging slots only after submission completion. If no input slot
   is free, apply bounded backpressure; if no observer slot is free, drop
   the observation, never an accepted learning event. No unbounded queue or
   implicit allocation is a fallback.
4. **Reconfigure:** stop admission, drain work, unseal, resize or migrate
   outside the control loop, then repeat validation and warmup. Runtime
   shape changes and device recovery are never disguised as sealed ticks.

There are two separate acceptance tests. The Drosa CPU core must have zero
heap allocations in successful and rejected ticks. The device model must
make zero new storage allocations or pool reservations after sealing.
A stronger claim of zero allocations across all host dispatch/runtime code
also needs allocation instrumentation and a pinned implementation audit.
It cannot be inferred from stable GPU handles.

**Observed upstream limitation [CLIENT]:** the inspected CubeCL client uses
`Vec`, `Box`, and allocating descriptor/readback paths; its `write` method
constructs a vector even when targeting an existing handle. Its WGPU graph
path re-encodes dispatches rather than reusing a hardware command graph.
Graph capture and persistent pools therefore do not establish zero host
allocation. A strict all-runtime zero-allocation adapter must prove or
repair these paths in a separately scoped implementation. Until then, the
CubeCL sealed-mode gate is open, not passed. The reference's ordinary CPU
core is the allocation-free fallback; WebGPU drivers and browser internals
are not claimed to be hard-real-time or allocation-free.

Acceptance instrumentation covers at least one million simulated ticks,
empty inputs, full-capacity inputs, alternating teacher signs, dense
eligibility, saturated weights, staging backpressure, and observer
starvation. It records buffer creation count, pool high-water bytes, host
allocation/reallocation counts, and queue depth. Device allocation failure
or a forbidden operation fails closed. Constant pool occupancy alone does
not prove zero allocation calls. These tests are proposed, not run here.

### 4.8 Resource and parity gates

CPU unit verification is lightweight and needs no GPU. Local device
verification, when authorized, runs in a bounded user service under
`training.slice`, with explicit `MemoryMax`, CPU quota, task and wall-time
limits, `Restart=no`, no auto-start, and completion/failure CLADE hooks.
Preflight states VRAM including runtime/graphics reserves, CPU utilization,
and the share of the configured slice ceiling. A nominal VRAM payload in
section 4.2 is not authorization to allocate an entire free GPU.

Bit-exact parity gates compare master words, eligibility, event ordinals,
pre-update outputs, and clipping counts against the CPU reference. The
corpus includes K=1, odd K, 2,000 and 2,048; empty and maximum activity;
all trace endpoints; positive, negative, and zero pulses; all weight box
edges; sub-byte and sub-master steps; nonmultiple workgroup tails; wrong-row,
backward, and expired pairings; and repeated seeds. Test both native WGPU
and browser WebGPU before either is admitted. No throughput or parity
measurement is claimed in this document.

## 5. Integration blueprint for the drosa crate

### 5.1 Module boundaries and current executable API

| Surface | Status and responsibility |
|---|---|
| `src/lib.rs` | implemented: `no_std`, unsafe forbidden, public `plasticity` module and existing `SynapticWeightQ16` |
| `src/plasticity.rs` | implemented: bounded signal types, configuration, scalar update, counter-keyed rounding, fixed-array learner, tests |
| `crates/drosa-cubecl/src/` | proposed: packed codecs, CubeCL forward/trace/update kernels, fixed-buffer runtime and parity harness |
| `crates/drosa-component/wit/learning.wit` | proposed: typed interoperable control interface |
| `crates/drosa-component/src/` | proposed: fixed-capacity slot validation, teacher routing, checkpoint and snapshot adapters |
| Browser render bridge | proposed extension of the existing visualizer design; display-only consumer |

The device implementation belongs in a separate crate so that native WGPU,
JIT, and WebAssembly binding dependencies cannot enter the `no_std` core.
The proposed paths are not created as empty scaffolding. CubeCL expansion
may require unsafe launch wrappers; such code cannot enter a crate with
`forbid(unsafe_code)` and requires its own reviewed boundary.

The present API has the following semantics:

- `FractionQ15::from_raw` accepts `0..=32768`.
- `DopamineQ7::from_raw` accepts `-128..=128`.
- `PlasticityConfig::new` validates retention, gain, and weight interval.
- `Plasticity::<K,M>::new(config, initial_weight, seed)` allocates nothing;
  the fixed arrays are part of the returned value. Its owner chooses stack,
  static arena, or startup heap placement. Large values must not be copied
  through a small embedded stack on every tick.
- `forward(active)` validates the sparse list and returns an `[i32; M]`.
- `step(active, row_pulses)` returns the pre-update response and performs
  the ordered eligibility and learning transition. Inputs are borrowed;
  no `Vec`, `Box`, allocator, global state, floating point, or unsafe code
  is used. It is a policy tick, not a motor tick.
- `weights()`, `eligibility()`, and `reinforcement_events()` expose immutable
  observations. Configuration and seed remain with the owning adapter.

This module does not generate KC codes, assign action valence, detect reward
omission, implement a DAN baseline estimator, enforce wall-clock teacher
rate limits, serialize state, or launch devices. Those explicit absences
keep a small numerical reference from masquerading as a complete controller.

### 5.2 WIT types and bounded transport

The following WIT is a proposed control-plane shape, not generated or
parser-tested bindings. Fixed scalar arguments avoid a `list` allocation
on every tick. A resource owns two preallocated input slots; all pending
teacher fields start at zero, and each successful submission consumes and
clears its input slot before reuse.

```wit
package drosa:learning@0.1.0;

interface local-learning {
    record configuration {
        kenyon-cells: u32,
        output-rows: u32,
        retention-q15: u16,
        learning-rate-q8: u16,
        minimum-weight-q8: s16,
        maximum-weight-q8: s16,
        initial-weight-q8: s16,
        rounding-seed: u32,
    }

    record observation-stamp {
        response-tick: u64,
        weights-after-tick: u64,
        reinforcement-ordinal: u32,
    }

    enum learning-error {
        invalid-configuration,
        invalid-active-set,
        invalid-teacher,
        invalid-slot,
        slot-busy,
        not-ready,
        stale-tick,
        stale-observation,
        counter-exhausted,
        device-lost,
    }

    resource learner {
        set-active: func(slot: u32, position: u32, kc: u32)
            -> result<_, learning-error>;
        set-teacher: func(slot: u32, row: u32, dose-q7: s16)
            -> result<_, learning-error>;
        submit: func(slot: u32, tick: u64, active-count: u32)
            -> result<observation-stamp, learning-error>;
        output: func(tick: u64, row: u32)
            -> result<s32, learning-error>;
    }

    create: func(config: configuration) -> result<learner, learning-error>;
}

world learning-component {
    export local-learning;
}
```

The adapter specializes permitted dimensions at initialization; runtime
shape discovery never instantiates arbitrary const generics. `set-active`
writes a bounded slot, not an append-only collection. A fixed validity
bitmap ensures that every position below `active-count` was written in the
current slot generation. `set-teacher` replaces one pending dose rather
than adding duplicate delivery. `submit` checks strictly increasing ticks,
unique sorted KC indices, slot ownership, and all signal ranges before
mutating the learner. Errors retain a defined editable slot without
consuming an event; a repeated successful tick is rejected to prevent
double reinforcement. For an asynchronous device adapter, the returned
stamp identifies accepted work, not completed device execution. Input
storage stays busy until its consuming commands complete. Output lookup
returns `not-ready` for accepted but incomplete work and `stale-observation`
for an expired observation; only a retained complete snapshot yields data.
A host completion callback schedules observation, not a synchronous GPU
readback or a busy-poll loop. Device loss invalidates pending stamps.

Scalar WIT setters trade call overhead for a clear portable type boundary.
A browser-specific bulk path may fill a preallocated Wasm linear-memory
slot via versioned byte offsets, but those offsets are local to that
instance and are not portable WIT pointers. `memory.grow`, temporary
canonical-ABI lists, JSON conversion, and recreated typed arrays are outside
the sealed loop. Canonical ABI lowering and generated bindings need their
own allocation audit; WIT syntax alone is not an allocation guarantee.

### 5.3 Teacher routing and checkpoint identity

The adapter holds `row_to_compartment[M]` and a calibrated policy mapping
reward/punishment/omission events to signed doses per compartment. Each
row has a separately documented downstream approach/avoidance coefficient.
A shared compartment supplies the same dose to all its rows. A wrong-row
control means that the expected behavioral contrast does not change as
trained; it need not mean that no other row learns.

Simultaneous physical pulses are normalized and combined in widened
arithmetic before a single bounded dose is quantized. Silent per-event
clipping is forbidden: an out-of-range aggregate is rejected or rescheduled
under an explicitly different protocol. Replaying individually rounded
pulses is not generally identical to rounding their sum, especially near
weight bounds. Event grouping is therefore part of checkpoint identity.

A future restart image includes schema/model version, K/M, row-major INT16
masters, unsigned Q15 eligibility, gain/retention/weight bounds, policy tick
period, rounding seed, next reinforcement ordinal, compartment routing,
valence mapping, and the frozen sensory representation's content hash.
Binary integer fields have declared little-endian encoding and exact
lengths; the manifest and payload have a content hash. A changed sensory
projection invalidates learned cue identities even when K and M match.
Invalid images are rejected before any active state changes. The current
module has no loader, so serialization parity remains an integration gate.

### 5.4 WebGPU visualizer hooks

The visualizer remains a pure consumer of exported engine state. In the
previously specified single-agent architecture, Rust compiled to Wasm owns
learning in a Worker and direct WebGPU renders snapshots; that integration
is not implemented by this change. A future CubeCL mode owns
learning on the device and exports queue-ordered snapshots through its
runtime adapter. It does not expose a concurrent mutable weight bank to
renderer shaders or recompute plasticity in TypeScript.

A fixed snapshot ring carries the existing tick/frame fields plus:

| Field | Presentation use |
|---|---|
| KC active bitmap | highlight current cue activity |
| KC unsigned Q15 trace slice | show eligibility fading independently of current activity |
| Signed Q7 teacher vector and event ordinal | pulse the targeted compartments; distinguish zero from omitted display frames |
| Selected INT16 master row and derived INT8 view | show sub-byte accumulation and the actual runtime quantization boundary |
| Pre-update MBON outputs and explicit stamps | display the response actually returned by the tick |
| Changed-row mask and clipped count | select a bounded refresh and indicate saturated learning |

For K=2,000 a full trace vector is 4,000 B and one selected weight row is
4,000 B. Sending both at 60 Hz costs 480,000 B/s before headers, bitmap,
and output state, from `8000 * 60`. Streaming all 21 master rows would cost
5,040,000 B/s before other state; selected rows are the baseline. These are
payload calculations, not measured browser bus rates. A fixed ring can drop
stale visual frames but never change the number or timing of learning ticks.

The renderer converts raw values to display units, colors and geometry.
It does not infer a reward from a color, advance eligibility by render
elapsed time, or infer an INT8 behavioral change merely because a master
changed. Pending GPU readbacks use preallocated asynchronous slots; allocating
readback utilities cannot satisfy the sealed standard. SharedArrayBuffer
is optional and requires its separate deployment/security preconditions;
the existing transferable-buffer pool remains the baseline.

### 5.5 Verification status and next gates

The bounded native verification on 2026-09-19 passes 43 unit tests: 24
existing foundation tests and 19 plasticity tests. `cargo check --offline
--lib`, `cargo clippy --offline --all-targets -- -D warnings`, and
`cargo fmt --check` pass. `cargo package --offline --list --allow-dirty`
contains eight package files, including the new source module and no
architecture documents or development-cache files. The run uses
`nix develop` with the existing Rust 1.94.0 pin, `CARGO_BUILD_JOBS=1`, one
test thread, no device dependencies, and no GPU work. Its service has
`MemoryMax=2G`, `CPUQuota=100%`, and a 180-second wall-time limit under
`training.slice`; systemd reports approximately 397 MiB peak memory and
3.496 s service runtime. These are local CPU verification measurements,
not a learner throughput benchmark.

The unit battery covers 100-of-2,000 one-shot cue-specific readout changes,
explicit opponent-teacher extinction without memory erasure, cue-alone
and idle retention, delayed and backward/expired pairing, replacement
rather than accumulating traces, seconds-scale decay, INT8 and sub-master
quantization, replay, full-range saturation, invalid input atomicity,
zero-rate freezing, event exhaustion, factorized storage, and sparse/dense
forward parity at signed limits. An independent signed 64-bit rational
oracle checks 8,232 boundary combinations (`6` gains, `7` eligibility
values, `7` doses, `7` starting weights, and `4` rounding samples) against
the portable `u32` implementation.

Remaining release gates are explicit:

1. Add the omitted full-controller conditioning and float-oracle battery;
   measure acquisition and extinction behavior rather than treating these
   arithmetic fixtures as performance-index tests.
2. Resolve the CubeCL/toolchain pin, implement the packed kernels and
   fixed-buffer adapter, and run the exact CPU/device/browser parity corpus.
3. Instrument all allocation boundaries and enforce the memory-sealed
   acceptance test, including runtime dispatch and observer starvation.
4. Implement and validate WIT bindings, image import/export and visualizer
   integration, then perform governed GPU scaling and FPGA co-simulation.

## 6. References and related specifications

- **[H15]** Hige T, Aso Y, Modi MN, Rubin GM, Turner GC (2015).
  *Heterosynaptic plasticity underlies aversive olfactory learning in
  Drosophila*. Neuron 88(5), 985-998.
  [DOI](https://doi.org/10.1016/j.neuron.2015.11.003),
  [full text and Figures 1, 3, 4, 6](https://pmc.ncbi.nlm.nih.gov/articles/PMC4674068/).
- **[A14]** Aso Y et al. (2014). *The neuronal architecture of the mushroom
  body provides a logic for associative learning*. eLife 3:e04577.
  [Article](https://elifesciences.org/articles/04577).
- **[L20]** Li F et al. (2020). *The connectome of the adult Drosophila
  mushroom body provides insights into function*. eLife 9:e62576.
  [Article](https://elifesciences.org/articles/62576).
- **[H19]** Handler A et al. (2019). *Distinct dopamine receptor pathways
  underlie the temporal sensitivity of associative learning*. Cell.
  [DOI](https://doi.org/10.1016/j.cell.2019.05.040).
- **[F18]** Felsenberg J et al. (2018). *Integration of parallel opposing
  memories underlies memory extinction*. Cell 175(3), 709-722.e15.
  [DOI](https://doi.org/10.1016/j.cell.2018.08.021),
  [full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC6198041/).
- **[CUBE]** CubeCL project, programming model and runtime targets.
  [README at the inspected revision](https://github.com/tracel-ai/cubecl/blob/2a3c997dafc43081d58c1417111c35c726dc46d6/README.md).
- **[PIN]** CubeCL workspace versions and compiler minimum.
  [Manifest at the inspected revision](https://github.com/tracel-ai/cubecl/blob/2a3c997dafc43081d58c1417111c35c726dc46d6/Cargo.toml).
- **[CLIENT]** CubeCL runtime client, persistent allocation, writes, readback,
  graph capture and replay.
  [Source at the inspected revision](https://github.com/tracel-ai/cubecl/blob/2a3c997dafc43081d58c1417111c35c726dc46d6/crates/cubecl-runtime/src/client.rs).
- **[WGSL]** W3C, *WebGPU Shading Language*, scalar types, memory layout,
  integer arithmetic and synchronization.
  [Specification](https://www.w3.org/TR/WGSL/).
- **[WEBGPU]** W3C, *WebGPU*, device limits, buffers and command submission.
  [Specification](https://www.w3.org/TR/webgpu/).
- **[WIT]** WebAssembly Component Model, WIT types and resources.
  [WIT reference](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md).

Related Drosa documents:

- [Connectome specification](connectome-specification.md): biological
  organization and the full-controller conditioning gates. The present
  document refines its trace recurrence, format, compartment count, and
  factorization assumptions for the implemented local-learning variant.
- [Hardware mapping](hardware-mapping.md): FPGA memory ledger and master
  weight contract. Eligibility-vector capacity is preserved; worst-case
  reinforcement cost includes all nonzero historical traces.
- [Visualizer implementation architecture](../vision/webgpu-interactive-visualizer-architecture.md):
  authoritative dynamics/rendering separation, pooled snapshot transport,
  and browser deployment model.
