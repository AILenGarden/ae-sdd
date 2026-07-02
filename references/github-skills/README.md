# GitHub Skill References

This directory stores third-party skill repositories used as reference material while designing and reviewing ae-sdd SKILL/runtime behavior.

Rules:

- Reference only: files here are not ae-sdd runtime source and are not loaded by `scripts/build_dist.py`.
- Source of truth remains `source/`, `tools/`, and `scripts/`.
- Upstream repository names and download notes are recorded in `DOWNLOAD-REPORT.md`.
- Do not copy upstream code into ae-sdd runtime without an explicit design decision and license review.
- Do not commit nested upstream `.git` directories; keep this directory as a plain reference snapshot.
