# Raven

Raven is an experimental language bringing modern features to every platform.

Raven's goal is to allow one codebase to run on every platform, without having to sacrifice speed or usability.

Currently, Raven mainly targets desktop applications, but web, mobile, and JVM targets are planned.

# !!! DO NOT DOWNLOAD RAVEN ANYWHERE WITH A SPACE IN THE PATH. EX: "C:/Program Files/Raven". IT WILL NOT WORK. I CANNOT FIX IT !!!

# Building

Requires LLVM set with the environmental variable LLVM_SYS_130_PREFIX = (path to folder with bin)

Please download LLVM from https://github.com/PLC-lang/llvm-package-windows/releases/tag/v13.0.0

C++ is also required to be installed somewhere on your system, for Windows get it from https://visualstudio.microsoft.com/vs/community/

Nightly is required for building the compiler, you can set the project to nightly with ```rustup override set nightly```

# Running

```cargo run --bin cli -- "--root lib/test/src"```

That command will build and run Raven's CLI with the following options
