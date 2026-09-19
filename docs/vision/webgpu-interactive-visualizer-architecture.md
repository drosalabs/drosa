# Drosa 2D/3D WebGPU Interactive Connectome Visualizer: Implementation Architecture and Space Engineering Specification

Date: 2026-09-18
Status: implementation architecture of record for the browser visualizer and
its Hugging Face Space deployment. Extends
`docs/vision/2d-3d-webgpu-visualizer.md`, which remains the decision of record
for where the simulation runs.
Depends on: the Phase 1 Rust fixed-point core (crate `drosa` 0.1.0, live on
crates.io since 2026-09-18), the connectome specification and its evidence
ledger, the WebGPU and WebAssembly browser standards, and Hugging Face Spaces.

---

## 1. Scope and position in the document set

This document specifies how the visualizer described in
`docs/vision/2d-3d-webgpu-visualizer.md` is engineered and deployed: the
rendering stack, the dual-loop timing architecture, the WGSL compute
pipelines that turn engine state into render-ready buffers, the mesh asset
pipeline, the WASM to WebGPU memory bridge, and the deployment blueprint for
the Hugging Face Space `drosalabs/connectome-3d` and its embedding at
`drosa.org`.

The division of authority fixed by the prior document is unchanged and
governs everything below: neural dynamics run only in the Rust core compiled
to WebAssembly; rendering is a pure function of exported state; fixed-point
semantics are never re-implemented in TypeScript or WGSL. The WGSL kernels
specified here are derivation kernels: they consume engine-exported state and
produce render-ready buffers on the GPU. The same kernels are the substrate
for the parity-gated batched swarm stretch goal, which remains admitted only
after bit-exact agreement with the CPU core on the fixed episode battery.

Evidence classes carry over from the connectome specification: A for measured
biology, B for engineering choice. Engineering quantities in this document
are budgets or estimates, each labeled with its basis; none is a measurement
until the corresponding gate in section 8 records it.

## 2. System architecture and rendering stack

### 2.1 Stack selection

Three stacks were assessed against the two hard product constraints:
sub-100 ms first 3D frame on mid-tier mobile and desktop, and low power draw
on battery-constrained devices.

| Criterion | Direct WebGPU, thin TypeScript layer | Three.js | wgpu compiled to Wasm |
|---|---|---|---|
| Framework payload added to the page (estimate, gzip) | none; app code only, ~100 to 150 KB | ~150 to 300 KB after tree shaking, more with WebGPU renderer and loaders | ~400 to 700 KB Wasm plus JS glue |
| First-frame cost | minimal parse and instantiate; pipelines precompiled at init | framework bootstrap cost on top | Wasm instantiate plus wgpu adapter plumbing in JS |
| Per-frame control | full: bind groups and dispatches owned by the app | retained scene graph walk adds JS cost per frame | full on paper, but every call crosses the Wasm to JS boundary into the browser API |
| Distance between app and engine state | shortest: engine state maps directly to buffers | scene graph objects must be synchronized from state, an extra translation layer with allocation churn risk | shortest in language, longest in indirection at runtime |
| Failure modes | app owns camera, math, and controls (small, bounded code) | version churn between renderer backends | two abstraction layers over WebGPU complicates shader debugging and timing |
| Suitability | selected | rejected for this product | rejected for the browser path |

Payload figures are estimates based on published bundle statistics for the
respective libraries and on the size of the `drosa` core (the published 0.1.0
crate package is 13.8 KB compressed; a full Phase 1 core is expected well
under 150 KB compressed, to be measured at gate S2).

**Decision: direct WebGPU with a thin TypeScript render layer.** The
render layer owns camera, input, buffer plumbing, and the render graph, with
a hand-rolled linear algebra set (vectors, matrices, quaternions; a few
hundred lines) instead of a scene graph framework. The WebGL2 fallback and
the Canvas 2D schematic mode follow the prior document's design and share the
same render-layer interface.

### 2.2 Load-time budget

The metric is defined precisely so it can be measured: median cold-cache wall
time from navigation start to the first rendered frame of the 3D circuit
scene, on a declared device class (mid-tier 2020-class Android phone over
wifi at 20 Mbps or better, and a 2020-class laptop on broadband), five runs,
cold HTTP cache per run. Target: under 100 ms median, 200 ms p95.

The first frame is the procedural circuit scene (PN arc, KC band, EB ring, DN
funnel, arena): all of it is generated at init from the population constants
in the crate, so its download cost is zero. Anatomical meshes (section 4)
stream after the first frame.

| Stage | Budget | Basis |
|---|---:|---|
| Fetch critical payload (HTML shell, app JS, wasm core, manifest; <= 200 KB compressed total) | 40 ms | 200 KB at 2.5 MB/s, 20 Mbps wifi floor |
| Parse JS, instantiate Wasm core | 15 ms | estimate, 2020-class mobile CPU |
| GPU adapter, device, pipeline compile (all pipelines created up front, <= 12 of them) | 25 ms | estimate; async adapter request in parallel with fetch |
| Upload procedural geometry + L0 state, first render pass | 10 ms | estimate; procedural buffers are a few hundred KB |
| Reserve | 10 ms | |

Total: 100 ms. Every row is an estimate until gate S2 records the first
measurement on the declared device class; the budget then becomes a
regression bound enforced in CI through a headless load probe.

Power draw measures (all class B):

1. Rendering is aligned to `requestAnimationFrame`; no rendering occurs when
   the document is hidden (the simulation pauses with it; simulated time is
   the reference, so pause and resume are trivially correct).
2. GPU frame budget is 6 ms at 1080p on an integrated GPU (carried from the
   prior document), enforced by adaptive resolution scaling (device pixel
   ratio capped at 2, dynamic scale 0.6 to 1.0).
3. A 30 frames per second cap mode is offered and defaults on when the
   Battery Status API or heuristic thermal pressure suggests a battery
   constrained device.
4. Zero per-frame heap allocation in steady state: snapshot buffers,
   matrices, and instance buffers are pooled at init.
5. The simulation Worker's 1 kHz tick is arithmetic on a few KB of state;
   its expected cost is well under 0.5 ms per tick on the declared laptop
   class (estimate; Phase 1 gate G1.4 bounds the native core and the browser
   parity measurement lands at gate M1).

### 2.3 Dual-loop architecture

The simulation loop and the render loop run on separate threads at unrelated
rates, connected only by a sequence-stamped snapshot stream.

```text
+------------------------------------------------------------------+
| Web Worker: drosa core (Rust -> wasm32 via wasm-bindgen)         |
| fixed-timestep accumulator, deterministic, seeded                |
|   motor tick   1 kHz   CPG phase, gait ROM, reflex latch         |
|   policy tick  500 Hz   expansion, MBON readout, CX, steering    |
|   teaching     <= 10 Hz compartment events, W_out update         |
| writes FrameBlock + arrays into pooled snapshot buffers          |
+---------------------------+--------------------------------------+
                            | postMessage with transferable buffer
                            | pool (baseline), or SharedArrayBuffer
                            | with seqlock (upgrade path, 5.2)
                            v
+------------------------------------------------------------------+
| Main thread: TypeScript render layer, zero dynamics              |
|   take latest complete snapshot (sequence stamp check)           |
|   device.queue.writeBuffer: frame head, KC bitmap, DN vector     |
|   interpolate continuous pairs, hold-last discrete signals       |
|   dispatch WGSL derivation kernels (section 3)                   |
|   render graph: 3D pass, arena inset, oscilloscope overlay       |
+---------------------------+--------------------------------------+
                            |
                            v
                WebGPU, with WebGL2 and Canvas 2D fallbacks
```

Tick ladder basis: the connectome specification fixes the biological
timescale hierarchy as motor reflex at 1 to 5 kHz, spatial guidance and
policy at 100 to 500 Hz, and neuromodulatory learning at 1 to 10 Hz (class
B, from measured biology timescales). The browser default instantiates the
ladder as 1 kHz motor, 500 Hz policy, 10 Hz teaching bound. The prior
document's "500 Hz core step" corresponds to the policy tick; this document
makes the motor tick explicit. The 1 kHz motor tick leaves a 1 ms period
budget; the whole ladder tick (motor increment plus every second policy
tick) is estimated under 0.5 ms mean on the declared laptop class, above.

Decoupling contract:

1. The Worker never blocks on rendering and never reads render state.
2. The render loop consumes only the latest snapshot whose sequence stamp
   is complete; a frame never mixes two simulation ticks (carried from the
   prior document).
3. Interpolation policy: continuous exported pairs (heading, goal) are
   interpolated as unit pairs; discrete signals (KC activation bitmap,
   arbitration latches) use hold-last. The renderer never computes
   client-side trigonometry on a wrapped angle (carried from the prior
   document).
4. Slow motion (0.05x to 1x) and single-tick stepping scale the accumulator
   or advance it by one tick; the model equations and their rates are never
   altered (carried from the prior document's interaction model).

### 2.4 Render graph

Three passes over one command encoder per frame: the 3D pass (instanced
opaque geometry, then translucent overlays: plume fields, synapse lines,
bump glow), the 2D arena inset (top-down agent, trail, plumes, landmarks),
and the oscilloscope overlay (trace ring buffer as line batches). Draw calls
stay under 40 for the full scene (carried from the prior document). All
pipelines are created at init; none is compiled at frame time.

## 3. WGSL compute and simulation derivation pipelines

Three derivation kernels own the per-neuron visualization work that would
otherwise be per-frame JavaScript loops. Each kernel's inputs are
engine-exported values in the crate's fixed-point representations; the only
arithmetic applied in WGSL beyond presentation mapping is a constant scale
(for example Q8.8 to float: divide the raw i16 by 256.0) or an exported unit
pair. The kernels do not implement dynamics.

### 3.1 Central Complex compass: ellipsoid body ring attractor view

Biology anchor (A): the ellipsoid body is a ring of 16 wedges [C4]; a
population bump of E-PG compass neurons encodes heading; P-EN circuitry
rotates the bump by angular velocity; darkness persistence is real but
drifts [C1], [C2], [C3].

Engine inputs (per policy tick): heading unit pair exported by the core from
the heading state (the crate stores heading as `HeadingQ8` with 256 units
per turn and the high-resolution `AngleQ16` with 65,536 units per turn; the
accumulator form of the connectome specification is the primary), goal unit
pair, raw angular velocity (i16 Q8.8), bump amplitude, and the darkness
latch.

Kernel `eb_bump`, one thread per wedge, single workgroup of 16. The bump is
rendered as a von Mises-form profile around the exported heading pair, so
the kernel evaluates alignment by dot product and never constructs a wrapped
angle:

```wgsl
struct CompassState {
  heading_pair: vec2<f32>,
  goal_pair: vec2<f32>,
  omega_q8: i32,
  bump_amplitude: f32,
  bump_kappa: f32,
  darkness: u32,
  reserved: u32,
}

struct WedgeVisual {
  intensity: f32,
  shift_direction: f32,
  selected: f32,
  reserved: f32,
}

@group(0) @binding(0) var<uniform> state: CompassState;
@group(0) @binding(1) var<storage, read_write> wedges: array<WedgeVisual>;

const TAU: f32 = 6.28318530718;

@compute @workgroup_size(16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let index = gid.x;
  let angle = f32(index) * (TAU / 16.0);
  let alignment = cos(angle) * state.heading_pair.x + sin(angle) * state.heading_pair.y;
  let profile = exp(state.bump_kappa * (alignment - 1.0));
  let omega = f32(state.omega_q8) / 256.0;
  wedges[index].intensity = state.bump_amplitude * profile;
  wedges[index].shift_direction = sign(omega) * clamp(abs(omega) * 8.0, 0.0, 1.0);
}
```

`bump_kappa` is a presentation gain (class B): it shapes how sharp the
rendered bump looks, nothing else. Velocity integration is visible three
ways: the wedge intensities rotate as the engine integrates angular
velocity; a trail band between the previous and current heading pair shows
the per-tick shift direction; and the steering needle geometry is derived in
the same pass from the goal-minus-heading form of the connectome
specification, `u = K (goal_x * heading_y - goal_y * heading_x)`, which
equals `K sin(g - theta)` without ever forming either angle. Darkness mode
removes the landmark sprite; the bump persists from self-motion and its
long-run drift is visible on screen, bounded by gate G1.1 (5 degrees over
the 10-minute darkness run for the INT16 pipeline).

Bytes per dispatch: uniform head 48 B read, 16 instances x 16 B written.
Dispatched once per rendered frame.

### 3.2 Olfactory sparse activation: PN to KC expansion view

Biology anchor (A): about 50 projection neuron classes diverge onto about
2,000 Kenyon cells per hemisphere, each KC sampling about 6 claws; APL
feedback inhibition holds the active fraction near 5 percent; KCs project to
21 MBON compartments [H12], [P1].

Browser constants come from the crate: 50 PNs, 2,000 KCs, 5 percent
sparsity target (100 cells), and the hardware specification's 2,048
power-of-two variant is noted where it applies. Both counts are class B
derived from the measured ~2,000 per hemisphere (A).

Engine inputs: the KC activation bitmap (2,000 bits, 250 B per policy tick),
per-KC thresholds (u8, frozen), the claw connectivity CSR (6 u8 input
indices per KC, 12,000 B, frozen and content-hashed with the seed), the PN
rate vector (50 i8), and the tick stamp.

Kernel `kc_bloom`, workgroup size 64, 32 workgroups over the 2,000 cells:

```wgsl
struct ExpansionState {
  tick: f32,
  flash_tau_ticks: f32,
  reserved: vec2<f32>,
}

struct KenyonVisual {
  intensity: f32,
  flash: f32,
  claw_seed: f32,
  selected: f32,
}

@group(0) @binding(0) var<uniform> state: ExpansionState;
@group(0) @binding(1) var<storage, read> activation_bits: array<u32>;
@group(0) @binding(2) var<storage, read_write> last_active_tick: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> cells: array<KenyonVisual>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let index = gid.x;
  if (index >= 2000u) { return; }
  let word = activation_bits[index / 32u];
  let active = (word >> (index % 32u)) & 1u == 1u;
  if (active) {
    atomicStore(&last_active_tick[index], u32(state.tick));
  }
  let since = max(state.tick - f32(atomicLoad(&last_active_tick[index])), 0.0);
  cells[index].intensity = select(0.06, 1.0, active);
  cells[index].flash = exp(-since / state.flash_tau_ticks);
}
```

The active-cell count that drives the sparsity HUD (target 100 of 2,000) is
reduced by the engine and exported in the frame head; the HUD reads the
engine's number and never recounts in JavaScript. The APL arc renders from
the rank-select cutoff threshold exported by the core, so the inhibition
visual is also engine-owned. Whisker lines from a hovered or pinned KC to its
six claw inputs render only for the inspected cell, reading the frozen CSR.

Swarm stretch goal: the expansion compute (popcount over claw inputs,
threshold compare, global rank-select to the top 5 percent) has the same
kernel shape and workgroup layout as `kc_bloom`. If the 1,000-agent batched
demo is built, the dynamics kernel is that shape, and it is admitted only
after bit-exact parity with the CPU core over the fixed episode battery
(prior document, section 3 decision).

Bytes per dispatch: 250 B bitmap plus 8 KB `last_active_tick` read, 2,000
instances x 16 B written. Dispatched on bitmap change, at most once per
rendered frame.

### 3.3 Motor readout: descending neuron funnel view

Biology anchor (A): about 1,300 descending neurons (1,314 counted in the
male CNS) form the neck-connective bottleneck, roughly 100:1 compression
from brain-wide computation [H3], [H4], [H5].

Engine inputs: the DN activity vector (1,300 i8 inference-scale bytes, packed
into 325 u32 words), the arbitration source per DN (2 bits: reflex, CX,
MBON bias), and the command scalars v and omega.

Kernel `dn_funnel`, workgroup size 64, 21 workgroups:

```wgsl
struct FunnelState {
  tick: f32,
  reserved: vec3<f32>,
}

struct DescendingVisual {
  brightness: f32,
  source_tint: f32,
  selected: f32,
  reserved: f32,
}

@group(0) @binding(0) var<uniform> state: FunnelState;
@group(0) @binding(1) var<storage, read> activity_words: array<u32>;
@group(0) @binding(2) var<storage, read> source_words: array<u32>;
@group(0) @binding(3) var<storage, read_write> stubs: array<DescendingVisual>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let index = gid.x;
  if (index >= 1300u) { return; }
  let word = activity_words[index / 4u];
  let byte = (word >> ((index % 4u) * 8u)) & 0xFFu;
  let rate = (f32(byte) - 128.0) / 127.0;
  let source = source_words[index / 16u] >> ((index % 16u) * 2u) & 3u;
  stubs[index].brightness = clamp(rate, 0.0, 1.0);
  stubs[index].source_tint = f32(source);
}
```

A second, single-workgroup reduction kernel derives the two command gauges
(forward velocity and turn rate arcs) from the engine's command scalars and
the arbitration latch; it is 8 threads of work and exists only so the gauge
geometry is also GPU-resident. Arbitration tints encode which source is
currently winning (reflex override, CX steering, MBON valence bias) exactly
as the engine reports it; the renderer never re-derives arbitration.

Bytes per dispatch: 325 B activity plus 82 B source words read, 1,300
instances x 16 B written. Dispatched once per rendered frame.

### 3.4 Dispatch cadence and cost

| Kernel | Trigger | Work items | Read | Written |
|---|---|---:|---:|---:|
| `eb_bump` | every frame | 16 | 48 B | 256 B |
| `kc_bloom` | bitmap change, <= 60 Hz | 2,000 | 8.4 KB | 32 KB |
| `dn_funnel` | every frame | 1,300 | 407 B | 20.8 KB |
| `command_gauges` | every frame | 8 | 32 B | 128 B |

Steady-state GPU time for all derivation kernels combined is budgeted at
0.3 ms per frame (estimate; measured at gate M1 alongside the render pass
budget). Aggregate buffer traffic is on the order of 3 MB per second at
60 Hz, negligible against the frame budget.

## 4. Geometry and asset optimization pipeline

### 4.1 Source data and scale

The reference connectomes are FlyWire/FAFB (139,255 neurons, ~54.5 million
chemical synapses between reconstructed neurons [F1]) and MaleCNS (166,700
neurons, brain plus ventral nerve cord [M1]). Raw per-neuron mesh exports
are far beyond any browser budget; a derived estimate: a mid-size neuron
mesh exports at 50,000 to 500,000 triangles, which at the ~10 B per vertex
budget below is 0.4 to 3.8 MB per neuron, so a full per-neuron mesh set is
on the order of tens to hundreds of GB per connectome (estimate from stated
assumptions, not a published figure). Even single-neuropil raw segmentations
exceed the page budget by orders of magnitude.

Consequence (class B): the browser never receives raw EM-derived geometry.
It receives a curated, decimated, quantized asset set derived offline, with
selection and derivation recorded in a manifest. The assets are orientation
context and story anchors, not measurement surfaces; the scene labels them
as such.

### 4.2 Offline pipeline

A deterministic Rust binary (`viz-prep`, a workspace member) runs the
pipeline in CI, content-hashing every output:

1. Selection: neuropil surfaces for the overview set, exemplar neurons for
   the focus set; the selection list is committed data, not an interactive
   choice.
2. Voxel remesh at a 2 to 8 um grid (resolution choice class B), producing
   watertight surfaces per segment.
3. Quadric error decimation to per-LOD triangle budgets (table in 4.4).
4. Quantization: vertex positions as i16 triples in per-mesh normalized
   bounding boxes (KHR mesh quantization practice), normals as octahedral
   i8 pairs, no texture coordinates; material identity is a per-mesh id.
5. meshopt optimization (vertex cache, overdraw, fetch) followed by the
   meshopt encoder (EXT_meshopt_compression practice).
6. Container assembly and manifest emission (hashes, LOD table, license
   attribution, build hash).

Cost basis: 6 B position plus 2 B normal plus ~2 B amortized index and
metadata gives a working constant of 10 B per vertex after encoding
(budget constant, verified against real assets at gate S3; meshopt ratios
on this asset set are recorded in the manifest at the same gate).

### 4.3 Binary container

One `scene-<hash>.bin` file, memory-mapped as typed arrays at load with no
JSON parsing in the hot path:

| Region | Contents |
|---|---|
| Header | magic, version, build hash, table offsets |
| Mesh table | per mesh: byte offset, length, LOD id, bounding box, quantization box, triangle count |
| Vertex and index blocks | per mesh, meshopt-encoded |
| Instance tables | synapses and somas: position i16 x3 plus attributes u8 x2 per instance |
| Frozen viz data | claw CSR, per-KC thresholds |
| Manifest (sidecar JSON, small) | content hashes, LOD table, license ledger, provenance |

The build hash in the header ties the asset set to the core build,
continuing the prior document's deterministic replay discipline: any episode
sharing (seed, event schedule, build hash) reproduces bit-identically.

### 4.4 Level-of-detail strategy

| Level | Contents | Budget |
|---|---|---|
| L0, streams right after first frame | ~18 neuropil hulls (antennal lobes, MB calyces and lobes, EB, PB, FB, noduli, LAL, optic lobe stubs), 2,500 triangles each | 350 KB |
| L1, on focus | ~50 focus meshes (CX sub-neuropils, MB compartments, glomeruli), 4,000 triangles each | 1.5 MB |
| L2, on selection | ~20 exemplar neurons (one E-PG tile, one P-EN, a KC cohort bundle, the giant fiber pair, MBON and DAN exemplars), 8,000 triangles each | 1.2 MB |
| Primitive instances | active synapses (spheres), somas (spheres), tracts (cylinders) | table below |

Selection rule: camera distance promotes L0 to L1; hover or pin promotes
the touched structure to L2. Instance caps: the demo scene caps the synapse
cloud at 100,000 instances (a curated subset of the 54.5 million counted
synapses [F1]; the cap is class B, bounded by fill rate). Somas and tracts
are capped at 14,000 instances combined.

### 4.5 Asset size budget (target: <= 5 MB total)

All rows use the 10 B per vertex constant and 0.75 vertices per triangle
after dedup (both verified at gate S3):

| Block | Arithmetic | Budget |
|---|---|---:|
| L0 hulls | 18 x 2,500 x 0.75 x 10 B | 350 KB |
| L1 focus | 50 x 4,000 x 0.75 x 10 B | 1.5 MB |
| L2 exemplars | 20 x 8,000 x 0.75 x 10 B | 1.2 MB |
| Synapse instance table | 100,000 x 12 B | 1.2 MB |
| Soma and tract instances | 14,000 x 16 B | 230 KB |
| Frozen viz data (CSR, thresholds) | 2,000 x 7 B | 14 KB |
| Manifest and license ledger | | 32 KB |
| **Total** | | **4.5 MB** |

Headroom to the 5 MB ceiling is 11 percent. The critical-path subset (code
plus manifest only; the first frame is procedural per 2.2) stays under
200 KB compressed. Gate S3 enforces both numbers in CI.

### 4.6 Licensing and provenance

FlyWire and MaleCNS data carry their own licenses. Before any derived mesh
ships in the Space, the license of each source dataset is verified, recorded
in the manifest's license ledger, and linked from the Space README; this
verification is a gate (S3), not an assumption. Exemplar neurons are labeled
by cell class only; no claim is made that a rendered exemplar is the
reconstruction of one specific biological neuron unless the manifest pins
its root id.

## 5. WASM-WebGPU shared memory bridge

### 5.1 State authority and frame block layout

The core owns a `#[repr(C)]` frame block inside its Wasm linear memory,
written once per policy tick, at a fixed offset exported to the render
layer. No field is ever read or written individually from JavaScript; the
main thread moves the whole block.

```rust
#[repr(C)]
pub struct FrameBlock {
    pub tick: u32,
    pub seq: u32,
    pub flags: u32,
    pub active_kenyon_cells: u32,
    pub heading_pair: [f32; 2],
    pub goal_pair: [f32; 2],
    pub omega_raw: i16,
    pub forward_raw: i16,
    pub bump_amplitude: u16,
    pub reserved: [u16; 5],
}

#[no_mangle]
pub extern "C" fn frame_block_ptr() -> *const FrameBlock {
    &ENGINE.frame
}
```

The `HeadingQ8`, `AngleQ16`, and `SynapticWeightQ16` semantics stay inside
the core. What crosses the bridge per frame is: the exported unit pairs
(the core performs the one conversion per tick), raw fixed-point scalars
(decoded in WGSL by a constant divide, a presentation scale), and counts.
A `SynapticWeightQ16` raw i16 decodes to float as `f32(raw) / 256.0`; a
`HeadingQ8` unit decodes to a turn fraction as `f32(units) / 256.0`; an
`AngleQ16` unit as `f32(units) / 65536.0`. These scale factors are the only
fixed-point knowledge the render side holds.

WGSL mirror, laid out to match the Rust field order (total 48 B, uniform
binding):

```wgsl
struct FrameBlock {
  tick: u32,
  seq: u32,
  flags: u32,
  active_kenyon_cells: u32,
  heading_pair: vec2<f32>,
  goal_pair: vec2<f32>,
  scalars: vec4<f32>,
}
```

### 5.2 Transfer paths

Baseline (deployable everywhere, no cross-origin isolation required): a pool
of 8 snapshot ArrayBuffers cycles between the threads. The Worker writes a
snapshot and transfers the buffer to the main thread with
`postMessage(..., [buffer])`, zero-copy handoff; the main thread queues the
bytes into WebGPU and transfers the empty buffer back for reuse. After
warmup there are no allocations on either side.

```ts
const head = new Uint8Array(memory.buffer, frameOffset, FRAME_BLOCK_BYTES);
device.queue.writeBuffer(frameUniform, 0, head);
```

`writeBuffer` performs one asynchronous copy into a driver-internal staging
ring; that single copy is the entire per-frame marshalling cost. There is no
serialization pass, no per-field marshalling, no JSON, and no garbage.

Bulk state rides the same discipline with dirty flags: the full W_out
master (2,000 x 21 `SynapticWeightQ16` i16 values, 84,000 B) uploads only
when a teaching event changed it (bounded at 10 Hz worst case), into an
R16_SNORM texture sampled by the synapse and MBON shaders, exactly as the
prior document specified at the 2,048-row hardware size. The KC bitmap (250
B), PN vector (50 B), and DN vector (1,300 B) upload at the decimated
display cadence, not at tick rate.

Upgrade path (optional): with COOP and COEP response headers, the pool
collapses into one SharedArrayBuffer backing store; the Worker publishes a
snapshot by writing the block then incrementing the sequence number, and
the render thread reads only sequence-even snapshots (seqlock read with
`Atomics.load`). This removes the postMessage hop. Header constraints
decide availability: Hugging Face static Spaces and GitHub Pages cannot set
response headers, so this path is available only through the Docker SDK
Space (section 6) or a future custom origin. The baseline is therefore
load-bearing and is the deployed configuration; the SAB path is an
optimization measured behind a flag.

### 5.3 Bandwidth arithmetic

| Stream | Size | Cadence | Path |
|---|---:|---|---|
| Frame head | 48 B | 60 to 120 Hz | uniform writeBuffer |
| KC activation bitmap | 250 B | <= 60 Hz | storage writeBuffer |
| PN rate vector | 50 B | <= 60 Hz | storage writeBuffer |
| DN activity vector | 1,300 B | <= 60 Hz | storage writeBuffer |
| Event ring entries | 64 B | on event | storage writeBuffer |
| W_out master | 84,000 B | on change, <= 10 Hz | texture writeBuffer |

Steady state is about 1.7 KB per frame, roughly 100 KB per second at 60 Hz;
worst case with W_out churn every teaching event stays under 1 MB per
second. All sizes derive from the crate constants.

### 5.4 Determinism and tear avoidance

The bridge carries no computation; snapshots are pure state copies, so the
prior document's replay discipline (seed, event schedule, build hash) is
untouched. Tearing is prevented structurally: a single writer (the Worker)
writes only into buffers it owns; the render thread consumes only complete
sequence-stamped snapshots; on an incomplete stamp it holds the last
complete snapshot for one frame. No lock is ever taken on the render path.

## 6. Hugging Face Space and drosa.org deployment blueprint

### 6.1 Hosting model

The demo is deployed as the Hugging Face Space `drosalabs/connectome-3d`
(currently responding 401 to anonymous requests, consistent with a private
Space; visibility is Andy's decision at launch). SDK: static. The Space
serves prebuilt assets from CDN with no server process: no cold start, no
queueing on the free CPU tier, and global edge caching. The constraint that
static Spaces cannot set response headers locks in the transferable-buffer
bridge baseline of section 5.2, which is why that baseline exists. The
Docker SDK is the documented upgrade path if response headers (for the SAB
bridge) or server-side telemetry ever become necessary; the upgrade is a
Dockerfile with an nginx config sending COOP and COEP headers, and nothing
else changes.

### 6.2 Repository split

Source of truth: `github.com/drosalabs/drosa`, with the visualizer as the
`viz/` workspace member (prior document, section 9) and the offline asset
pipeline as `viz-prep`. CI builds and tests on every push to `main`.

Deployment target: the Space repository receives built artifacts only, one
commit per release, tagged with the build hash. The Space repository never
holds source; its history is the release log.

### 6.3 Space file scaffold

Exact contents of the Space repository:

```text
connectome-3d/
  README.md
  index.html
  assets/
    app-<hash>.js
    app-<hash>.css
    core-<hash>.wasm
    scene-<hash>.bin
  metadata/
    manifest.json
    licenses.md
```

`README.md` begins with the Space configuration block, followed by the
scope statement (section 8 wording), the control list, repository link, and
embed instructions:

```yaml
---
title: Drosa Connectome 3D
emoji: 🪰
colorFrom: indigo
colorTo: purple
sdk: static
pinned: true
license: mit
---
```

`index.html` is the entire shell; there is no framework and no runtime
template:

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="theme-color" content="#0b0e14">
  <title>Drosa Connectome 3D</title>
  <link rel="preload" href="assets/core.wasm" as="fetch" crossorigin>
  <link rel="preload" href="assets/scene.bin" as="fetch" crossorigin>
  <link rel="stylesheet" href="assets/app.css">
</head>
<body>
  <canvas id="scene"></canvas>
  <div id="hud"></div>
  <script type="module" src="assets/app.js"></script>
</body>
</html>
```

### 6.4 CI workflow

The workflow lives in the source repository at
`.github/workflows/deploy-space.yaml` and runs on pushes to `main` touching
`viz/**`, `src/**`, `Cargo.*`, or the workflow itself:

```yaml
name: deploy-space
on:
  push:
    branches: [main]
    paths: [viz/**, src/**, Cargo.*, .github/workflows/deploy-space.yaml]
jobs:
  core:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
  web:
    needs: core
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: jetli/wasm-pack-action@v0.4.0
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - run: pnpm --filter viz build
      - run: node scripts/check-asset-budget.mjs
      - name: push release to space
        env:
          HF_TOKEN: ${{ secrets.HF_TOKEN }}
        run: node scripts/push-to-space.mjs
```

The toolchain action reads the repository's `rust-toolchain.toml`, so CI,
the Nix devShell, and local rustup builds all compile with the same pinned
version. `check-asset-budget.mjs` fails the build if the dist output
exceeds 5 MB or the critical-path subset exceeds 200 KB compressed (gate
S3). `push-to-space.mjs` clones the Space repository with the token,
replaces its contents with the dist output, and commits with the plain
message `release <build-hash>`.

### 6.5 drosa.org landing site via GitHub Pages

The landing site is a static page published through GitHub Pages from a
`drosalabs` organization repository with `CNAME` set to `drosa.org`. DNS
status as of 2026-09-18: `drosa.org` resolves to parking addresses
(207.207.210.107 and .229) with no HTTPS listener; pointing the records at
Pages is an Andy-owned action recorded in `STATE.md`.

The landing embeds the Space by direct iframe to the runtime host, which
serves the app without the Space page chrome:

```html
<iframe
  src="https://drosalabs-connectome-3d.hf.space"
  title="Drosa Connectome 3D"
  allow="fullscreen"
  loading="lazy"
  scrolling="no">
</iframe>
```

The iframe loads lazily on intersection, shows a skeleton frame until the
app signals readiness over `postMessage`, and falls back to a link card if
the runtime host is unreachable. The same dist bundle is also published
under `drosa.org/viz/` by the Pages workflow: the app has no server
dependency and no cross-origin isolation requirement, so the mirror runs
identically and serves as the outage fallback and the offline demo path.
The Space remains the canonical host referenced in all links.

### 6.6 Interaction controls

The three flagship controls, with their exact wiring into the observation
pipeline. All interaction writes enter the core as observations, never as
direct state mutations, preserving the property that every episode is a
deterministic function of (seed, event schedule) (prior document).

| Control | Wiring | Visible effect |
|---|---|---|
| Light source drag | pointer drag moves the visual landmark in the arena and the 3D scene; the drag writes the landmark bearing into the observation vector; the core re-anchors the heading bump with the gated correction `theta <- theta + k_vis * sin(theta_vis - theta)` (connectome specification 3.4) | the compass bump rotates toward the landmark over a few policy ticks; releasing and toggling lights out removes the anchor and the bump persists from self-motion alone, drifting visibly on long runs |
| Odor stimulus injection | hold-to-inject buttons for three preset odors and a blend pad write PN rate codes into the observation vector; the expansion updates on the next policy tick | the KC band blooms to about 100 of 2,000 cells (5 percent target), the sparsity HUD reads the engine's active count, the APL arc pulses at the exported cutoff; a KC-direct injection tool fires a chosen sparse code bypassing the expansion for readout demonstrations and is labeled an engineering tool, not biology |
| Slice view toggle | one to three clip planes through the 3D scene, applied as fragment-stage discard on signed distance to the plane; presets (coronal, horizontal, sagittal through the EB) plus a draggable plane gizmo; per-layer masks (hulls, synapses, somas, circuits) | cross-sections through neuropil hulls and synapse clouds with zero geometry work; instance counts are CPU-culled by the same plane so slicing also reduces load |

The prior document's full control set (speed, single-tick stepper, event
scrubber, seed field, dopamine slider, plasticity slider, heading
perturbation, punishment placement) carries over unchanged and is not
re-specified here. Touch input uses pointer events with 44 px minimum
targets (class B).

### 6.7 Capability detection and fallbacks

The boot sequence probes in order: WebGPU, WebGL2, Canvas 2D schematic.
WebGPU is available in current Chromium browsers and Safari; Firefox
support varies by platform, and the probe, not a user-agent string, decides
the path. WebGL2 reduces instance caps (KC band at 25 percent density,
arena-only mode at 30 fps); the Canvas 2D schematic renders the signal-path
diagram with live node colors (both carried from the prior document). The
static Space serves one shell for all three paths.

## 7. Fidelity and labeling on the deployed demo

1. Every layer label names its evidence class (measured biology versus
   engineering choice) exactly as fixed in the connectome specification;
   the rendered neuropil hulls and exemplar neurons are labeled as
   decimated orientation assets, not measurement surfaces.
2. The about panel on the Space, the Space card description, and static
   text on the landing page all carry the scope statement: rate-coded model
   dynamics of a Drosa engineering model, no sub-millisecond spike-timing
   claims, no whole-fly emulation claim, activity patterns are model states
   and not recorded fly neurons.
3. The KC-direct injection tool and the 30 fps cap are labeled engineering
   tools; the bump profile width (`bump_kappa`) is a presentation gain and
   is listed in the about panel's rendering notes for transparency.
4. Learning demonstrations report the PI metric computed by the core with
   the G1.2 gate thresholds displayed on the oscilloscope; the UI never
   computes its own score (carried from the prior document).

## 8. Milestones and gates

The prior document's M1 to M4 stand; the deployment gates below attach to
them. A gate records its first measurement when it passes; before that,
every number in this document is a budget or estimate.

| Gate | Attached to | Exit test |
|---|---|---|
| S1 | M1 | Space serves the shell; the HF card renders; the load beacon fires on first visit |
| S2 | M1 | Median cold-cache first-3D-frame under 100 ms and p95 under 200 ms on the declared device class (2.2), measured over five runs |
| S3 | M4 | CI budget check passes on real assets: dist under 5 MB, critical path under 200 KB compressed, license ledger complete |
| S4 | M4 | `drosa.org` iframe interactive end to end; the `/viz/` mirror serves the identical build hash |
| S5 | M4 | Optional privacy-light telemetry: load beacons only, no cookies, no accounts, documented in the Space README |

## 9. References

Connectome and biology sources reuse the evidence ledger of
`docs/architecture/connectome-specification.md`: [C1] Seelig and Jayaraman
2015; [C2] Turner-Evans et al. 2017; [C3] Turner-Evans et al. 2020; [C4]
Hulse et al. 2021; [F1] Dorkenwald et al. 2024; [M1] Berg et al. 2026;
[H3], [H4], [H5] descending control; [H12] Aso et al. 2014; [P1] Li et al.
2020; [P2] Hige et al. 2015.

Engineering references:

- [W1] W3C, *WebGPU Specification*. https://www.w3.org/TR/webgpu/
- [W2] meshoptimizer, vertex processing and compression library.
  https://github.com/zeux/meshoptimizer
- [W3] glTF `EXT_meshopt_compression` and `KHR_mesh_quantization`
  extensions. https://github.com/KhronosGroup/glTF/tree/main/extensions
- [W4] wasm-bindgen. https://rustwasm.github.io/wasm-bindgen/
- [W5] Hugging Face Spaces documentation.
  https://huggingface.co/docs/hub/spaces
- [W6] Cross-origin isolation and `SharedArrayBuffer` requirements.
  https://web.dev/coop-coep/
- [W7] GitHub Pages documentation.
  https://docs.github.com/en/pages
- [W8] FlyWire. https://flywire.ai and MaleCNS https://male-cns.janelia.org/
