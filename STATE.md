# STATE

Last updated: 2026-09-21 by drosa-07-ds

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
  crates.io metadata with an include whitelist (`src/**/*`, `/Cargo.toml`,
  `/README.md`, `/LICENSE`) that excludes `docs/` from the packaged tarball;
  `LICENSE` holds the MIT and Apache-2.0 texts. `src/lib.rs` exposes the
  `no_std` population constants (50 AL PNs, 2,000 KCs, 5 percent APL
  sparsity target, 16 EB ring columns, 1,300 DNs), the Neuromodulator,
  Valence, and CircuitModule enums, and the HeadingQ8, AngleQ16, and
  SynapticWeightQ16 fixed-point types (INT16 Q8.8 master with INT8
  inference-byte reads, per the hardware mapping). Verified: 24 unit tests,
  clippy, fmt, and `cargo package --list` free of `docs/` on rustc 1.94.0.
  The publish later landed; see the external presence entry below.
- Packaging whitelist corrected 2026-09-18 by drosa-03-ds: the original
  unanchored include patterns (`Cargo.toml`, `README.md`, `LICENSE`) follow
  gitignore any-depth matching and swept 95 files from the gitignored
  `.direnv/flake-inputs/` nixpkgs tree into `cargo package` (101 files,
  357.0KiB; the `cargo publish` attempt failed at crates.io with a 400
  unverified-email error before upload). Root-anchored patterns cut the
  package to exactly 7 files (43.3KiB, 13.8KiB compressed): `src/lib.rs`,
  `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock`, `.cargo_vcs_info.json`,
  `README.md`, `LICENSE`. `include` was already under `[package]`; table
  placement was not the cause.
- External presence verified 2026-09-18 by drosa-04-ds (read-only probes,
  same day): crates.io `drosa` 0.1.0 is live (created 2026-09-19T00:12:35Z,
  MIT OR Apache-2.0, homepage `https://drosa.org`, repository
  `github.com/drosalabs/drosa`), closing the publish half of the drosa-01-ds
  stub item; GitHub organization `drosalabs` exists (created
  2026-09-18T00:28:53Z, no public repositories yet, so the crate's declared
  repository URL is not yet backed by a visible repo); `drosa.org` resolves
  to parking A records 207.207.210.107 and .229 with no HTTPS listener;
  Hugging Face organization `drosalabs` page is live and
  `huggingface.co/spaces/drosalabs/connectome-3d` returns 401 to anonymous
  requests, consistent with a private Space. The org handle landed as
  `drosalabs`, superseding the `drosa-ai` handle predicted at planning time.
- Visualizer implementation architecture landed 2026-09-18 by drosa-04-ds:
  `docs/vision/webgpu-interactive-visualizer-architecture.md` is the
  implementation architecture of record for the browser deployment (direct
  WebGPU stack decision with a sub-100 ms load budget, dual-loop 1 kHz
  motor / 500 Hz policy timing decoupled from the 60 to 120 Hz render loop,
  three WGSL derivation kernels over engine-exported fixed-point state, a
  mesh decimation and quantization pipeline budgeted at 4.5 MB against the
  5 MB ceiling, a transferable-buffer WASM to WebGPU bridge that needs no
  cross-origin isolation, the static Space scaffold and CI workflow for
  `drosalabs/connectome-3d`, the `drosa.org` GitHub Pages embed plan, and
  gates S1 to S5). It extends and does not amend the decision of record in
  `docs/vision/2d-3d-webgpu-visualizer.md`: dynamics stay in the Rust core,
  WGSL derives render state only.

- Local-learning reference authored 2026-09-19 in `src/plasticity.rs`,
  exported by `src/lib.rs`: fixed INT16 Q8.8 masters, 16-bit unsigned Q15
  replacement eligibility, signed compartment pulses, and deterministic
  event-keyed stochastic rounding. Bounded CPU verification passes 43 tests
  (24 foundation and 19 plasticity, including 8,232 rational-oracle cases),
  `cargo check --offline --lib`, clippy with warnings denied, and fmt on
  Rust 1.94.0. Package listing contains eight files with no docs or cache
  leakage. The service used one CPU equivalent, peaked at approximately
  397 MiB under a 2 GiB cap, and completed in 3.496 s. Device kernels and
  GPU parity are not implemented or measured.
- Local-learning specification authored in
  `docs/architecture/3-factor-plasticity-and-cubecl-learning.md`: biological
  evidence and exclusions, exact overflow and factorization proofs, packed
  CubeCL kernel contracts, allocation-sealing requirements, proposed WIT
  types, and visualizer hooks. README and prior architecture links include
  scoped corrections for row/compartment counts, historical eligibility
  cost, and sub-master rounding. The inspected CubeCL revision requires
  Rust 1.95 versus Drosa's 1.94 pin and contains allocating host dispatch
  paths; dependency adoption and full-runtime sealing remain unverified.
- Publication transport checked 2026-09-19: no git remote is configured;
  `gh repo view drosalabs/drosa` reports no accessible repository
  (re-probed at commit time). The Phase 2 change is verified (fmt, clippy
  with warnings denied, 43 tests) and committed to local `main`; remote
  push waits for Andy to create `drosalabs/drosa` on GitHub and configure
  it as origin. No remote repository creation occurred.
- Publication transport landed 2026-09-21 by drosa-07-ds under a main-ag
  dispatch: the GitHub repository `drosalabs/drosa` was created public
  (`gh repo create drosalabs/drosa --public`), `origin` was configured as
  `https://github.com/drosalabs/drosa.git`, and `main` was pushed with
  upstream tracking at 49bff00, verified identical on both ends with a
  clean working tree. The repository URL declared in the crates.io
  metadata is now backed by a visible public repository. Branch protection
  on `main` is not yet configured (open queue item).

## 2. QUEUE

- [x] [owner:drosa-05-ds] [P1] Land the Phase 2 local-learning specification
  in `docs/architecture/3-factor-plasticity-and-cubecl-learning.md`, the
  `src/plasticity.rs` CPU reference and tests, its `src/lib.rs` export,
  and documentation links and scoped corrections in `README.md`,
  `docs/architecture/connectome-specification.md`, and
  `docs/architecture/hardware-mapping.md`; verify, commit, and push `main`.
  Landed 2026-09-19: fmt, clippy with warnings denied, and 43 tests pass on
  the pinned toolchain; committed to local `main`. Push is deferred to the
  owner:Andy remote item below because no origin is configured. The
  remaining Phase 1 and Phase 2 implementation work stays with drosa-01-ds.
- [ ] [owner:Andy] [P1] Register the remaining core domains `drosa.ai` and
  `drosa.io` (`drosa.org` is registered and parked 2026-09-18, parking A
  records, no HTTPS listener; `drosa.dev` as an optional third).
- [x] [owner:Andy] [P1] Push the repository to `github.com/drosalabs/drosa`
  (landed 2026-09-21 by drosa-07-ds under a main-ag dispatch: repository
  created public, `origin` configured, `main` pushed at 49bff00).
- [ ] [owner:Andy] [P1] Set branch protection on `main` in
  `github.com/drosalabs/drosa` (organization `drosalabs` claimed
  2026-09-18; the public repository landed 2026-09-21).
- [x] [owner:Andy] [P2] Claim the Hugging Face organization (landed as
  `drosalabs`, org page verified 2026-09-18).
- [ ] [owner:Andy] [P1] Set launch visibility for the private Space
  `drosalabs/connectome-3d` (returns 401 to anonymous requests,
  2026-09-18) and reserve the model and dataset namespaces for exported
  frozen-weight blobs and manifests.
- [ ] [owner:Andy] [P1] Stand up the `drosa.org` landing site: Pages
  repository under `drosalabs` with `CNAME drosa.org`, DNS re-pointed from
  the parking records, embedding the Space per section 6.5 of
  `docs/vision/webgpu-interactive-visualizer-architecture.md`.
- [ ] [owner:drosa-01-ds] [P1] Publish minimal stubs on PyPI and npm with
  the same no-squatting bar as crates.io (README plus a minimal API surface,
  LICENSE files in the same change). The crates.io 0.1.0 stub landed and is
  verified live as of 2026-09-18.
- [ ] [owner:drosa-01-ds] [P1] Implement the Phase 1 Rust CPU fixed-point
  reference: `no_std`-friendly core, `I8F8`/`I4F12`-style fixed-point
  arithmetic, zero heap allocation in the control loop, seeded and persisted
  `W_in`. Exit gates G1.1 through G1.5 per the `~/main` audit appendix.
- [ ] [owner:drosa-01-ds] [P1] Implement Phase 2 CubeCL device kernels:
  pinned CubeCL commit, Phase 1 CPU model as the golden reference, batched
  rollouts on the RX 6900 XT inside the fleet `training.slice` standard.
  Exit gates G2.1 through G2.4.
- [ ] [owner:drosa-01-ds] [P2] Build the interactive 2D/3D WebGPU synaptic
  visualizer per `docs/vision/2d-3d-webgpu-visualizer.md` and the
  implementation architecture in
  `docs/vision/webgpu-interactive-visualizer-architecture.md` (deploy
  target: Space `drosalabs/connectome-3d`, embedded at `drosa.org`).

## 3. WAITS

- [ ] [waiting:drosa.ai and drosa.io registered] Point the README and Cargo
  metadata at the full domain set and re-run the naming audit's DNS probes
  to confirm resolution (`drosa.org` is registered, already the crate
  homepage, and parked rather than serving).
- [ ] [waiting:drosa.org DNS re-pointed to Pages] Flip the landing embed
  from the parking placeholder to the live Pages site; verify HTTPS issue
  and the Space iframe per gate S4.
- [ ] [waiting:FPGA boards in hand] Phase 3 RTL exit gates G3.1 through G3.5
  require an Artix-7 XC7A100T board and an AMD Kria K26 or KV260.
- [ ] [waiting:trademark clearance in Nice classes 9 and 42] Required before
  any commercial (non-open-source) use of the Drosa name; the naming audit's
  registry probes were blocked, so clearance is a separate action.
