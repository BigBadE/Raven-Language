# Building

Requires LLVM set with the environmental variable LLVM_SYS_130_PREFIX = (path to folder with bin)

Please download LLVM from https://github.com/PLC-lang/llvm-package-windows/releases/tag/v13.0.0

C++ is also required to be installed somewhere on your system, for Windows get it from https://visualstudio.microsoft.com/vs/community/

Nightly is required for building the compiler

# Running

```cargo run -- "--root ../../lib/core/src"```