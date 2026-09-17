# Drosa Connectome Specification: Biological Basis and Computational Model

Date: 2026-09-17
Status: Specification of record for the Drosa computational architecture
Sources: audited research ledger in `~/main` (see section 9); every claim
below is labeled with its evidence class and source index.

---

## 1. Scope, provenance, and evidence discipline

This document specifies the four computational subsystems of Drosa: sensory
expansion, the Central Complex ring attractor, Mushroom Body three-factor
plasticity, and motor convergence. For each subsystem it states the
biological substrate, the computational model implemented in silicon, the
declared engineering simplifications, and the storage and timing interfaces.

### 1.1 Reference connectomes

| Resource | Specimen and coverage | Verified counts | Publication |
|---|---|---|---|
| FlyWire / FAFB | one adult female brain, central brain and both optic lobes; no ventral nerve cord | 139,255 neurons; ~54.5 million chemical synapses between reconstructed neurons | Dorkenwald et al., Nature 2024 [F1] |
| FlyWire annotation atlas | hierarchical classes, cell types, hemilineages | 8,453 annotated cell types | Schlegel et al., Nature 2024 [F2] |
| MaleCNS v1.0 | one adult male central nervous system, brain plus ventral nerve cord | 166,700 neurons; 11,710 types; released 2026-06-08 | Berg et al., Cell 2026 [M1], official release pages [M2] |

Drosa is connectome-inspired, not a connectome emulation. An EM connectome
establishes cell identities, processes, chemical contacts, and topology. It
does not supply membrane dynamics, synaptic efficacies, receptor complements,
propagation delays, sensory transduction, a loss function, or a learning
rule. Where Drosa needs those, it either imports a value measured in
physiological experiments (cited per claim) or declares an engineering
choice. Nothing in this specification claims whole-fly fidelity.

### 1.2 Evidence classes

Every substantive claim below carries one label:

- **A, measured biology**: anatomical observation, curated reconstruction,
  or physiological/behavioral experiment.
- **B, engineering choice**: a model form, parameterization, or interface
  decision. Useful and motivated, but not a biological measurement.
- Claims of class C (unsupported) are not made anywhere in this
  specification; where a tempting claim is unsupported, it is named as
  unsupported and not made.

The distinction is enforced because the two published connectome-based games
audited in the research ledger are cautionary examples: DoomFly's plasticity
is operational but failed its own conditioning and survival controls, and
Aimbug's aiming depends on engineered sight gates and tuned couplings rather
than discovered fly circuitry. Drosa adopts the division of responsibility
those audits support (structured recurrent state, sparse associations,
compartment-specific adaptation, separate fast control) and avoids their
overclaims.

### 1.3 System organization

The fly brain is not a serial feedforward stack. It is a parallel
architecture with downstream convergence and recurrent feedback: fast reflex
paths, a state-maintaining navigation circuit, and a valuation circuit all
run concurrently and converge on a shared motor bottleneck.

```text
                            +---> Fast direct reflexes (giant fiber system)
                            |     loom features -> ballistic escape
                            |     central delay: a few ms [H7]
                            |
Sensory inputs -------------+---> Central Complex (EB / PB / FB)
50 PN rate channels,              heading bump theta_t, 16 EB wedges
optic flow, haltere rates         P-EN angular velocity integration
                            |     home vector, goal steering
                            |          ^
                            |          | goal weight modulation
                            |          | (one multiply-add)
                            +---> Mushroom Body          |
                                  50 PNs -> 2,048 KCs    |
                                  ~5% active (APL rule)  |
                                  KC -> 21 MBON rows     |
                                  dopamine-gated         |
                                  heterosynaptic         |
                                  depression             |
                                      |            |
                                      |            +--> direct MBON -> DN
                                      |                 valence bias
                                      +--> MBON -> FB / LAL goal input
                            |
                            v
                  Descending bottleneck (~1,300 DNs [H3]-[H5])
                  arbitration onto scalar commands v, omega
                            |
                            v
                  Thoracic CPGs: tripod gait, wing-beat oscillator
```

Three clock domains, matching the biological timescale hierarchy:

| Domain | Rate | Contents |
|---|---|---|
| Motor reflex | 1 to 5 kHz | wing-beat modulation, tripod leg oscillation, haltere trim, giant fiber reflex latch |
| Spatial guidance and policy | 100 to 500 Hz | KC expansion, MBON readout, compass bump shift, vector integration, steering |
| Neuromodulatory learning | 1 to 10 Hz | dopamine compartment events, eligibility trace decay, in-place W_out update |

Frozen versus plastic partition: the sensory expansion, Central Complex
geometry, and CPG waveforms are frozen at development (class B, pre-trained
in simulation then baked into ROM); the KC to MBON readout is the only
runtime-plastic structure. This partition is the central engineering bet and
is exactly where the fly evidence is strongest: the fly's olfactory learning
is expressed through readout valence, not through re-wiring its expansion or
its compass.

---

## 2. Sensory expansion: projection neurons to Kenyon cells

### 2.1 Biological substrate (A)

The adult Mushroom Body receives roughly 50 classes of olfactory projection
neurons, one per glomerulus. Projection neurons diverge onto approximately
2,000 Kenyon cells per hemisphere; each KC samples on average 6 glutamatergic
claws from that pool. KC firing is sparse: the giant GABAergic anterior
paired lateral (APL) interneuron provides global feedback inhibition that
holds the active fraction near 5%. KCs project to 21 classes of mushroom
body output neurons (MBONs) in anatomically segregated compartments, each
compartment innervated by a distinct dopaminergic cell class (Aso et al.
2014 [H12]; Li et al. 2020 [P1]).

A KC claw is present or absent; there is no evidence that graded W_in
efficacy is required for associative conditioning. The adult MB also
contains feedback (MBON to DAN), cross-compartment interactions, and
multiple memory timescales [P1, P3]; Drosa models none of these in the
expansion stage.

### 2.2 Computational model (B)

The hardware parameter set (section 7) instantiates the expansion as a fixed
sparse random projection followed by global competition:

$$z_j = \operatorname{topk}_{5\%}\!\left( \mathbf{1}\!\left[ \sum_{i \in \mathcal{I}_j} x_i \;>\; \theta_j \right] \right), \qquad |\mathcal{I}_j| = 6, \quad j = 1 \dots 2{,}048$$

where $x_i$ are baseline-centered INT8 projection-neuron rate codes, the
index sets $\mathcal{I}_j$ are drawn once at development from the 50 input
channels and persisted (seeded, content-hashed), $\theta_j$ are per-KC INT8
thresholds, and the top-k stage keeps the 102 largest supra-threshold
responses, approximating APL-mediated global feedback inhibition. KC outputs
are binary: a cell either responds in this tick or does not.

### 2.3 Pattern separation analysis (B, engineering derivation)

The expansion functions as a binary locality-sensitive hash. For two
unrelated odors whose active-KC codes are independent 5% Bernoulli codes,
the expected code overlap is

$$K \rho^2 = 2048 \times 0.05^2 \approx 5.1 \text{ cells} \quad \text{vs} \quad K\rho \approx 102 \text{ cells for identical odors},$$

a 20:1 separation between "same" and "unrelated". For binary input vectors
whose per-channel agreement probability is $s$, a KC's six sampled channels
all agree with probability $s^6$ (the dominant term when odors are sparse,
since differing channels rarely rescue an equal six-way sum). The expected
code agreement rate therefore follows a threshold function of input
similarity:

| per-channel agreement $s$ | $s^6$ |
|---:|---:|
| 1.00 | 1.000 |
| 0.90 | 0.531 |
| 0.70 | 0.118 |
| 0.50 | 0.016 |

Nearby inputs map to overlapping codes; distant inputs map to nearly
disjoint codes. With ~100 active cells out of 2,048 and pairwise overlaps of
this magnitude, arbitrary pairs of trained associations are linearly
separable by a single linear readout, which is the property the plastic
stage (section 4) depends on. The exact overlap distribution depends on the
threshold and rank-select nonlinearity; the Phase 1 gate G1.3 verifies
activation agreement against a float oracle to >= 99% under noisy inputs
rather than relying on this analytic sketch alone.

### 2.4 Interfaces and storage

| Object | Format | Size |
|---|---|---:|
| W_in connectivity | CSR, 6 x u8 input indices per KC | 12,288 B (frozen ROM) |
| KC thresholds, gains | u8 each | 4,096 B (frozen ROM) |
| Input rate registers | 50 (biological anchor) to 64 (hardware register file) x i16 | 128 B |
| KC activation bitmap | 2,048 x 1 bit | 256 B |

The 64-channel hardware register file hosts optic-flow, airflow, haltere,
and internal-state lines alongside the ~50 olfactory channels; the
biological anchor count and the register file width differ by design and are
both stated to prevent silent conflation.

### 2.5 Declared simplifications (B)

1. Random claw wiring replaces the stereotyped but not fully characterized
   KC sampling structure; the random projection is seeded and persisted so
   learned weights keep their meaning across restarts.
2. One hemisphere's expansion is instantiated; bilateral redundancy is a
   robustness feature, not a computational one, at this stage.
3. KC subclasses (gamma, alpha/beta, alpha'/beta') are not distinguished;
   all KCs share one threshold distribution and one plastic compartment
   routing family.
4. APL inhibition is approximated by a deterministic global top-5% rank
   select. The audit ledger flags that a cap on active units is not a
   requirement to activate units when all input is absent.
5. Graded PN rates are INT8; KC outputs are binarized. The audit identifies
   this as the strongest projection in the stage and provides a fallback:
   keeping INT8 rate sums with binary KC outputs (the recommended operating
   point in the hardware mapping) costs no multipliers and preserves more
   odor separability on hard odor sets.

---

## 3. Central Complex ring attractor

### 3.1 Biological substrate (A)

The Central Complex comprises structured neuropils: the ellipsoid body (EB,
16 wedges), the protocerebral bridge (PB, 18 glomeruli), the fan-shaped body
(FB, 8 columns), and the noduli. A population bump of E-PG compass neurons
across the EB/PB ring encodes angular heading relative to an internal
reference (not geographic north). The bump is anchored by visual landmarks
and, in darkness, is rotated by self-motion: P-EN circuitry combines heading
activity with angular-velocity signals through shifted recurrent loops
(Seelig and Jayaraman 2015 [C1]; Turner-Evans et al. 2017, 2020 [C2, C3]).
Darkness persistence is real but imperfect: darkness trials include
accumulating heading error [C3]. FB pathways transform body-centered
movement into world-centered traveling-direction signals via vector-like
operations (Lyu et al. 2022 [C5]). FC2 activity represents a goal angle and
PFL3 neurons combine goal and heading to produce left/right steering
(Mussells Pires et al. 2024 [C6]; Westeinde et al. 2024 [C7]).

"Local excitation plus global inhibition" is a reduced motif for the bump
circuit, not a complete wiring list; recurrent support (P-EG, PEN2, Delta7)
shapes the bump in ways this specification abstracts into the attractor
form below.

### 3.2 Attractor mathematics (A for the biology, B for the reduction)

A rotationally symmetric neural field over preferred heading $\varphi$:

$$\tau\, \partial_t r(\varphi) = -r(\varphi) + \phi\!\left[ \int W(\varphi - \psi)\, r(\psi)\, d\psi + I_0 \right]$$

If a localized stationary bump $r_0$ exists and perturbations that change
bump shape or amplitude decay, then by rotational symmetry every shifted
bump $r_\theta(\varphi) = r_0(\varphi - \theta)$ is also stationary, and
differentiating $F(r_\theta) = 0$ along the shift direction gives a neutral
tangent mode: $DF(r_\theta)\, \partial_\theta r_\theta = 0$. The bump's
angular position is an undamped coordinate. Stable transverse modes restore
bump shape; the phase mode remembers where the animal is pointing. This is
the mathematical statement of "persistent heading in darkness", conditional
on the symmetry assumptions, and real finite noisy networks deviate: they
drift, and [C3] measures that drift.

The reduced normal form makes the same point in two state variables. With
$z = (c, s)^\top$ and $J = \begin{bmatrix} 0 & -1 \\ 1 & 0 \end{bmatrix}$:

$$\dot z = \kappa (1 - \lVert z \rVert^2)\, z + \omega J z
\quad\Longrightarrow\quad
\dot\rho = \kappa (1 - \rho^2)\rho, \quad \dot\theta = \omega$$

The unit circle is radially attracting (radial derivative $-2\kappa$ at
$\rho = 1$); with $\omega = 0$ every angle persists; with angular-velocity
drive, $\theta(t) = \theta(0) + \int_0^t \omega(s)\, ds$. The origin is an
unstable equilibrium and must never be used as an initialized heading.

Numerical discipline matters here: exact rotation
$z_{t+1} = R(\omega_t \Delta t)\, z_t$ preserves the norm in exact
arithmetic, while naive Euler rotation $z_{t+1} = (I + \Delta t\, \omega J)\, z_t$
inflates the norm by $\sqrt{1 + (\Delta t\, \omega)^2}$ every step. A
careless integrator manufactures runaway neural activity out of a
norm-preserving system.

### 3.3 Engineering realization (B)

Drosa implements heading in two forms, one primary and one reference:

1. **Primary: INT32 phase accumulator.** $\theta_{t+1} = \theta_t +
   \omega_q \Delta t$ in a 32-bit phase word, with $(\cos\theta, \sin\theta)$
   produced by CORDIC or quarter-wave LUT on demand. The state store adds
   zero rounding error; all drift is the once-quantized angular-velocity
   input, a sensor-calibration property rather than a storage walk. This is
   the cheaper form: 4 to 6 DSP slices versus 12 to 16.
2. **Reference: recurrent bump network.** A 16-wedge discrete attractor
   (local excitation, global inhibition, shifted P-EN-like velocity input)
   is maintained in the Phase 1 simulator as the biologically literal
   comparison condition, and earns RTL only if it demonstrates a functional
   advantage over the accumulator.

Drift arithmetic fixes the precision floor. Storing the heading pair in
Q1.15, each exact-rotation step's four products round to a grid of
$q = 2^{-15} \approx 3.05 \times 10^{-5}$. Treating per-step phase error as
a zero-mean random walk of step size ~$2q$, over $N = 3 \times 10^5$ steps
(10 minutes of darkness at 500 Hz):

$$\sigma \approx \sqrt{N}\, \cdot 2q \approx 548 \times 6.1 \times 10^{-5} \approx 3.3 \times 10^{-2} \text{ rad} \approx 1.9^\circ$$

The same walk in Q1.7 state ($q$ larger by $2^8$) reaches ~4.2 radians: not
a compass. Hence INT16 is the minimum for stored trigonometric state, INT8
is disqualified for heading storage (while remaining fine for readout
slices), and the INT32 accumulator removes the storage term entirely. Gate
G1.1 requires <= 5 degrees over the 10-minute darkness run for the INT16
pipeline and zero storage-induced drift for the accumulator, both
cross-checked against a float64 oracle.

### 3.4 Vector path integration and steering (B, on audited biology A)

Body-frame velocity $v_b$ rotates into the world frame and integrates
position:

$$v_w = R(\theta)\, v_b, \qquad p_{t+1} = p_t + \Delta t\, v_w, \qquad h = p_{\text{home}} - p$$

The home vector $h$ and the goal angle $g = \operatorname{atan2}(h_y, h_x)$
feed the steering law, which subtracts vectors on the circle without an
angle discontinuity:

$$u_{\text{turn}} = K \sin(g - \theta) = K\, [\cos\theta \sin g - \sin\theta \cos g]$$

This is a compact engineering approximation of the PFL3-class goal-minus-
heading computation [C6], not a verbatim fit of every PFL3 response. Odometry
and heading drift accumulate without external correction; landmark
observations re-anchor the bump with a gated correction
$\theta \leftarrow \theta + k_{\text{vis}} \sin(\theta_{\text{vis}} - \theta)$,
and "no visual input" means skip the correction, never zero the heading
state.

Goal sources are pluggable: an innate visual-landmark goal, or a learned
goal rotated by Mushroom Body valence. From the steering circuit's
perspective a learned-attraction input is interchangeable with an innate one
[H2], which is precisely the abstraction Drosa implements: MBON output
enters as a goal-weight modulation, one multiply-add, never inside the
reflex or motor loops.

### 3.5 Optional context state, and a naming disambiguation (B)

A small Gated DeltaNet-2 matrix state (4 heads x 16 x 16 INT16, 2,048 B) may
sit beside the explicit heading state to carry non-geometric context under
partial observation. Two rules from the audit attach to it:

1. Naming: "GDN" in Gated DeltaNet-2 is unrelated to the giant fiber
   descending neuron of section 5.2. Fleet documents write "GDN-2" only for
   the linear-attention module.
2. Mathematics: a biological ring attractor is not a linear GDN-2
   recurrence, and ordinary learned decay does not guarantee landmark-free
   retention (unwritten state with decay gate $\alpha < 1$ decays; exact
   retention needs an explicit hold mode). Drosa keeps the explicit heading
   estimator regardless; the learned context state is an adjunct, never the
   compass.

---

## 4. Mushroom Body: three-factor plasticity without backpropagation

### 4.1 Biological substrate (A)

Kenyon cells converge onto 21 MBON output classes in compartments, each
innervated by distinct dopaminergic populations: PPL1 classes on the
aversive side and PAM classes on the appetitive side (Aso et al. 2014
[H12]). The key experiment for Drosa's learning rule is Hige et al., Neuron
2015 [P2]: pairing odor with PPL1 activation depresses KC to MBON synaptic
efficacy, and the depression survives complete suppression of postsynaptic
MBON action potentials (QX-314 in the recording pipette). Dopamine gates
KC to MBON plasticity; KC and dopamine coincidence drives
compartment-specific depression; a mandatory simultaneous MBON spiking
factor is ruled out for this synapse class.

Dopamine is not one scalar reward signal. Different dopaminergic populations
carry punishment, reward, sensory, movement, and internal-state signals
depending on compartment and task; changes above and below baseline both
matter; MBON to DAN feedback links shorter- and longer-lasting memories
[P3]; distinct receptor pathways (DopR1 via cAMP, DopR2 via IP3) encode
temporal order [P5]. Drosa compresses this richness into compartment-routed
signed teacher signals, and the compression is declared as engineering.

### 4.2 The learning rule

Each KC carries one INT16 eligibility trace; each compartment carries one
teacher trace. On each policy tick (period $\Delta t$, trace time constant
$\tau_e$):

$$e_{j,t} = \lambda_t\, e_{j,t-1} + x_{j,t}, \qquad \lambda_t = e^{-\Delta t / \tau_e}$$

On a teaching event in compartment $c$ with baseline-centered dopaminergic
drive $d_c - d_{0,c}$:

$$\Delta W_{i,j} = -\eta\, (d_{c(i)} - d_{0,c})\, e_j, \qquad W \leftarrow \Pi_{\mathcal{W}}\!\left[ W + \Delta W \right]$$

$\Pi_{\mathcal{W}}$ is a box projection clamping weights to a fixed
interval, so bounded teachers cannot drive runaway readout growth. The
punishment side depresses synapses onto compartments whose MBONs promote
approach; the appetitive side (PAM-analog compartments) potentiates approach
rows, or equivalently depresses avoidance rows, through the same routing
table. Learning events are bounded at ~10 Hz.

Trace-decay arithmetic, for $\Delta t = 5$ ms and $\tau_e = 1$ s:
$\lambda = e^{-0.005} \approx 0.9950125$; an isolated tag retains 36.8%
after 1 s and 0.674% after 5 s. These are properties of the chosen
exponential, not measured fly constants; eligibility on a seconds scale and
retained synaptic memory on a minutes scale are distinct variables, and
$\tau_e$ is a tunable, not a law.

### 4.3 Why the eligibility storage factorizes: 4 KB, not 84 KB

Under the biologically supported rule the presynaptic factor $x_j$ is a
property of the KC and the teaching factor is a property of the compartment.
The eligibility of synapse $(i, j)$ therefore factorizes:

$$\operatorname{elig}(i, j) = \operatorname{trace}(x_j) \cdot \operatorname{route}(i)$$

and the storage is 2,048 per-KC INT16 traces (4,096 B) plus tens of bytes
of per-compartment teacher state. A full 21 x 2,048 eligibility matrix (84
KB) would correspond to a postsynaptically gated rule variant that the key
fly experiment rules out for this synapse class [P2]. The earlier Drosa
hardware draft allocated that 84 KB; the audited ledger corrects it, and
the optional policy-gradient actor head (section 4.5), which genuinely
needs per-action traces, costs 8 x 2,048 x INT16 = 32 KB, not 84 KB.

### 4.4 Relation to policy gradients: why this is not backpropagation

For a single linear readout $y_i = \sum_j W_{ij} z_j$ under reward $R$, the
REINFORCE update is
$\Delta W_{ij} = \eta\, R\, (y_i - \bar y)\, x_j$ and the three-factor
Hebbian form is $\Delta W_{ij} = \eta\, R\, y_i\, x_j$: in one layer the
score derivative $\partial y_i / \partial W_{ij}$ is literally the input
activation $x_j$, so computing a gradient requires no backward pass.

What backpropagation would demand, and Drosa never pays for:

1. No activation caching. BPTT stores the activation history of the whole
   network to replay backward. Drosa's per-synapse memory is one trace
   register; $|e_t| \le \lambda^t |e_0| + F/(1-\lambda)$ for bounded
   increments, so memory is O(1) in episode length.
2. No weight transport. Updating an early layer through a later one
   requires transposed global weight products. Layer 1 ($W_{in}$) is frozen
   at development, so no backward chain through it exists or is needed.
3. No batching, no replay buffers, no optimizer state. One teaching event
   updates in place, forward in time.

The honest limits, carried from the audit: eligibility fades, so delayed
credit can vanish; a scalar teacher does not distinguish which feature
caused an outcome; a local rule is not the exact gradient of any recurrent
long-horizon objective; and plasticity cannot repair a representation that
never separated the cues in the first place. The conditioning gates in
section 4.6 exist to test that the mechanism earns its keep.

### 4.5 Optional actor head (B)

An optional extension reuses the MBON rows as a small policy head over 8
bounded motion options, replacing the teacher with a TD signal
$\delta_t = r_t + \gamma V(z_{t+1}) - V(z_t)$ and the eligibility with the
score trace $E^\pi_{a,j,t} = \gamma \lambda_t E^\pi_{a,j,t-1} +
z_{j,t}\,[\mathbf{1}(a = a_t) - \pi_{a,t}]$. This is the one component
whose traces do not factorize (hence the 32 KB), it is marked optional, and
the main scientific baseline uses a single shared TD signal for every action
row rather than inventing per-channel teachers that optimize no common
objective.

### 4.6 Learning behavior gates

The Phase 1 conditioning battery is a two-arm choice assay on synthetic
odor vectors with Performance Index
$\mathrm{PI} = (\text{avoid} - \text{approach}) / (\text{approach} + \text{avoid})$:

| Gate | Requirement |
|---|---|
| One-shot | PI >= 0.5 after a single reinforced pairing |
| Multi-shot | PI >= 0.7 within six pairings |
| Retention | < 10% PI loss over 60 minutes of simulated time |
| Extinction | cue-driven extinction within ~10 unreinforced presentations; no spontaneous loss |
| Controls | unpaired, backward-paired, frozen-weight, shuffled-teacher, and wrong-compartment conditions must all fail to learn |

These thresholds sit inside the range classically reported for fly
conditioning; before treating them as replication, Phase 1 pins the
comparison to specific experimental papers. The "1-to-5 shot" headline claim
is bound to these gates, not to a demonstration.

### 4.7 Quantization-aware learning (summary)

The readout stores an INT16 Q8.8 master per synapse and serves inference
from its upper byte. An update smaller than one inference LSB accumulates
in the master instead of rounding to zero forever; with per-event master
updates of 32 to 64 LSB, the inference byte moves every 4 to 8 reinforced
events per synapse, while ~102 co-active KCs shift the MBON sum measurably
on the first reinforced event. The design rule is: choose $\eta$ against
the master LSB, never the inference LSB. Full arithmetic in the hardware
mapping document.

---

## 5. Motor convergence: the descending bottleneck and the CPGs

### 5.1 Biological substrate (A)

The neck connective, the bottleneck between brain and ventral nerve cord,
carries approximately 3,100 parallel axons: about 1,300 descending neurons
(1,314 counted in the male CNS) and about 1,800 ascending neurons [H3, H4,
H6]. The brain holds ~130,000 to 140,000 neurons, the VNC ~22,000 [H5]:
roughly 100:1 compression from brain-wide computation onto the descending
bus. MBONs reach motor output through two routes: direct synapses onto
descending neurons [P1], and projections into the fan-shaped body and
lateral accessory lobe [P1, H1], where GABAergic and cholinergic MBONs
converge on the same target neurons, a motif read as rapid updating of
sensory processing before behavior [H1].

### 5.2 The giant fiber reflex (A, with a refined latency claim)

Looming visual expansion drives escape through the giant fiber (GF)
descending neuron pair. GF spike timing is set by summation of two loom
features, angular size and angular velocity, with velocity input from LC4
lobula columnar cells [H9]; a single GF spike selects short versus long
takeoff [H9]; the GF drives synchronized leg extension and wing depression
regardless of stimulus azimuth [H8]. Central escape-circuit latencies run
"a few milliseconds" [H7]; the interval from loom onset to jump additionally
includes visual feature integration and is longer. Drosa's 10 ms silicon
reflex budget is therefore conservative: the fabric path (loom feature
registers, threshold comparator, command latch) is 2 to 4 clock cycles at
100 MHz, roughly 20 to 40 ns, and end-to-end reflex time is bounded by
sensor cadence, not logic depth.

### 5.3 Engineering realization (B)

All brain-side output converges onto a small command interface, mirroring
the biological bottleneck:

- Arbitration priority: giant fiber reflex override, then CX steering
  ($u_{\text{turn}}$, forward vigor), then direct MBON valence bias.
- The command surface is two scalars, forward velocity $v$ and turn rate
  $\omega$, plus discrete gait and mode latches: the same ~100:1
  compression the neck connective implements.
- Thoracic CPGs convert $v$ and $\omega$ into rhythmic waveforms from
  phase-indexed INT8 ROM tables: tripod walking gait at leg-loop rate,
  wing-beat amplitude and frequency modulation around 200 Hz. The motor
  loop runs at 1 kHz (phase increment, gait ROM read, servo update), with
  the policy loop at 100 to 500 Hz feeding it.

---

## 6. Parameter set of record

| Parameter | Value | Basis |
|---|---:|---|
| Sensory input channels | 50 biological anchor / 64 hardware register file | A (PN classes) / B (register width) |
| Kenyon cells K | 2,048 | B, from ~2,000 per hemisphere (A) |
| Claws per KC | 6 | A |
| Active fraction rho | 5% (~102 cells) | A |
| MBON compartments M | 21 | A [H12] |
| EB wedges / PB glomeruli / FB columns | 16 / 18 / 8 | A [C4] |
| Descending neurons | ~1,300 (1,314 counted) | A [H3, H4] |
| Policy tick | 100 to 500 Hz | B |
| Motor tick | 1 to 5 kHz | B |
| Teaching event rate | <= 10 Hz | B |
| Eligibility time constant tau_e | 1 s class (tunable) | B |
| Learning rate eta | set against INT16 master LSB | B, hardware mapping |

---

## 7. Evidence ledger

| Claim in this specification | Class | Sources |
|---|---|---|
| ~50 PN classes, ~2,000 KCs/hemisphere, ~6 claws, APL sparsity near 5%, 21 MBON + 20 DAN types | A | [H12], [P1] |
| KC claw topology is presence/absence; graded W_in efficacy not required for conditioning | A (absence of evidence, stated as such) | [P1] |
| EB 16 wedges, PB 18 glomeruli, FB 8 columns | A | [C4] |
| E-PG heading bump; P-EN angular velocity integration; darkness persistence with accumulating error | A | [C1], [C2], [C3] |
| FB vector computation; FC2 goal; PFL3 goal-minus-heading steering | A | [C5], [C6], [C7] |
| Learned valence interchangeable with innate attraction at the steering input | A | [H2] |
| Dopamine-gated heterosynaptic depression without postsynaptic spiking | A | [P2] |
| Compartment-specific dopaminergic organization, PPL1/PAM | A | [H12], [P1] |
| MBON to FSB/LAL and MBON to DN direct routes | A | [P1], [H1] |
| Neck connective ~3,100 axons, ~1,300 DNs, ~1,800 ANs | A | [H3], [H4], [H6] |
| GF loom size+velocity summation, LC4 input, single-spike takeoff, azimuth invariance, few-ms central latency | A | [H7], [H8], [H9] |
| Random projection as expansion model; top-k as APL proxy | B | this document |
| INT32 accumulator as primary heading; bump as reference condition | B | audit appendix |
| Eligibility factorization into per-KC traces | B, derived from A rule of [P2] | this document |
| PI gate thresholds as engineering targets inside classical ranges | B, pending pinned paper comparison | audit appendix |
| Ring attractor is / is not a linear SSM | the exact statement: recurrent state-space abstractions are appropriate; exact linear or GDN equivalence is unestablished | audit section 7.4 |

---

## 8. What this specification does not claim

1. No claim of whole-fly emulation or of biological fidelity beyond the
   labeled evidence.
2. No claim that sub-millisecond spike-timing codes are preserved; rate
   coding preserves the dynamics of the functions implemented (valence
   integration over ~1 s windows, heading integration, CPG phase), which is
   the scope.
3. No claim that the local learning rule solves long-horizon credit
   assignment; the frozen-trunk, plastic-readout partition is the design
   answer to that limit, and the conditioning gates test it.
4. No neuron-for-neuron correspondence between Drosa blocks and GDN-2
   keys, values, or gates.
5. No performance claims from the audited games; both are treated as
   cautionary evidence, not validation.

---

## 9. References

Connectome datasets and validated models:

- [F1] Dorkenwald et al. (2024), *Neuronal wiring diagram of an adult brain*,
  Nature 634, 124-138.
  https://doi.org/10.1038/s41586-024-07558-y
- [F2] Schlegel et al. (2024), *Whole-brain annotation and multi-connectome
  cell typing of Drosophila*, Nature 634, 139-152.
  https://doi.org/10.1038/s41586-024-07686-5
- [F3] Eckstein et al. (2024), *Neurotransmitter classification from
  electron microscopy images at synaptic sites in Drosophila melanogaster*,
  Cell 187, 2574-2594.
  https://doi.org/10.1016/j.cell.2024.03.016
- [F4] Shiu et al. (2024), *A Drosophila computational brain model reveals
  sensorimotor processing*, Nature 634, 210-219.
  https://doi.org/10.1038/s41586-024-07763-9
- [F5] Lappalainen et al. (2024), *Connectome-constrained networks predict
  neural activity across the fly visual system*, Nature 634, 1132-1140.
  https://doi.org/10.1038/s41586-024-07939-3
- [M1] Berg et al. (2026), *Sexual dimorphism in the complete Drosophila
  male central nervous system connectome*, Cell 189, 5504-5526.
  https://doi.org/10.1016/j.cell.2026.08.015
- [M2] MaleCNS official project and release pages,
  https://male-cns.janelia.org/

Central Complex, navigation, and steering:

- [C1] Seelig and Jayaraman (2015), *Neural dynamics for landmark
  orientation and angular path integration*.
  https://doi.org/10.1038/nature14446
- [C2] Turner-Evans et al. (2017), *Angular velocity integration in a fly
  heading circuit*. https://elifesciences.org/articles/23496
- [C3] Turner-Evans et al. (2020), *The neuroanatomical ultrastructure and
  function of a biological ring attractor*.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC8356802/
- [C4] Hulse et al. (2021), *A connectome of the Drosophila central complex
  reveals network motifs suitable for flexible navigation*.
  https://elifesciences.org/articles/66039
- [C5] Lyu, Abbott, and Maimon (2022), *Building an allocentric travelling
  direction signal via vector computation*.
  https://doi.org/10.1038/s41586-021-04067-0
- [C6] Mussells Pires et al. (2024), *Converting an allocentric goal into
  an egocentric steering signal*. https://doi.org/10.1038/s41586-023-07006-3
- [C7] Westeinde et al. (2024), *Transforming a head direction signal into
  a goal-oriented steering command*.
  https://doi.org/10.1038/s41586-024-07039-2

Mushroom Body and plasticity:

- [P1] Li et al. (2020), *The connectome of the adult Drosophila mushroom
  body provides insights into function*. https://elifesciences.org/articles/62576
- [P2] Hige et al. (2015), *Heterosynaptic plasticity underlies aversive
  olfactory learning in Drosophila*, Neuron 88(5), 985-998.
  https://doi.org/10.1016/j.neuron.2015.11.003
- [P3] Huang, Luo et al. (2024), *Dopamine-mediated interactions between
  short- and long-term memory dynamics*.
  https://doi.org/10.1038/s41586-024-07819-w
- [P4] Gerstner et al. (2018), *Eligibility traces and plasticity on
  behavioral time scales*. https://doi.org/10.3389/fncir.2018.00053
- [P5] Handler et al. (2019), *Distinct dopamine receptor pathways
  underlie the temporal sensitivity of associative learning*.
  https://doi.org/10.1016/j.cell.2019.05.040
- [P6] Bellec et al. (2020), *A solution to the learning dilemma for
  recurrent networks of spiking neurons*.
  https://doi.org/10.1038/s41467-020-17236-y

Descending control and hardware:

- [H1] Scaplen et al. (2021), *Transsynaptic mapping of Drosophila mushroom
  body output neurons*. https://elifesciences.org/articles/63379
- [H2] *How does the insect central complex use mushroom body output for
  steering?*, Current Biology (2018).
  https://www.sciencedirect.com/science/article/pii/S0960982218306961
- [H3] *Distributed control circuits across a brain-and-cord connectome*,
  Nature (2026). https://www.nature.com/articles/s41586-026-10735-w
- [H4] *The Drosophila connectome reveals axo-axonic synapses on descending
  neurons*. https://pubmed.ncbi.nlm.nih.gov/42063566/
- [H5] *Descending control of motor sequences in Drosophila*.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC11215313/
- [H6] *Comparative connectomics of Drosophila descending and ascending
  neurons*, Nature (2025). https://www.nature.com/articles/s41586-025-08925-z
- [H7] *A Computational Model of the Escape Response Latency in the Giant
  Fiber System of Drosophila melanogaster*, eNeuro (2019).
  https://www.eneuro.org/content/6/2/ENEURO.0423-18.2019
- [H8] *Azimuthal invariance to looming stimuli in the Drosophila giant
  fiber escape circuit*, J Exp Biol 226(8) (2023).
  https://journals.biologists.com/jeb/article/226/8/jeb244790
- [H9] *Neural Basis for Looming Size and Velocity Encoding in the
  Drosophila Giant Fiber Escape Pathway*, Current Biology (2019).
  https://www.sciencedirect.com/science/article/pii/S0960982219301381
- [H10] Kria K26 SOM data sheet (117,120 LUTs, 144 BRAM36, 64 UltraRAM,
  1,248 DSPs). https://www.amd.com/en/products/system-on-modules/kria/k26.html
- [H11] AMD DS181 Artix-7 data sheet and Avnet Artix-7 product table.
- [H12] Aso et al. (2014), *The neuronal architecture of the mushroom body
  provides a logic for associative learning*, eLife 3:e04577.
  https://elifesciences.org/articles/04577

Internal engineering references (read-only):

- Hardware architecture and execution plan:
  `~/main/docs/design/2026-09-16-drosophila-hardware-architecture-and-execution-plan.md`
- Connectome and learning audit with hardware verification appendix:
  `~/main/docs/research/2026-09-16-fruit-fly-connectome-and-biological-learning-audit.md`
- Naming whitespace audit:
  `~/main/docs/research/2026-09-17-drosophila-naming-whitespace-audit-glm.md`
- Open-source and visualization vision:
  `~/main/docs/vision/2026-09-16-drosophila-open-source-interactive-viz-and-profile.md`
