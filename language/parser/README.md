# Parser

The parser is the first step of compilation, which is split into two major parts:
- Tokenizer: This folder breaks down the source file into a series of tokens, such as a variable being the token "VARIABLE" and a period being the token "PERIOD".
This allows easier syntax highlighting.
- Parser: This folder converts the tokens into a Syntax, and passes it along to the checker to continue compilation.

