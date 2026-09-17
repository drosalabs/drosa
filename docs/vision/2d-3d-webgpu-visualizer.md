# Drosa Interactive 2D/3D Synaptic Firing Visualizer: Architecture Specification

Date: 2026-09-17
Status: design specification for the browser-based visualizer queued in
`STATE.md` (owner drosa-01-ds, P2)
Depends on: Phase 1 Rust fixed-point reference (the simulation core), the
WebGPU and WebGL2 browser APIs.

---

## 1. Purpose

The visualizer renders the Drosa controller running in real time in the
browser: sparse Kenyon cell firing, the compass bump rotating through the
16-wedge ellipsoid body ring, dopamine-gated synaptic depression, and the
convergence onto descending commands and CPG oscillators, all live, all
interactive. It serves three audiences:

1. The open-source release: a visitor with no hardware background watches
   the architecture learn an association in five pairings, with every
   internal signal visible.
2. The neuromorphic engineering community: the visualizer doubles as an
   inspection probe for the Phase 1 reference model, with deterministic
   replay of any recorded episode.
3. Teaching: each scene layer maps one-to-one onto a section of the
   connectome specification, so the visualization is a walkable diagram of
   the document.

The visualizer shows the Drosa engineering model, not a biological animal.
Every layer carries a label naming its evidence class per the connectome
specification, and the about panel states that activity patterns are
rate-coded model states, not recorded fly neurons.

## 2. Scene layers

| Layer | Geometry | Data source | Visual behavior |
|---|---|---|---|
| Arena (2D top-down) | ground plane | agent pose, odor plumes, landmarks, punishers | agent trajectory trail; home-vector arrow; plume isosurfaces as translucent fields |
| Sensory periphery | radial arc of 50 PN nodes | INT8 rate registers | node luminance tracks rate code; halo shows baseline deviation |
| Expansion fan | 50 PN nodes to 2,048 KC points in a 3D band | W_in CSR, KC activation bitmap | active ~5% bloom bright; inactive points dim; brief flash on threshold crossing |
| APL inhibition | one arc over the KC band | rank-select state | arc pulses once per tick, its radius reflecting the cutoff threshold that trims to 5% |
| MBON compartments | 21 rows behind the KC band | W_out master bytes | row color encodes valence weight; synapse lines from active KCs |
| Synaptic field | up to 2,148 active synapse lines (102 x 21) per tick | W_out bytes, eligibility traces | line opacity maps |weight|; a teaching event flashes the compartment's lines and visibly dims depressed lines |
| Dopamine compartments | 21 small emitters on the MBON rows | teacher traces, event queue | emitter burst on teaching event with baseline-centered color (red punishment, green appetitive) |
| Central Complex ring | 16-wedge torus (EB) + PB glomeruli arc + FB columns | heading bump, phase accumulator, goal vector | bump of activity rotates with angular velocity; in darkness mode the visual landmark disappears and the bump keeps rotating from self-motion; goal marker and steering needle show sin(g - theta) |
| Home vector | arrow from agent to home in arena; FB column activation | integrated position | arrow length and direction; drift is visible on long darkness runs |
| Descending funnel | 1,300 dimmed stubs converging to 2 command gauges | arbitration state | stubs brighten with contribution source (reflex, CX, MBON bias); gauges show v and omega |
| CPG oscillators | phase circles for leg tripod and wing-beat | CPG phase accumulators, ROM readouts | rotating phase pointers with waveform traces |
| Oscilloscope panel | 2D overlay traces | ring buffer of any tapped signal | selectable cell rates, membrane proxy, MBON sums, PI over time |

The 3D scene organizes the layers along the signal path (periphery at left,
CX ring center, MB band right, funnel and CPGs at right edge), with the 2D
arena inset. All layers derive from one shared state snapshot per rendered
frame; nothing renders from stale or mixed-tick state.

## 3. Architecture decision: where the simulation runs

Three candidate architectures were considered:

| Option | Description | Assessment |
|---|---|---|
| A. Native WGSL compute | full simulation as WebGPU compute kernels | maximum throughput, but duplicates every fixed-point semantic in a second language; parity with the Phase 1 golden model becomes a permanent verification burden; the audit's CPU-golden-model discipline exists precisely to prevent this |
| B. WASM simulation core, WebGPU rendering | Phase 1 Rust core compiled to WebAssembly, stepped on a Worker thread; WebGPU used for rendering only | single source of truth for dynamics: the same crate that passes gates G1.1 to G1.5 runs in the browser; rendering is a pure function of exported state |
| C. Server-streamed frames | simulation on a server, frames to the client | contradicts the open-source goal (no required infrastructure), adds latency, breaks the portfolio demo story (must run offline) |

**Decision: option B.** The Phase 1 fixed-point core is compiled to WASM
with `wasm-bindgen`, stepped at the model's native cadence inside a Web
Worker, and rendered with WebGPU. Option A remains a stretch goal for a
batched 1,000-agent swarm demo once the single-agent path is proven, and it
must reproduce CPU parity tests first.

Consequences:

- The browser model is bit-identical to the reference model by construction
  (same crate, same seed, same integer semantics; WASM i32/i64 arithmetic is
  exact).
- Fixed-point semantics are never hand-ported to TypeScript or WGSL. The
  TypeScript layer handles UI state, interpolation, and rendering only.
- The simulator exposes a schema-versioned snapshot API (section 4), so the
  renderer and the core can evolve on separate schedules.

## 4. Data flow and timing

```text
+---------------------------+       500 Hz (configurable to 50 Hz)
|  Phase 1 core (WASM)      |  step(observation) -> StateSnapshot
|  in Web Worker            |  deterministic, seeded
+------------+--------------+
             |  postMessage, transferable ring buffer
             v
+---------------------------+       60 fps
|  Main thread:             |  decimate / interpolate snapshots
|  render state builder     |  -> per-layer uniform + instance buffers
+------------+--------------+
             |
             v
+---------------------------+
|  WebGPU render graph      |  instanced points, line batches,
|  (fallback: WebGL2)       |  arena pass, oscilloscope overlay
+---------------------------+
```

- The Worker steps the core at the policy rate (500 Hz default; a 50 Hz
  slow-motion mode and a step-by-step single-tick mode exist for teaching).
- Snapshots enter a ring buffer (~8 seconds at 500 Hz, ~30 KB per snapshot
  head: activation bitmap 256 B, MBON rows, CX state, telemetry scalars; the
  full 86 KB W_out is transferred only on change events, not per tick).
- The main thread decimates to the display rate with hold-last semantics
  for discrete signals (KC bitmap) and linear interpolation only for
  continuous quantities (heading, gauges). No rendered frame mixes two
  simulation ticks.
- Teaching events, threshold crossings, and reflex triggers are recorded as
  timestamped events for the scrubber (section 6).

## 5. Rendering techniques

- **Instanced geometry.** The 2,048 KCs render as one instanced point
  cloud (one draw call); PN nodes, MBON rows, and DN stubs are likewise
  instanced. Total draw calls stay under 40 for the full scene.
- **Synapse lines.** Active synapses render as a batched line list capped
  at 2,148 segments (102 active KCs x 21 rows); line color maps weight sign
  and magnitude (diverging colormap), opacity maps recency of eligibility.
  Depression events animate a line from bright to its new resting opacity
  over ~300 ms so the change is perceptible.
- **Weight texture.** The full W_out master uploads as a 2,048 x 21
  R16_SNORM texture (86 KB) on change, sampled by the synapse and MBON
  shaders; the browser never re-derives weights.
- **Compass bump.** The 16-wedge ring renders as instanced arc segments
  with intensity from the bump vector; rotation uses the exported unit
  pair, never client-side trigonometry on a wrapped angle.
- **Depth-of-interaction.** Hover on any KC, MBON, or wedge pins an
  inspector card (indices, current value, threshold, last event) and adds
  its signals to the oscilloscope.
- **Fallback.** Where WebGPU is unavailable the same render graph
  re-targets WebGL2 with instance caps reduced (KC band at 25% density,
  arena-only mode at 30 fps). A pure-Canvas 2D schematic mode renders the
  signal-path diagram with live node colors for low-end devices.

## 6. Interaction model

| Control | Effect on the core |
|---|---|
| Odor plume drag | moves a plume source; observation vector recomputes per tick |
| Landmark drag / lights-out toggle | anchors or removes the visual landmark; darkness mode demonstrates dead reckoning and drift |
| Punishment placement | places a shock region; crossing it emits a compartment teacher event |
| Dopamine injection slider | manual teacher event with signed magnitude, routed to a chosen compartment |
| Plasticity rate slider | scales eta against the master LSB (bounds enforced by the core, not the UI) |
| Heading perturbation | injects angular velocity steps; bump and phase accumulator respond |
| Speed control | 0.05x to 1x real time; single-tick stepper |
| Event scrubber | seek to any recorded timestamp and replay deterministically from the nearest checkpoint |
| Seed field | fixed seeds for reproducible demos; random for exploration |

Interaction writes go through the same observation pipeline as simulated
sensors: the UI never mutates model state directly, so every episode
remains a deterministic function of (seed, event schedule).

## 7. Performance budget

| Quantity | Budget |
|---|---:|
| Simulation step (WASM, single agent, 500 Hz) | well under the 2 ms tick budget on a 2020-class laptop (Phase 1 gate G1.4 bounds the native core; WASM parity is measured at M2) |
| Snapshot transfer | ~30 KB per 60 Hz frame, transferable buffers, zero copy |
| Draw calls | <= 40 |
| Instances | ~5,500 points/segments in full mode |
| Frame budget | 16.6 ms; render pass target <= 6 ms at 1080p integrated GPU |
| Heap | <= 64 MB including WASM module and ring buffers |

## 8. Fidelity and labeling rules

1. Every layer label names its evidence class (measured biology versus
   engineering choice) as fixed in the connectome specification; the MBON
   to DN direct route is drawn alongside the goal-weight route, both real
   anatomy, while the rank-select APL proxy is labeled a model
   approximation.
2. No neuron in the scene is claimed to be a specific biological neuron
   unless the connectome specification pins it (for example EB wedge
   count, PB glomerulus count, DN population size).
3. Learning demos report the PI metric live, computed by the core, with
   the same gate thresholds as G1.2 displayed on the oscilloscope; the UI
   never computes its own score.
4. The about panel states scope: rate-coded model dynamics, no
   sub-millisecond spike-timing claims, no whole-fly emulation claim.
5. Deterministic replay: any episode sharing (seed, event schedule, build
   hash) reproduces bit-identically; the build hash is derived from the
   crate version and the W_in manifest hash.

## 9. Build and packaging

- The simulation core is the Phase 1 crate compiled with
  `wasm-bindgen`/`wasm-pack` (`no_std`-friendly core plus a thin std shell
  for the Worker).
- The renderer is TypeScript with the WebGPU API, bundled by Vite; the npm
  package name `drosa` is queued for registration with the registry stub
  milestone.
- The visualizer ships in the open-source repository as a `viz/` workspace
  member with its own minimal CI (build + a headless render smoke test),
  and deploys as a static site to GitHub Pages under the claimed
  organization.

## 10. Milestones

| Milestone | Deliverable | Exit test |
|---|---|---|
| M1 | WASM bridge + arena + PN arc + KC band with 5% sparsity, live at 60 fps | bit-identical snapshot parity vs native core on a fixed 10^4-tick episode |
| M2 | CX ring with bump rotation, darkness mode, home vector, steering needle | darkness demo shows bounded drift per G1.1 model |
| M3 | MBON rows, synapse field, dopamine events, PI oscilloscope, scrubber | scripted 5-pairing conditioning episode replays deterministically and reaches PI >= 0.7 |
| M4 | DN funnel, CPG oscillators, WebGL2 fallback, teaching labels, deploy | full-scene 60 fps on integrated GPU; fallback modes pass; Pages deployment live |
