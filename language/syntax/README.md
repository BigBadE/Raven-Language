# Syntax

This folder contains all the structure needed for the general syntax
along with some feature implementation (such as type degenericing).

The main responsibilities of this module are:
- The in-memory representation of a Raven project
- Integrating with the Chalk library
- Providing ways to asynchronously get data

As such, this module contains a lot more general purpose files.
These files should have a header with their various components that explains what they do,
which is why this file doesn't go into depth on the individual files.