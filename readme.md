# crunch-cli

![Crates.io Version](https://img.shields.io/crates/v/crunch-app)

Turbocharge your Rust workflow.

`crunch` seamlessly integrates cutting-edge hardware into your local development environment. Cut compile times and iterate faster.

## Usage

Get started with no new commands or flags to learn, just replace `cargo` with `crunch`.

```bash
c̶a̶r̶g̶o̶crunch check
c̶a̶r̶g̶o̶crunch clippy --workspace
c̶a̶r̶g̶o̶crunch t -p sys-internals
```

## Installation

```bash
cargo install crunch-app
```

## Setup

### Bring-Your-Own-Hardware

1. Install Rust on a Debian-based machine
2. Add a `crunch` host to your `~/.ssh/config`

```text
Host crunch
  HostName your-machine-ip
  User your-machine-user
  IdentityFile ~/.ssh/your-key.pem
  ControlMaster auto
  ControlPath ~/.ssh/control-%r@%h:%p
  ControlPersist 5m
```

### Provisioned Hardware

Coming soon!

## rust-analyzer

Using `crunch` with `rust-analyzer` frees up local resources and can enable faster LSP hint feedback.

Just set `rust-analyzer.check.overrideCommand` in your LSP configuration to your preferred `crunch` command.

e.g. for VSCode, set

```text
  "rust-analyzer.check.overrideCommand": [
    "crunch",
    "check",
    "--quiet",
    "--workspace",
    "--message-format=json",
    "--all-targets",
    --all-features
  ],
```

in your `settings.json`.

## Advanced Usage

See `crunch --help` for advanced usage options and examples.

## `cargo-remote`

`crunch` was inspired by [cargo-remote](https://github.com/sgeisler/cargo-remote).

The largest difference at the moment is `crunch` aims to be as simple to use as possible:

- Just replace `cargo` with `crunch`
- Zero configuration (aside from a `~/.ssh/config` host when bringing your own hardware)
