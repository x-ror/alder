---
title: Stability
description: What Alder keeps stable from 1.0, what may change in any release, and how rules, the formatter and deprecations change between versions.
---

From 1.0.0 Alder follows [Semantic Versioning](https://semver.org). One version number covers the VS Code extension, the `alder` command (npm `@x-ror/alder`) and the language server inside them. A change that breaks anything listed under [Covered](#covered) needs a new major version, even when only one of the three is affected.

## Covered

- **Rule names and departments**: every `Department/Name` in the [rules](/alder/reference/rules/) and [diagnostics](/alder/reference/diagnostics/) references, `Syntax/*` and `Config/Invalid` included. A rule is not renamed, merged or removed in a minor version.
- **`.alder.toml`**: the tables, keys, options and values described under [configuration](/alder/reference/rules/#configuration), how files merge, and the defaults: which rules are on and the default value of every option.
- **The command line**: the `check` and `format` subcommands and their flags, the [exit codes](/alder/guides/command-line/#exit-codes), and the shape of each [`--format` output](/alder/guides/command-line/#ci): the `text` line format, the GitHub workflow commands, the JSON field names and types, and the SARIF 2.1.0 log structure. New JSON fields and SARIF properties may be added.
- **VS Code**: the ids of the [settings](/alder/guides/settings/), the [commands](/alder/guides/commands/) and the [languages](/alder/guides/file-types/).

## Not covered

These may change in any release:

- The text of diagnostic messages, including the message part of a `text` line. Match on the rule, not the words.
- Which cases a rule reports, within its documented purpose: fixing false positives or catching more of the same problem is a fix.
- `--verbose` output.
- The language server protocol between the extension and its server: `initializationOptions`, custom requests and notifications such as `alder/routes`, and `Diagnostic.data`. The server ships inside the extension, and other editors are not supported yet.
- Experimental features: anything a page labels experimental.

## Rules in minor versions

A minor version may add rules, but a project that passes `alder check` and `format --check` keeps passing:

- A new formatting rule is off by default, and a department's `enabled = true` doesn't turn it on: only its own `enabled = true` does.
- A new lint rule is off by default or reports at information severity, which never fails `check`. It may be turned on, or raised, in the next major version.
- A rule's severity may be lowered in a minor version, and raised only in a major one.

## Formatter output

In a minor version the formatter's output changes only to fix wrong output: output that changes how the page renders, that changes again when formatted a second time, or that contradicts the documented behaviour. Style changes wait for a major version.

## Deprecations

A setting, key, flag or rule is deprecated in at least one minor version before it is removed: it keeps working and reports a deprecation note. It is removed only in a major version.

## Older versions and newer config

A `.alder.toml` that uses a rule or key added in 1.N is reported as a problem (exit 1) by versions before 1.N. Pin `@x-ror/alder` in CI to a version that knows your config.
