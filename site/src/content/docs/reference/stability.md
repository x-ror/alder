---
title: Stability
description: What Ruby Templates keeps stable from 1.0, what may change in any release, and how rules, the formatter and deprecations change between versions.
---

From 1.0.0 Ruby Templates follows [Semantic Versioning](https://semver.org). One version number covers the VS Code extension, the `ruby-templates` command (npm `@x-ror/ruby-templates`) and the language server inside them. A change that breaks anything listed under [Covered](#covered) needs a new major version, even when only one of the three is affected.

## Covered

- **Rule names and departments**: every `Department/Name` in the [rules](/vscode-ruby-templates/reference/rules/) and [diagnostics](/vscode-ruby-templates/reference/diagnostics/) references, `Syntax/*` and `Config/Invalid` included. A rule is not renamed, merged or removed in a minor version.
- **`.ruby-templates.toml`**: the tables, keys, options and values described under [configuration](/vscode-ruby-templates/reference/rules/#configuration), how files merge, and the defaults: which rules are on and the default value of every option.
- **The command line**: the `check` and `format` subcommands and their flags, the [exit codes](/vscode-ruby-templates/guides/command-line/#exit-codes), and the shape of each [`--format` output](/vscode-ruby-templates/guides/command-line/#ci): the `text` line format, the GitHub workflow commands, the JSON field names and types, and the SARIF 2.1.0 log structure. New JSON fields and SARIF properties may be added.
- **VS Code**: the ids of the [settings](/vscode-ruby-templates/guides/settings/), the [commands](/vscode-ruby-templates/guides/commands/) and the [languages](/vscode-ruby-templates/guides/file-types/).

## Not covered

These may change in any release:

- The text of diagnostic messages, including the message part of a `text` line. Match on the rule, not the words.
- Which cases a rule reports, within its documented purpose: fixing false positives or catching more of the same problem is a fix.
- `--verbose` output.
- The language server protocol between the extension and its server: `initializationOptions`, custom requests and notifications such as `rubyTemplates/routes`, and `Diagnostic.data`. The server ships inside the extension, and other editors are not supported yet.
- Experimental features: [Liquid](/vscode-ruby-templates/guides/file-types/#liquid), and anything a page labels experimental.

## Rules in minor versions

A minor version may add rules, but a project that passes `ruby-templates check` and `format --check` keeps passing:

- A new formatting rule is off by default.
- A new lint rule is off by default or reports at information severity, which never fails `check`. It may be turned on, or raised, in the next major version.
- A rule's severity may be lowered in a minor version, and raised only in a major one.

## Formatter output

In a minor version the formatter's output changes only to fix wrong output: output that changes how the page renders, that changes again when formatted a second time, or that contradicts the documented behaviour. Style changes wait for a major version.

## Deprecations

A setting, key, flag or rule is deprecated in at least one minor version before it is removed: it keeps working and reports a deprecation note. It is removed only in a major version.

## Older versions and newer config

A `.ruby-templates.toml` that uses a rule or key added in 1.N is reported as a problem (exit 1) by versions before 1.N. Pin `@x-ror/ruby-templates` in CI to a version that knows your config.
