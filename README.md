# Estuary [![codecov](https://codecov.io/gh/onelson/estuary/branch/main/graph/badge.svg?token=2NJBNOIRL3)](https://codecov.io/gh/onelson/estuary)

*An estuary* is a coastal semi-enclosed body of water where fresh and salt
waters meet.

Apparently *the blue crab* calls this sort of environment home.

The high-level mission here is to provide a rust package registry in the same
vein as [devpi] (for python packages).

The emphasis on providing a space for publishing private or internal packages
that works with standard tooling (cargo in this case), while also providing a
way to passively cache external dependencies in the service so that builds can
continue even when cut off from the outside world, or during github/crates.io
outages.

[devpi]: https://github.com/devpi/devpi
