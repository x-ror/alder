# lamina

Checks and a safe formatter for Rails views in **ERB, Haml and Slim**, for CI, pre-commit hooks and editors. It is the command line of the [Lamina](https://marketplace.visualstudio.com/items?itemName=x-ror.lamina) VS Code extension, so the editor and CI report the same errors and format the same way.

- **No Ruby needed.** A single prebuilt binary (written in Rust): no gems, no Bundler, fast on large `app/views`.
- **Errors the real engines would raise**: Ruby syntax inside ERB, Haml and Slim (checked with Prism), unclosed `<%` and blocks, mismatched HTML tags, missing `alt`, Haml and Slim indentation and nesting errors, unresolved `render` partials and strict locals mismatches, with Erubi's, Rails', Haml's and Slim's own messages.
- **Formatting that changes only whitespace**: indentation, trailing whitespace, blank lines, the final newline. It never rewraps lines or touches Ruby, checks that the template still compiles to the same thing, and leaves a file unchanged (saying why on stderr) when in doubt.

```sh
npm install --save-dev @x-ror/lamina

npx lamina check app/views            # errors and warnings; exit 1 if any
npx lamina format --check app/views   # list files that would change; exit 1 if any
npx lamina format --write app/views   # format them in place
npx lamina format --stdin-filepath app/views/users/show.html.erb < show.html.erb
```

Exit codes: 0 ok, 1 diagnostics or files that would change, 2 usage or I/O error.

The only formatter setting is the indentation width, from `.editorconfig` (`indent_size`, default 2), plus `--align-comments` for ERB comments; see [Formatting](https://x-ror.github.io/lamina/guides/formatting/) in the documentation.

The binary comes from a per-platform package (Linux x64/arm64, macOS arm64/x64, Windows x64) installed as an optional dependency; `LAMINA_BINARY` points the wrapper at another build.
