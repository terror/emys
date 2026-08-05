## honu

<img align="right" width="125" height="125" src="etc/icon.png">

[![release](https://img.shields.io/github/release/terror/honu.svg?label=release&style=flat&labelColor=1d1d1d&color=424242&logo=github)](https://github.com/terror/honu/releases/latest)
[![crates.io](https://img.shields.io/crates/v/honu.svg?style=flat&labelColor=1d1d1d&color=424242&logo=rust)](https://crates.io/crates/honu)
[![build](https://img.shields.io/github/actions/workflow/status/terror/honu/ci.yaml?branch=master&style=flat&labelColor=1d1d1d&color=424242&logo=GitHub%20Actions&logoColor=white&label=build)](https://github.com/terror/honu/actions/workflows/ci.yaml)
[![codecov](https://img.shields.io/codecov/c/gh/terror/honu?style=flat&labelColor=1d1d1d&color=424242&logo=Codecov&logoColor=white)](https://codecov.io/gh/terror/honu)
[![downloads](https://img.shields.io/github/downloads/terror/honu/total.svg?style=flat&labelColor=1d1d1d&color=424242)](https://github.com/terror/honu/releases)

`honu` records, imports, and searches your shell history with SQLite.

<img width="1667" alt="val" src="screenshot.png" />

If you need help with `honu` please feel free to open an issue. Feature requests
and bug reports are always welcome!

## Installation

`honu` should run on any system, including Linux, MacOS, and Windows.

The easiest way to install it is by using
[cargo](https://doc.rust-lang.org/cargo/index.html), the Rust package manager:

```bash
cargo install honu
```

Otherwise, see below for the complete package list:

#### Cross-platform

<table>
  <thead>
    <tr>
      <th>Package Manager</th>
      <th>Package</th>
      <th>Command</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td><a href=https://www.rust-lang.org>Cargo</a></td>
      <td><a href=https://crates.io/crates/honu>honu</a></td>
      <td><code>cargo install honu</code></td>
    </tr>
    <tr>
      <td><a href=https://brew.sh>Homebrew</a></td>
      <td><a href=https://github.com/terror/homebrew-tap>terror/tap/honu</a></td>
      <td><code>brew install terror/tap/honu</code></td>
    </tr>
  </tbody>
</table>

### Pre-built binaries

Pre-built binaries for Linux, MacOS, and Windows can be found on
[the releases page](https://github.com/terror/honu/releases).

## Prior Art

This project was inspired by tools like
[atuin](https://github.com/atuinsh/atuin) and [stinkpot](https://tangled.org/oppi.li/stinkpot).
