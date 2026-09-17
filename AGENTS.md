# Drosa Workspace Agent Rules

Project: Drosa, a Drosophila-inspired neuromorphic physical AI runtime.
Workspace root: `/home/andy/drosa`. Fleet-wide rules in `~/.dsh/AGENTS.md` bind
every seat operating here; this file adds the project-local contract on top.

## Cold-start read order

1. `STATE.md`
2. `AGENTS.md` (this file)
3. `README.md`

## Seats and roles

| Seat | Role |
|---|---|
| Andy | Project owner. Sole authority over external accounts (domain registration, GitHub organization, Hugging Face organization), publication decisions, and scope changes. |
| main-ag | Orchestrator. Formulates goals, dispatches work, tracks milestones over the CLADE inbox. Does not write into this workspace. |
| drosa-01-ds | Founding specialist seat (DSH). Owns workspace governance, architectural documentation, the Rust fixed-point reference, and device kernels until handed off. |
| drosa-<NN>-<harness> | Ephemeral dispatches scoped to one bounded task, dissolved on completion. |

Specialist state lives in `STATE.md` and the git history of this repository,
never in per-agent memory files.

## Zero code comments

All source files are comment-free: no `//`, no `/* */`, no `///` doc comments,
in every language, including tests, build scripts, shader code, and Nix
expressions. Every explanatory sentence belongs in publication-grade prose
under `docs/` or `README.md`, where it is held to the paper test below. Code
carries meaning through names and structure alone. A change containing a
source comment is rejected regardless of its other merits.

## The paper test

Every document written in this workspace must pass the paper test: it reads as
objective, publication-grade engineering literature. Concretely:

- Third person, declarative, citable. No conversational narration, no
  journaling, no attribution of intent to any person ("Andy asked for" and
  every variant is forbidden in documents).
- Every quantitative claim is either derived in place, cited to the source
  ledger in `~/main`, or marked as an estimate with its stated basis.
- Biological claims carry an evidence label: measured biology versus
  engineering choice versus unsupported. The distinction is never blurred.
- No em-dashes in any file (markdown, code, commit messages, telemetry). Use a
  period, colon, semicolon, or parentheses.
- Prose avoids AI writing tells: stacked jargon, coined labels for plain
  ideas, promotional summary language. Say the plain thing.

## Single-writer discipline

Each file has exactly one writer at a time: the seat that claimed the task in
the `QUEUE` section of `STATE.md`. Reads are unrestricted. A seat never edits
a file claimed by another seat without a handoff recorded in `STATE.md`.
Cross-workspace writes never happen; propose a diff over the CLADE inbox
instead.

## Toolchain

Enter the environment with `nix develop`. The Rust toolchain is pinned by
`rust-toolchain.toml` and materialized through oxalica/rust-overlay; the
devShell and any future Nix package build must use that same pinned toolchain,
never raw `pkgs.rustc` or `pkgs.cargo`. Non-Nix contributors get the identical
toolchain via rustup, which reads the same `rust-toolchain.toml`. Verification
commands: `cargo fmt --check`, `cargo clippy`, `cargo test`.

## Git conventions

- Commit messages: plain imperative subject, no conventional-commit prefixes,
  no AI attribution of any kind (no Co-Authored-By lines, no Generated-with
  footers, no session links).
- Branch names: short descriptive names, no `feat/`, `fix/`, or `chore/`
  prefixes. `docs/` is acceptable for genuinely docs-only branches.
- Commits land at real milestones with a clean working tree.

## Telemetry

Progress reports go to `main-ag` over the CLADE inbox, fire-and-forget:

```
CLADE_AGENT_ID=drosa-01-ds clade-inbox-send main-ag "<one-line summary>"
```

One message per landed milestone, not per step. `STATE.md` is updated in the
same change as any state it records.
