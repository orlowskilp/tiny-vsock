# Tiny vsock library

[![codecov](https://codecov.io/github/orlowskilp/tiny-vsock/graph/badge.svg?token=2R7TFgUos4)](https://codecov.io/github/orlowskilp/tiny-vsock)
[![MIT License](https://img.shields.io/badge/license-MIT-green)](/LICENSE)

Minimal vsock library, keeping the bare minimum functionality to communicate over vsock
with minimum set of dependencies

## Features

These are additional features supported by the library

### `std-io`

Implements `io::Write` and `io::Read` traits for compatibility.

**Note**: It is strongly suggested to use the provided `Vsock::send` and `Vsock::receive`
methods instead as they handle buffering and data caps for increased performance and
security.

---

Copyright (c) Lukasz Orlowski <lukasz@orlowski.io>. All rights granted under MIT license.
