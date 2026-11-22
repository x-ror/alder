---
title: Command line for CI
description: Run the same template checks and formatter outside the editor with the alder command from npm.
sidebar:
  order: 8
---

The same checks and formatter run outside the editor, from npm, with no gems to install.

## Install

```sh
npm install --save-dev @x-ror/alder
```

`npx alder --version` prints the version and `--help`, also after `check` or `format`, the usage; both exit with 0.

## Check templates

```sh
npx alder check app/views
```

`check` prints the diagnostics (errors and warnings) of ERB, Haml, Slim, Pug, Jbuilder and Builder templates, one per line with the file, line and column. Pug templates get Pug's own errors, their `include` and `extends` checks and the HTML rules, none of the Rails ones. Jbuilder and Builder views are plain Ruby: `check` reports their partial and `locals:` problems, as the editor does, and leaves their Ruby syntax errors to Ruby LSP or `ruby -c`:

```text
app/views/users/index.html.erb:4:3: error: unexpected end-of-input, assuming it is closing the parent top level context [Syntax/Ruby]
app/views/users/index.html.erb:3:5: warning: `<img>` without `alt`: the alt attribute must be specified (HTML 4.8.4.4.1) [Html/ImgAlt]
```

The rule at the end is the name to use in a [`.alder.toml`](/alder/reference/rules/#configuration); a problem in the file itself ends in `[Config/Invalid]`. It exits with 1 if there are any, or if a `.alder.toml` has a problem (printed the same way, once per file). The checks are the ones described in [Errors as you type](/alder/guides/checks/); the command line checker reads ERB as Erubi does, so `erb-percent` `%` lines are not modelled.

## Format templates

```sh
npx alder format --check app/views   # files that would change; exit 1 if any
npx alder format --write app/views   # format them
```

Without `--check` or `--write`, the result goes to standard output. A file whose structure is in doubt is left as it is, with the reason on standard error. See [Formatting](/alder/guides/formatting/) for what the formatter changes.

### Options

| Option                    | What it does                                                                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `--check`                 | Lists the files that would change; exits with 1 if any, or if a `.alder.toml` has problems                                                 |
| `--write`                 | Writes the formatted files                                                                                                                 |
| `--verbose`               | Also prints the settings, and the `.editorconfig` and `.alder.toml` files used                                                             |
| `--align-comments`        | Moves the lines inside ERB comments with the line the comment opens on; it can only turn this on, over `format.align_comments` in the file |
| `--stdin-filepath <path>` | Formats the template read from standard input as the file at `<path>`, for editors                                                         |

```sh
npx alder format --stdin-filepath show.html.erb < show.html.erb   # from stdin, for editors
npx alder format --verbose --check app/views   # also the settings and .editorconfig files used
```

## Configuration

Both commands read the `.alder.toml` files above each template, as the editor does: rules turned off, files excluded, `[erb] format.enabled = false` (also `[haml]`, `[slim]`, `[pug]`), `format.align_comments`, `extra_engines`. The [rules reference](/alder/reference/rules/#configuration) describes the file. `format` prints a file's problems once on standard error and fails only with `--check`, never when it prints the result for an editor (`--check --format github` annotates them). `format --verbose` also prints each file read; an excluded file is reported as `left unchanged: excluded by <file>`.

## Files and folders

Pass files or folders. Folders are walked for template files, skipping hidden folders, `node_modules`, `vendor`, `tmp`, `public`, `log` and `lib/templates` (Rails generator templates, which are ERB that produces templates, not views). `vendor`, `tmp`, `public` and `log` are walked inside `app/views`, where they can be view folders. A file named on the command line is always checked, and `.gitignore` is not read. `check` and `format` walk the same folders.

## Exit codes

| Code | Meaning                                                                                    |
| ---- | ------------------------------------------------------------------------------------------ |
| `0`  | OK                                                                                         |
| `1`  | Diagnostics (errors or warnings), a problem in a `.alder.toml`, or files that would change |
| `2`  | Usage or I/O error, a formatter bug, or (for `check`) a named file that isn't a template   |

## GitHub Actions

With `@x-ror/alder` in your `package.json` dev dependencies, add steps like these to a job that has checked out the repository:

```yaml
- run: npm ci
- run: npx alder check app/views
- run: npx alder format --check app/views
```

Each step fails the job when the command exits with a non-zero code.

## CI

`check --format` changes how the diagnostics print; the exit codes stay the same.

| Format   | Output                                                                                                            |
| -------- | ----------------------------------------------------------------------------------------------------------------- |
| `text`   | `file:line:column: severity: message [Rule]`, the default                                                         |
| `github` | GitHub Actions workflow commands, which annotate the files of a pull request                                      |
| `sarif`  | One SARIF 2.1.0 log, for code scanning                                                                            |
| `json`   | One JSON object per line, with `file`, `line`, `column`, `endLine`, `endColumn`, `severity`, `rule` and `message` |

In `github`, `sarif` and `json` the paths are relative to the current folder, so run the command from the repository root for GitHub to find the files; SARIF carries absolute `file:` URIs, which code scanning maps to the repository. Columns count characters from 1 in every format, as GitHub shows them, and a leading byte order mark takes none. Every format carries the rule: `Html/ImgAlt` is the `title` of an annotation, the `rule` of a JSON line, and a SARIF rule with its summary and a link to the [diagnostics reference](/alder/reference/diagnostics/). A problem in a `.alder.toml` has the rule `Config/Invalid`. Errors are `error`, warnings `warning` and information (such as `Rails/InstanceVariableInPartial`) `info`, in `text` and `json`; GitHub and SARIF use their own words for it, `notice` and `note`. A problem in a `.alder.toml` is reported the same way, against that file, before the results of the first template that uses it.

Annotations on the pull request:

```yaml
- uses: actions/setup-node@v4
- run: npx @x-ror/alder check app/views --format github
```

Code scanning alerts, from the SARIF file (the step that creates it must not stop the job, so the upload runs):

```yaml
- run: npx @x-ror/alder check app/views --format sarif > templates.sarif
  continue-on-error: true
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: templates.sarif
```

`format --check --format github` annotates each file that would change with a `Format` warning.
