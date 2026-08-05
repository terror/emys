## honu

<img align="right" width="125" height="125" src="etc/icon.png">

[![build](https://img.shields.io/github/actions/workflow/status/terror/honu/ci.yaml?branch=master&style=flat&labelColor=1d1d1d&color=424242&logo=GitHub%20Actions&logoColor=white&label=build)](https://github.com/terror/honu/actions/workflows/ci.yaml)
[![codecov](https://img.shields.io/codecov/c/gh/terror/honu?style=flat&labelColor=1d1d1d&color=424242&logo=Codecov&logoColor=white)](https://codecov.io/gh/terror/honu)

`honu` records, imports, and searches your shell history with SQLite.

<img width="1667" alt="val" src="screenshot.png" />

If you need help with `honu` please feel free to open an issue. Feature requests
and bug reports are always welcome!

## Installation

`honu` should run on Linux, macOS, and Windows.

For now, install the latest version from source using
[cargo](https://doc.rust-lang.org/cargo/), the Rust package manager:

```bash
cargo install --git https://github.com/terror/honu
```

## Prior Art

This project was inspired by tools like
[atuin](https://github.com/atuinsh/atuin) and [stinkpot](https://tangled.org/oppi.li/stinkpot).
