# STATE

Last updated: 2026-09-17 by drosa-02-ds

## 1. STATE

- Workspace `/home/andy/drosa` initialized 2026-09-17: git repository on
  branch `main`, no commits before the governance scaffold.
- Project name locked by Andy on 2026-09-17: Drosa. Etymology and naming
  rationale recorded in `README.md`.
- Naming whitespace verified 2026-09-17
  (`~/main/docs/research/2026-09-17-drosophila-naming-whitespace-audit-glm.md`):
  `drosa` unclaimed on crates.io, PyPI, and npm; `drosa.ai`, `drosa.io`,
  `drosa.org`, `drosa.one`, `drosa.dev` show no DNS resolution; the bare
  GitHub user `drosa` is a dormant 2013 personal account, so the organization
  needs a suffixed handle.
- Primary research inputs live read-only in `~/main`: the hardware
  architecture and execution plan
  (`docs/design/2026-09-16-drosophila-hardware-architecture-and-execution-plan.md`),
  the connectome and biological learning audit
  (`docs/research/2026-09-16-fruit-fly-connectome-and-biological-learning-audit.md`
  including the section 15 hardware verification appendix), the naming audit,
  and the open-source and visualization vision
  (`docs/vision/2026-09-16-drosophila-open-source-interactive-viz-and-profile.md`).
- Architecture of record: the audited line-item on-chip budget is 124 KB
  (core configuration) to 165 KB (full configuration with the optional actor
  head), inside a 181 KB conservative envelope. Where the audit appendix
  corrects the earlier design doc (eligibility-trace sizing, drift arithmetic,
  DSP inventory, power scoping, RX 6900 XT training stack), the audit governs.
- Toolchain: Rust pinned through `rust-toolchain.toml` and oxalica
  rust-overlay in `flake.nix`; no host-global Rust toolchain is used.
  Verified end to end 2026-09-17 on rustc 1.94.0: `nix flake check`,
  `cargo build`, `cargo test`, `cargo fmt --check`, and `cargo clippy`
  all pass on the 0.1.0 stub crate.
- Documentation set of record landed 2026-09-17: `README.md` (vision,
  design principles, etymology, budget and precision summaries),
  `docs/architecture/connectome-specification.md` (four subsystems with
  evidence classes and reference ledger),
  `docs/architecture/hardware-mapping.md` (BRAM ledger, quantization and
  drift arithmetic, LUT/DSP inventory, verification gates, risk register),
  `docs/vision/2d-3d-webgpu-visualizer.md` (architecture decision, scene
  layers, interaction model, milestones). The 181 KB figure is used as a
  conservative envelope over the audited 124 to 165 KB line-item totals,
  and the two numbers are always stated together.
- Public 0.1.0 crate foundation landed 2026-09-17: `Cargo.toml` carries the
  crates.io metadata with an include whitelist (`src/**/*`, `Cargo.toml`,
  `README.md`, `LICENSE`) that excludes `docs/` from the packaged tarball;
  `LICENSE` holds the MIT and Apache-2.0 texts. `src/lib.rs` exposes the
  `no_std` population constants (50 AL PNs, 2,000 KCs, 5 percent APL
  sparsity target, 16 EB ring columns, 1,300 DNs), the Neuromodulator,
  Valence, and CircuitModule enums, and the HeadingQ8, AngleQ16, and
  SynapticWeightQ16 fixed-point types (INT16 Q8.8 master with INT8
  inference-byte reads, per the hardware mapping). Verified: 24 unit tests,
  clippy, fmt, and `cargo package --list` free of `docs/` on rustc 1.94.0.
  The crates.io publish itself remains open under the drosa-01-ds queue
  item.

## 2. QUEUE

- [ ] [owner:Andy] [P1] Register core domains `drosa.ai` and `drosa.io`
  (registrar confirmation supersedes the audit's no-resolution observation;
  `drosa.dev` as an optional third).
- [ ] [owner:Andy] [P1] Claim GitHub organization `drosa-ai` and repository
  `zh4ngx/drosa` (the bare `github.com/drosa` user is dormant, so the
  organization handle must be suffixed; the repository itself is named
  exactly `drosa`).
- [ ] [owner:Andy] [P2] Claim Hugging Face organization `drosa-ai`.
- [ ] [owner:drosa-01-ds] [P1] Scaffold and publish minimal 0.1.0 stubs on
  crates.io, PyPI, and npm. Both crates.io and PyPI prohibit name squatting:
  each publish needs a real first artifact (README plus a minimal API
  surface), and LICENSE files (MIT OR Apache-2.0) land in the same change.
- [ ] [owner:drosa-01-ds] [P1] Implement the Phase 1 Rust CPU fixed-point
  reference: `no_std`-friendly core, `I8F8`/`I4F12`-style fixed-point
  arithmetic, zero heap allocation in the control loop, seeded and persisted
  `W_in`. Exit gates G1.1 through G1.5 per the `~/main` audit appendix.
- [ ] [owner:drosa-01-ds] [P1] Implement Phase 2 CubeCL device kernels:
  pinned CubeCL commit, Phase 1 CPU model as the golden reference, batched
  rollouts on the RX 6900 XT inside the fleet `training.slice` standard.
  Exit gates G2.1 through G2.4.
- [ ] [owner:drosa-01-ds] [P2] Build the interactive 2D/3D WebGPU synaptic
  visualizer per `docs/vision/2d-3d-webgpu-visualizer.md`.

## 3. WAITS

- [ ] [waiting:core domains registered] Point the README homepage and Cargo
  metadata at live `drosa.ai`/`drosa.io` URLs; re-run the naming audit's DNS
  probes to confirm resolution.
- [ ] [waiting:GitHub organization claimed] Add the org remote, push the
  repository, set branch protection on `main`.
- [ ] [waiting:Hugging Face organization claimed] Reserve the namespace for
  exported frozen-weight blobs and manifests.
- [ ] [waiting:FPGA boards in hand] Phase 3 RTL exit gates G3.1 through G3.5
  require an Artix-7 XC7A100T board and an AMD Kria K26 or KV260.
- [ ] [waiting:trademark clearance in Nice classes 9 and 42] Required before
  any commercial (non-open-source) use of the Drosa name; the naming audit's
  registry probes were blocked, so clearance is a separate action.
