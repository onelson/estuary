# Estuary

[![crates.io](https://img.shields.io/crates/v/estuary.svg)](https://crates.io/crates/estuary)
[![crates.io](https://img.shields.io/crates/d/estuary.svg)](https://crates.io/crates/estuary)
![CI](https://github.com/onelson/estuary/workflows/CI/badge.svg)
[![codecov](https://codecov.io/gh/onelson/estuary/branch/main/graph/badge.svg?token=2NJBNOIRL3)](https://codecov.io/gh/onelson/estuary)


An [alternate cargo registry][alternate registry] suitable for *small-scale*
crate publishing and distribution.

---

*An estuary* is a coastal semi-enclosed body of water where fresh and salt
waters meet.

Apparently *the blue crab* calls this sort of environment home.

The high-level mission here is to provide a rust package registry in the same
vein as [devpi] (for python packages) or [verdaccio] (for npm packages).

Estuary aims to be a lightweight cargo registry for internal/private use.

*Devpi* and *Verdaccio* both offer a rich set of features including
search, passive upstream index mirroring and even nice a web UI.

Today, *Estuary* only supports the most fundamental registry functions:

- publishing
- yanking
- downloading
- barely a UI at all

These features allow us to `cargo install` or use crates from the registry as
dependencies in other crates.

**Estuary does not yet implement any sort of authentication handling and as
such is only appropriate for *internal use*.**


## Installation

```
$ cargo install estuary
```

Support for loading environment variables from a `.env` file (off by default)
can be added with:

```
$ cargo install estuary --features dotenv
```

Estuary depends on being able to run `git` on the command-line.

## Usage

### Estuary Server


For a full list of configuration options, run `estuary --help`.

Estuary allows for configuration to be specified by either flags on the command
line, or from environment variables.

Required Configuration:

- `--base-url`/`ESTUARY_BASE_URL` Public URL for the Estuary server, ex: `http://estuary.example.com`.
- `--crate-dir`/`ESTUARY_CRATE_DIR` Path to store crate files.
- `--index-dir`/`ESTUARY_INDEX_DIR` Path to store the git repository (used to manage the package index).

> Note: Estuary relies on being able to run `git` on the command line, and
> expects to be able to find `git` in the `PATH`. If for some reason you're
> running Estuary in an environment where this is not the case, you should
> specify a path to the `git` binary with `--git-bin` or `ESTUARY_GIT_BIN`.

An [example Dockerfile][Dockerfile] is included in the repo and may serve as a
good quickstart guide for deploying Estuary.

[Dockerfile]: https://github.com/onelson/estuary/blob/main/example.Dockerfile

To use the example Dockerfile as-is, you can build and run it like so:

```
# Build the image
docker build -t estuary-quickstart -f example.Dockerfile .

# You'll want to pick a more permanent spot, but this is our volume for package
# and index data.
mkdir /tmp/estuary-data

# Run the image, specifying the port-mapping, volume mount, and base-url
docker run --rm -it -p 1234:7878 -v /tmp/estuary-data:/var/lib/estuary  \
  estuary-quickstart  \
  --base-url=http://localhost:1234
```

### Configuring Cargo

Estuary exposes its package index git repository at the following URL:

```
<base-url>/git/index
```

To use Estuary for publishing or installing crates via cargo you need to add
some configuration. 

For example, if you defined your **base url** as `http://estuary.example.com`,
you would add the following to your `.cargo/config.toml`:

```toml
[registries]
estuary = { index = "http://estuary.example.com/git/index" }
```

With this entry added to your config, the next step is to "authenticate."

```
$ cargo login --registry estuary
```

> Note that Estuary currently does nothing with the token to validate access.
> The token currently *means nothing*, yet cargo *requires it*.

From here, you can publish crates to Estuary with

```
$ cargo publish --registry estuary
```

> You may want to add a [`publish` field][publish field] to your *private packages*
> to ensure they aren't accidentally published to crates.io.
>
> [publish field]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-publish-field

Crates published to Estuary can be listed as dependencies in other
crates by specifying the registry name like so:

```toml
[dependencies]
my-cool-package = { version = "1.2.3", registry = "estuary" }
```

Binary crates can also be installed using the `--registry` flag:

```
$ cargo install --registry estuary my-cool-app
```

Environment variables can also be used to configure cargo.
See the docs on [using an alternate registry] and
[publishing to an alternate registry] for more on this.

## Changelog

### v0.1.0 (2020-12-07)

Initial Release!

This release is very bare bones, offering only the *most essential* registry
features to allow you to publish and download crates.

[using an alternate registry]: https://doc.rust-lang.org/cargo/reference/registries.html#using-an-alternate-registry
[publishing to an alternate registry]: https://doc.rust-lang.org/cargo/reference/registries.html#publishing-to-an-alternate-registry
[alternate registry]: https://doc.rust-lang.org/cargo/reference/registries.html
[devpi]: https://github.com/devpi/devpi
[verdaccio]: https://github.com/verdaccio/verdaccio
[index format]: https://doc.rust-lang.org/cargo/reference/registries.html#index-format
