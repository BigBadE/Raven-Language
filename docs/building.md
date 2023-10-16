# !!! DO NOT DOWNLOAD RAVEN ANYWHERE WITH A SPACE IN THE PATH. EX: "C:/Program Files/Raven". IT WILL NOT WORK. I CANNOT FIX IT !!!

This project requires the latest Rust download. Download it using Rustup from https://www.rust-lang.org/tools/install

# Building

Requires LLVM set with the environmental variable LLVM_SYS_130_PREFIX = (path to folder with bin)

Please download LLVM from https://github.com/PLC-lang/llvm-package-windows/releases/tag/v13.0.0

C++ is also required to be installed somewhere on your system, for Windows get it from https://visualstudio.microsoft.com/vs/community/.
By default, Rustup should install this for you.

Nightly is required for building the compiler, you can set the project to nightly with ```rustup override set nightly```

# Running

Run this in the lib/test folder (or whatever Raven project in the repository you want to run)
```cargo run --bin magpie```

That command will build and run Magpie in that folder, which will run the project there.

Magpie can also be passed individual files, for example you can run this from the root folder:
```cargo run --bin magpie lib/test/src/main.rv```