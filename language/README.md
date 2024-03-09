# Modules

- Checker: Verifies all borrows, generics, method calls, returns, etc...
- Compilers: The compilers Raven uses
- Data: Settings data used by every package. Seperated to prevent dependency loops.
- Parser: Raven's Lexer and Parser
- Runner: Handles running compilation with the given arguments
- Syntax: Contains the structures for the language's syntax

# Terms

- Async/Asynchronous: Code that runs at the same time as other code, which is generally faster but needs special
  considerations to prevent bugs. See Arc and Mutex on the Rust docs for more info on how we stop multithreading
  bugs.
- Finalization: The process of linking methods and structures, and verifying that all types are correct.
- Linking: The process of attaching a name to the associated data. Can either be internal (for example, finding the code
  of the function called "main" from the name) or external (for example, calling a method in the C library like printf).
- Generics: Types that aren't "solid" and instead can be any type within the given bounds. These are "solidified" into
  their actual types.
- Compilation-time: Something that happens during compilation:
- Runtime: Something that happens when the program runs
- Constant: Something that is computed at compilation time, for example generics are constant types because the compiler
  degenerics them at compiler-time.
- Desugaring: Certain things are known as syntax sugar, which means it's just an easier way to write code. For example,
  if statements are really just jumps. Desugaring is the process of turning syntax sugar into its raw form for
  compilation.

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

For example, compiling the following example:

```
import stdio;

fn main() {
    printf("Two");
}
```

- A syntax object is created in runner/runner
- The compiler is started on another thread, waiting for code to be ready to compile (line 35). It's important to note
  that the compiler only runs on one thread (LLVM is not multithreaded), but it still runs on another thread.
- The parsing job is created on a new thread in runner/runner (line 46)
    - parser/tokens/top_tokenizer finds the import and the main function with no modifiers or arguments, and makes it
      into a UnfinalizedFunction
    - parser/tokens/code_tokenizer finds the method printf and turns it into a line with a single Effect: MethodCall
      with the argument "Two"
- Tokenizer returns, and the list of tokens is passed to the parser and a ParserUtils object is created to keep track of
  the parsing progress (parser/lib)
    - parser/parser/top_parser finds the import, and adds it to the ParserUtil's import list (parser/parser/top_parser
      line 85)
    - parser/parser/top_parser finds the function, and passes it to parser/parser/function_parser
    - parser/parser/function_parser finds no attributes or generics, finds a code body, and passes it to
      parser/parser/code_parser
    - parser/parser/code_parser finds a single line with one effect, a method call which is added to the code body and
      returned. This code is NOT linked, the method call doesn't actually know anything about what method it calls. This
      allows recursive functions to work
- Parser returns, and passes the single function to ParserUtils::add_function which passes it to Syntax::add
    - The function is added to syntax.types so it can be found when linking
    - Anything waiting for that function wakes up (nothing calls main, so nothing does)
- A verifying job is created on a new thread in parser/parser/top_parser (line 29) to verify the function
    - The code is split into a CodelessFinalizedFunction and a CodeBody as the function itself is finalized
    - The code is finalized, the method effect which is the only line gets turned into a FinalizedEffect
        - checker/check_code adds a waiter for the printf function to finish if it isn't finished yet.
        - When it is finished, the waker is woken up and the checker finds the FunctionData for printf using the
          previously found imports (see syntax/async_util/AsyncTypesGetter to see how functions are found)
- The finalized function (with the code) is put into compiling list, which the compiler thread can find.
    - The compiler finds the main function, and starts compiling it
    - The compiler finds the method call to printf (compilers/llvm/function_compiler line 177), adds a call to the
      function, and adds it to the compiling list
    - The compiler compiles every function in the compiling list (compilers/llvm/type_getter line 105), compiling each
      function, which may add more to the compiling list
- After the compiler has an empty compiling list it calls the main function and sends the result back to the runner (
  which is empty).

It's important to note how the design of the compiler reflects a lot of the goals of the language:

- Each module is separate, with only the syntax to unify them, allowing multiple compilers to work with the same
  language.
- The compiler is multi-threaded and job-based (where each async task is a single "job", and the async library (tokio in
  this case) manages which jobs are running). This is why the program has "waiters", which allow jobs to wait for other
  jobs to happen (for example, verifying "main" requires waiting for "printf" to be verified)