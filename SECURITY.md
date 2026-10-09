# Security

## Reporting a vulnerability

Please report suspected vulnerabilities through this repository's
[private vulnerability reporting form](https://github.com/codewhale-hq/codewhale-ratatui/security/advisories/new)
(**Security → Report a vulnerability**). Private reporting is enabled. Do not
include vulnerability details, secrets or exploit payloads in public issues
or pull requests.

Include the crate version or commit, Rust version, operating system, terminal,
affected component or example, and steps to reproduce. A minimal reproducer
and an explanation of the impact help maintainers investigate. Remove real
credentials and personal data from reproductions and logs.

## Supported versions

The crate is pre-1.0. Security fixes target the current `main` branch.
Maintenance and responses are best effort, with no guaranteed response time
or bug bounty.

## Scope and boundaries

This repository contains a Rust presentation library, terminal examples,
native character assets and preview/export tools. The host application owns
its event loop, actions, permissions, persistence and animation clock; the
library renders host-supplied state and returns input outcomes.

Terminal interaction is part of the library. Capability detection reads
environment hints such as `COLORFGBG`; on Unix, the query helpers write to
terminal stdout and read replies from terminal stdin. Callers must follow
the helpers' documented raw-mode and startup ordering requirements and replay
carried type-ahead before starting normal input handling.

File access also exists in the repository: examples can load local avatar
manifests, PNG atlases and roster data, and export commands write local
preview files. These inputs and output paths require the same care as other
caller-supplied data; the kit is not a sandbox for its host application.

Relevant security reports include:

- Untrusted display text escaping its intended presentation, including
  terminal control sequences or misleading bidirectional controls.
- Malformed or oversized character, image or roster input defeating parsing,
  allocation or geometry limits.
- Terminal query/reply handling that loses or misinterprets user input,
  accepts malformed replies, or fails to bound input and waiting.
- Unsafe file/path handling in the examples or export tools.

These boundaries describe what to examine, not a claim that every input path
has been security-audited. Reports concerning any code in this repository
are welcome through the private reporting route above.
