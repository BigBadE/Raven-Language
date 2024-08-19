# Raven

[![codecov](https://codecov.io/gh/BigBadE/Raven-Language/graph/badge.svg?token=J6vamFlpPp)](https://codecov.io/gh/BigBadE/Raven-Language)
[![DeepSource](https://app.deepsource.com/gh/BigBadE/Raven-Language.svg/?label=active+issues&show_trend=true&token=vt0MHeyRqyL4RlAChpnKveHm)](https://app.deepsource.com/gh/BigBadE/Raven-Language/)
[![CodeScene Code Health](https://codescene.io/projects/46592/status-badges/code-health)](https://codescene.io/projects/46592)

Raven is a modern low-level language prioritizing safety without sacrificing speed or usability.

For some examples of the language, look at the integrated tests in lib/test/test

# Features

## Fully Async Compilation

The Raven compiler is fully async, though it has not been heavily optimized yet (I have some concerns about job
switching and over-zealous locking).

## Traits

Rust-like traits allow for easy code re-use

## Polymorphism

Polymorphic functions and structs go hand-in-hand with traits to make code re-use extremely intuitive to programmers

## User-defined operators

Custom operators can be defined on specific types, making it easier to work with non-builtin types

## Simple Errors

Raven tries to be a simpler language than Rust, ditching the lifetimes and allowing escaping from the borrow checker in
a safe way

## Language Server

Currently, Raven's Language Server only supports syntax highlighting, but more features are planned

# Planned Features

- Loan-based borrow checker to allow for more granular
  borrows (https://smallcultfollowing.com/babysteps/blog/2024/03/04/borrow-checking-without-lifetimes/)
- Self-referential types with view
  types (https://smallcultfollowing.com/babysteps/blog/2024/06/02/the-borrow-checker-within/)
- Algebraic effects to allow async code without function
  painting (https://overreacted.io/algebraic-effects-for-the-rest-of-us/)
- Closures and first-class functions to enable functional programming

# Installing

For help on installing Raven, look at the [first chapter of the in-progress Raven book](docs/raven/1_installation.md)

# Building

To build Raven from scratch, read the [building.md file](docs/building.md)

# Contributing

Please read the [contributing guidelines](contributing.md) before contributing.

# Documentation

For the documentation on the Raven language, look at the [Raven book in the raven folder](raven/raven.md)

For crate-level, file-level, and function-level documentation over the Raven internals, look at the source itself in the
language folder. You can also find a helpful document going over internals in [language/README.md](language/README.md)