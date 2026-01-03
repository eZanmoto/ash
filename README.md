Ash
===

About
-----

This project defines the Ash programming language, which aims to be used as an
alternative to Bash scripting (and other shell languages in general). It aims to
keep the ergonomics that Bash has for commands and pipelines, while adding more
familiar C-style syntax and semantics that are common in a majority of modern
programming languages.

```ash
print("Hello, world!")
```

Overview
--------

### Goals, non-goals and trade-offs

As with all projects, ideally Ash could do everything efficiently, cleanly and
safely. But as with all projects, this isn't possible in all cases, so when
different priorities are in contention, the design of Ash makes the following
trade-offs:

* Maintainability over performance
* General cases over edge-cases
* High-level composition and delegation over low-level processing

Ash in particular is intended to be used as "glue" for assembling programs as
the composition of other programs, and isn't intended to perform any low-level
or performance-critical processing itself.

#### Correctness without verbosity

As an alternative to Bash, Ash is intended to be used as a scripting language
used to glue other programs and processes together. As such, we aim to minimise
overheads like static typing and other compile-time checks. However, we apply a
number of ideas to the language that are intended to encourage correctness,
while minimising the overheads resulting from such constraints:

* Strongly-typed operations: Operations are not overloaded nor coercive. This
  can help catch simpler type errors without the need for widespread type
  annotations.
* Immutability by default: The default declaration operation (`:=`) creates an
  immutable variable, which can limit the impact of accidentally sharing global
  variables. Declaring variables as mutable only requires an additional
  character.

Installation
------------

At present, this project can only be used by building it from scratch. See the
"Build environment" and "Building" sections under "Development" for more
details.

### With Docker and [Dock](https://github.com/eZanmoto/dock)

If Docker and Dock are installed, then the following can be used to build the
project without needing to install any other tools:

```bash
dock run-in build-env: cargo build --locked
```

### Without Docker

The instructions in `build.Dockerfile` can be followed to prepare your local
environment for building the project. With the local environment set up, the
project can be built using `cargo build --locked`.

Usage
-----

When `ash` is built, it can be used to run an `.ash` script by passing it as the
first argument:

    ash hello.ash

Development
-----------

### Build environment

The build environment for the project is defined in `build.Dockerfile`. The
build environment can be replicated locally by following the setup defined in
the Dockerfile, or Docker can be used to mount the local directory in the build
environment by running `dock`.

### Building

The dependencies for the project must first be installed using
`just install_deps`, or can be installed using `dock` by running the following:

    dock run-in build-env: just install_deps

The project can be built locally using `cargo build --locked`, or can be built
using `dock` by running the following:

    dock run-in build-env: cargo build --locked

### Testing

The project can be tested locally using `just check`, or the tests can be run
using `dock` by running the following:

    dock run-in build-env: just check

A subset of integration tests can be run by passing name patterns to `just`:

    just check add

The commands above will run all integration tests whose name contains "add".
