# Drosa

Drosa is an open-source neuromorphic physical AI architecture that maps the
sensory, navigation, and learning circuits of the fruit fly connectome onto
digital silicon. An adult *Drosophila melanogaster* executes agile 3D
aerobatics, spatial navigation, vector path integration, and one-shot
associative learning with roughly 100,000 to 140,000 neurons on under one
milliwatt of metabolic power. Drosa asks what the smallest faithful digital
rendering of that budget looks like, and answers with a complete controller
that runs in 124 to 165 KB of on-chip FPGA block RAM (inside a 181 KB
conservative envelope), with no floating-point arithmetic, no
backpropagation, and no external DRAM anywhere in the control path.

The name and the numerical claims are audited. Every quantitative statement
in this repository traces to the research ledger in `~/main` (see
[Scientific provenance](#scientific-provenance)), where connectome
provenance, learning-rule evidence, and hardware arithmetic were checked
against primary literature and vendor datasheets.

## Etymology

Drosa is the Romance feminine shape of the Ancient Greek noun *drosos*
(δρόσος), meaning dew. The genus name *Drosophila* compounds *drosos* with
*philos* (φίλος, lover): the dew-lover, from the flies' attraction to moist,
ripening fruit. The name preserves both the taxonomic root and the
grammatical gender of the source noun. It was selected by naming whitespace
audit on 2026-09-17: `drosa` was unclaimed on crates.io, PyPI, and npm, and
showed no DNS resolution across `drosa.ai`, `drosa.io`, `drosa.org`,
`drosa.one`, and `drosa.dev`, with no exact-name occupant in the AI,
robotics, or electronics sectors.

## Thesis

Evolution pre-computed most of what a flying insect needs. Perception
filters, the compass and path-integration circuits, and the thoracic motor
pattern generators are fixed in the adult animal; what remains plastic is a
single sparse associative readout that revalues sensory cues in one to five
reinforced trials. Drosa separates the architecture along exactly that line:

1. **The frozen backbone.** Sensory expansion, Central Complex heading and
   vector integration, and motor pattern generators are pre-trained once in
   physics simulation under heavy domain randomization, then frozen into
   ROM images and fixed logic.
2. **The plastic readout.** The Mushroom Body Kenyon cell to output neuron
   synapses form one wide, sparse linear layer. It learns online, in place,
   using a biologically measured dopamine-gated heterosynaptic depression
   rule. No backward pass ever runs: the first layer is frozen, so there is
   nothing to backpropagate through, and in a single linear layer the
   gradient with respect to a weight is just the input activation.
3. **Integer silicon.** Every operation in the control loop reduces to
   popcount, compare, gated integer addition, fixed-point multiply-accumulate
   on a handful of DSP slices, shift, or small table lookup. No FPU, no
   soft-float, no DDR controller. Single-cycle on-chip access latency is
   treated as a first-class design property, not a capacity fallback.

## Architecture at a glance

```text
                            +--> Fast direct reflexes (giant fiber escape)
                            |    loom size + velocity -> ballistic maneuver
                            |    fabric path: 2-4 clock cycles, < 10 ms budget
                            |
Sensory inputs ------------+----> Central Complex (compass, path vectors)
(50 PN rate channels,             16-wedge EB ring holds heading theta_t
optic flow, haltere)              P-EN shifts integrate angular velocity
                            |         ^
                            |         |  goal weight modulation
                            |         |
                            +----> Mushroom Body (associative valence)
                                   50 PNs -> 2,048 Kenyon cells (~5% active)
                                   KC -> 21 MBON rows, dopamine-gated
                                   heterosynaptic depression, 1-5 shot
                                       |              |
                                       |              +-> direct valence bias
                                       |                  onto DNs
                                       +-> goal rotation input to steering
                            |
                            v
                  Descending neuron bottleneck (~1,300 DNs)
                  scalar forward velocity v and turn rate omega
                            |
                            v
                  Thoracic CPGs: tripod gait, wing-beat oscillator
                  motor loop at 1 kHz, policy loop at 100-500 Hz
```

Four subsystems, each documented in depth in
[docs/architecture/connectome-specification.md](docs/architecture/connectome-specification.md):

| Subsystem | Biological basis | Silicon realization |
|---|---|---|
| Sensory expansion | ~50 projection-neuron classes diverge into ~2,000 Kenyon cells per hemisphere; APL feedback inhibition holds activity near 5% | Binary sparse random projection (a locality-sensitive hash), LUT adder trees, zero multipliers |
| Central Complex ring attractor | 16-wedge ellipsoid-body compass bump persists in darkness; P-EN neurons integrate angular velocity; fan-shaped body maintains home vector; PFL3 neurons steer toward goals | INT32 phase accumulator (drift-free by construction) with the recurrent bump kept as a simulation reference; exact-rotation fixed-point kernels |
| Three-factor plasticity | Dopamine-gated heterosynaptic depression at KC to MBON synapses, experimentally shown to not require postsynaptic spiking (Hige et al., Neuron 2015) | Per-KC INT16 eligibility traces times compartment teacher signals, gated integer addition into an INT16 Q8.8 master weight |
| Motor convergence | ~1,300 descending neurons funnel all brain output through the neck connective (~100:1 compression) onto thoracic CPGs | Fixed-priority arbitration onto scalar v and omega command registers; phase-accumulator CPG ROM |

## Design principles

- **100% float-free.** No floating-point operation exists in the control
  path. Fixed-point formats are chosen per signal class from error
  arithmetic, not habit: INT8 rates and inference weights, INT16 Q8.8 master
  weights, INT32 heading phase.
- **Zero backpropagation.** Learning is a local three-factor rule: an
  eligibility trace (which inputs were recently active) times a broadcast
  teaching signal (dopamine analog per compartment). Per-synapse memory is
  O(1) in episode length; there is no activation cache, no weight transport,
  no replay buffer, no batching.
- **Zero external DRAM.** The entire model state fits in 30 to 40 BRAM36
  blocks. Worst-case on-chip traffic (43 MB/s at a 500 Hz tick against ~24
  GB/s aggregate BRAM bandwidth, roughly 0.2% utilization) leaves about
  three orders of magnitude of headroom, and removes memory arbitration,
  refresh, and TLB variance from a hard-real-time loop.
- **181 KB envelope, 124 to 165 KB audited.** The line-item ledger is
  maintained in
  [docs/architecture/hardware-mapping.md](docs/architecture/hardware-mapping.md);
  the public headline rounds up to 181 KB so that every configuration,
  including the optional actor head and double-buffer slack, is inside the
  stated number.
- **Rate coding over parallel buses, not spike transport.** Biology spikes
  because unmyelinated axons are leaky salt-water cables that cannot hold a
  graded voltage over centimeters. Silicon wires do not have that problem:
  an 8-bit bus is 8 copper traces moving a full byte per clock edge. Drosa
  keeps the functional rate and activation dynamics and drops the physical
  spike transport, which buys two to three orders of magnitude in effective
  signal bandwidth.
- **Determinism as a feature.** Same inputs, same seed, same outputs, on
  every target: CPU reference, GPU kernels, and FPGA bitstream must agree
  within the published quantization spec, verified bit-exact through the
  export manifest.

## On-chip memory budget (summary)

| Item | Size | Class |
|---|---:|---|
| W_in connectivity (2,048 KCs x 6 x u8, CSR) | 12,288 B | frozen ROM |
| KC thresholds and gains | 4,096 B | frozen ROM |
| W_out master, INT16 Q8.8 (2,048 x 21) | 86,016 B | mutable BRAM |
| KC eligibility traces (2,048 x i16) | 4,096 B | mutable BRAM |
| CX state (heading pair, phase, home vector, bump) | ~200 B | mutable BRAM |
| CPG, front-end, buffers, telemetry, slack | ~17 KB | mixed |
| **Core total** | **~124 KB** | |
| Optional actor/critic head (traces + critic) | +40,960 B | mutable BRAM |
| **Full configuration total** | **~165 KB** | |

Full derivation, part-by-part utilization, and the DDR-free argument:
[docs/architecture/hardware-mapping.md](docs/architecture/hardware-mapping.md).

## Precision contract (summary)

| Signal | Format | Reason |
|---|---|---|
| Sensory channel rates, KC sums | INT8 | biology is graded but narrow; 8 bits carry the code |
| KC activations | binary | makes MBON readout a gated add; zero multipliers |
| W_out inference read | INT8 (upper byte of master) | readout noise sits below behavioral threshold |
| W_out master accumulator | INT16 Q8.8 | updates below one INT8 LSB accumulate instead of rounding to zero forever (the quantization stall) |
| Heading state | INT32 phase accumulator (primary), INT16 Q1.15 pair (reference) | INT8 state drifts ~4 radians per 10 minutes of darkness; INT16 bounds drift to ~2 degrees; the accumulator adds zero storage drift |
| Eligibility traces, teacher traces | INT16 Q8.8 | trace lifetime spans seconds at a 500 Hz tick |
| CPG joint trajectories | INT8 ROM | periodic waveforms indexed by phase |

## Execution roadmap

| Phase | Deliverable | Exit gates |
|---|---|---|
| 1 | Rust CPU fixed-point reference (`no_std`-friendly, zero heap in the control loop) | G1.1 compass drift; G1.2 conditioning battery (PI >= 0.5 after one pairing); G1.3 quantization parity vs float oracle; G1.4 determinism; G1.5 co-simulation |
| 2 | CubeCL/WGPU device kernels + domain-randomized pre-training on RX 6900 XT | G2.1 batched rollouts >= 1,024 agents; G2.2 randomization sweep; G2.3 hash-verified weight export; G2.4 bounded-service discipline |
| 3 | Synthesizable SystemVerilog RTL on AMD Kria K26 / Artix-7 XC7A100T | G3.1 cycle-accurate parity vs golden traces; G3.2 P&R closure at 100 MHz; G3.3 on-board timing; G3.4 72 h soak; G3.5 behavior demo |

## Toolchain and building

The project pins one Rust toolchain in `rust-toolchain.toml` (channel 1.94.0,
with rustfmt, clippy, rust-analyzer, and rust-src). Both supported entry paths
resolve to that identical toolchain.

With Nix:

```sh
nix develop
cargo fmt --check && cargo clippy && cargo test
```

Without Nix (rustup reads the same pin file):

```sh
rustup show
cargo fmt --check && cargo clippy && cargo test
```

The flake uses oxalica/rust-overlay with `fromRustupToolchainFile`, so the
devShell and any future Nix package build consume the same pinned compiler;
raw `pkgs.rustc` is never used. GPU work in Phase 2 runs under the fleet's
bounded `training.slice` service standard with a pre-flight resource
estimate, per the fleet GPU compute boundaries.

## Project layout

```text
.
|-- AGENTS.md                        workspace operating rules for every seat
|-- STATE.md                         live operational state, queue, waits
|-- flake.nix                        pinned toolchain via oxalica/rust-overlay
|-- rust-toolchain.toml              single source of truth for the compiler pin
|-- Cargo.toml                       root crate manifest (0.1.0)
|-- src/                             Rust implementation (Phase 1)
`-- docs/
    |-- architecture/
    |   |-- connectome-specification.md   biological and computational spec
    |   `-- hardware-mapping.md           BRAM ledger, precision bounds, RTL budget
    `-- vision/
        `-- 2d-3d-webgpu-visualizer.md    interactive visualizer architecture
```

## Scientific provenance

Drosa is connectome-inspired, not a connectome emulation. The distinction is
maintained everywhere in the documentation, with every biological claim
labeled by evidence class (measured biology versus engineering choice versus
unsupported). The two reference connectomes anchoring the anatomy:

- FlyWire / FAFB: one adult female brain, 139,255 neurons, ~54.5 million
  chemical synapses (Dorkenwald et al., Nature 2024); 8,453 annotated cell
  types (Schlegel et al., Nature 2024).
- MaleCNS v1.0: one adult male central nervous system including the ventral
  nerve cord, 166,700 neurons, 11,710 types (Berg et al., Cell 2026; v1.0
  released 2026-06-08).

An EM connectome establishes topology, cell identities, and contact counts.
It does not supply dynamics, synaptic efficacies, receptor effects, or a
learning rule; those are filled by physiological experiments (each cited
where used) or declared as engineering choices. The full evidence ledger
lives in
[docs/architecture/connectome-specification.md](docs/architecture/connectome-specification.md).

## Documentation index

| Document | Contents |
|---|---|
| [docs/architecture/connectome-specification.md](docs/architecture/connectome-specification.md) | Sensory expansion, ring attractor dynamics, three-factor plasticity, motor convergence; equations, parameters, evidence classes, references |
| [docs/architecture/hardware-mapping.md](docs/architecture/hardware-mapping.md) | Kria K26 / Artix-7 mapping, BRAM ledger, quantization-stall and drift arithmetic, LUT/DSP inventory, latency and power, verification gates |
| [docs/vision/2d-3d-webgpu-visualizer.md](docs/vision/2d-3d-webgpu-visualizer.md) | Browser visualizer: architecture decision, scene layers, interaction model, performance budget |

## License

MIT OR Apache-2.0. License texts land with the first registry publication
(see the queue in `STATE.md`).
