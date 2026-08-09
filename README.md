# zed-gsx

[GSX](https://github.com/gsxhq/gsx) language support for [Zed](https://zed.dev),
powered by [`tree-sitter-gsx`](https://github.com/gsxhq/tree-sitter-gsx).

## Features

- Syntax highlighting for Go and GSX markup in `.gsx` files
- JavaScript injection in `<script>` tags and `js` literals
- CSS injection in `<style>` tags and `css` literals
- Language server support via `gsx lsp`
- Bracket matching, indentation, and symbol outline support

## Language Server

The extension launches `gsx lsp`. It first looks for a valid `gsx` binary in
`PATH`, `GOBIN`, and `GOPATH/bin`. If none is found and Go is installed, it runs:

```sh
go install github.com/gsxhq/gsx/cmd/gsx@latest
```

The auto-installed binary is stored in the extension's private storage directory,
not in your global `GOBIN` or `GOPATH/bin`.

For workspaces that use Go's downloaded toolchains, the extension reads the root
`go.mod` and prepends the matching `golang.org/toolchain` `bin` directory when
it is already present in the module cache. This lets `gsx lsp` run with the same
PATH-local Go toolchain that `gsx generate` expects.

To use a specific binary, configure Zed's standard LSP binary setting:

```json
{
  "lsp": {
    "gsx": {
      "binary": {
        "path": "/absolute/path/to/gsx"
      }
    }
  }
}
```

## Install locally

Local development with LSP support requires Rust installed via
[`rustup`](https://rustup.rs/) so Zed can compile the extension Wasm module and
install the `wasm32-wasip2` target automatically.

In Zed, open the command palette and run `zed: install dev extension`, then select
this repository.
