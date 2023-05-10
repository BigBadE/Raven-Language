# Modules

- Checker: Verifies all borrows, generics, method calls, returns, etc...
- CLI: Command Line Interface for using Raven
- Compilers: The compilers Raven uses
- Parser: Raven's Lexer and Parser
- Runner: Handles running compilation with the given arguments
- Syntax: Contains the structures for the language's syntax

# Compilation

Compilation is done in the following steps:
- Tokenizer tokenizes the input files (async)
- Parser turns the tokens into the syntax (async)
  - Types are added to the syntax first, then code is resolved later
  - Types are resolved with an async waker system
- Checker verifies code doesn't have undefined behavior
  - Checks lifetimes and generic bounds
  - Logic engine is given generic types to determine inheritance of traits
- Code is compiled (sync)
  - Starts with the main function, adding every called function to the compilation queue
  - Generics are de-sugared instead of compiled