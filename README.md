# Estuary

[![crates.io](https://crates.io/crates/estuary)](https://img.shields.io/crates/v/estuary.svg)
[![codecov](https://codecov.io/gh/onelson/estuary/branch/main/graph/badge.svg?token=2NJBNOIRL3)](https://codecov.io/gh/onelson/estuary)

An [alternate cargo registry][alternate registry] suitable for *small-scale*
crate publishing and distribution.

---

*An estuary* is a coastal semi-enclosed body of water where fresh and salt
waters meet.

Apparently *the blue crab* calls this sort of environment home.

The high-level mission here is to provide a rust package registry in the same
vein as [devpi] (for python packages).

*Devpi* offers a rich set of features including user-centric indexes,
package search, passive upstream index mirroring and even a web UI.

Today, *Estuary* only supports the most fundamental registry functions:

- publishing
- yanking
- downloading

These features allow us to `cargo install` or use crates from the registry as
dependencies in other crates.

**Estuary does not yet implement any sort of authentication handling and as
such is only appropriate for *internal use*.**


## Installation

```
$ cargo install estuary
```

Estuary depends on being able to run `git` on the command-line.


## Usage

### Estuary Server

Estuary relies on environment variables for configuration.

Required:

- `ESTUARY_INDEX_DIR` Directory to store the git repository used to manage the package index.
- `ESTUARY_CRATE_DIR` Directory path for crate files to be written to.
- `ESTUARY_API_URL` Public root URL for the Estuary server ex: `http://estuary.example.com` (no trailing slash).

Optional:

- `ESTUARY_HOST` defaults to `0.0.0.0`.
- `ESTUARY_PORT` defaults to `7878`.
- `ESTUARY_DL_URL` defaults to `${ESTUARY_API_URL}/api/v1/crates/{crate}/{version}/download`.
- `ESTUARY_GIT_BIN` defaults to just `git`, expecting it to be in the `PATH`. 


When crates are published to Estuary, the `.crate` files are written to
`${ESTUARY_CRATE_DIR}/{crate}/{version}/{crate}-{version}.crate` which
aligns with the default value for `ESTUARY_DL_URL`.


> If you prefer, you can serve the `.crate` files using a separate web server.
> For this, you'd set `ESTUARY_DL_URL` to point to that other web server, using
> the same `{crate}` and `{version}` replacement tokens.
> 
> See the [section on `dl`][index format] in the alternate cargo registry docs 
> for a full list of available tokens cargo can use when building download urls.
> These other tokens may be useful if you end up moving the crate files into a
> different directory layout or have URL rewrite rules to deal with.

With your configuration in place, run `estuary` to launch the server.


### Configuring Cargo

Estuary exposes the package index git repository at the following URL:

```
${ESTUARY_API_URL}/git/index
```

To use Estuary for publishing or installing crates via cargo you need to add
some configuration. 

For example, if you defined `ESTUARY_API_URL` as `http://estuary.example.com`, you
would add the following to your `.cargo/config.toml`:

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

Crates published to Estuary can be listed as dependencies in other
crates by specifying the registry name like so:

```toml
[dependencies]
my-cool-package = { version = "1.2.3", registry = "estuary" }
```

Binary crates can also be installed using the `--registry` flag:

```
$ cargo install --registry estuary my-cool-cli
```

Environment variables can also be used to configure cargo.
See the docs on [using an alternate registry] and
[publishing to an alternate registry] for more on this.

[using an alternate registry]: https://doc.rust-lang.org/cargo/reference/registries.html#using-an-alternate-registry
[publishing to an alternate registry]: https://doc.rust-lang.org/cargo/reference/registries.html#publishing-to-an-alternate-registry
[alternate registry]: https://doc.rust-lang.org/cargo/reference/registries.html
[devpi]: https://github.com/devpi/devpi
[index format]: https://doc.rust-lang.org/cargo/reference/registries.html#index-format
