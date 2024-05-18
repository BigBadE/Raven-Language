# !!! DO NOT DOWNLOAD RAVEN ANYWHERE WITH A SPACE IN THE PATH. EX: "C:/Program Files/Raven". IT WILL NOT WORK. I CANNOT FIX IT !!!

This project requires the latest Rust download. Download it using Rustup from https://www.rust-lang.org/tools/install

# Building

Run this in the lib/test folder (or whatever Raven project in the repository you want to run)
```cargo run --bin magpie```

That command will build and run Magpie in that folder, which will run the project there.

Magpie can also be passed individual files, for example you can run this from the root folder:
```cargo run --bin magpie lib/test/src/main.rv```

# Common Issues

## /usr/bin/ld: cannot find -lzstd: No such file or directory

On Ubuntu: Install zstd-dev

``sudo apt-get install zstd-dev``

On MacOS: For some reason, brew doesn't seem to set the link dir correctly.

(If brew isn't installed, install it)
Run ``brew install zstd``
Run ``brew info zstd``
Set the environmental variable ZSTD_LIB_DIR to the directory listed in the info command (plus /lib)

## /usr/bin/ld: cannot find -lz: No such file or directory

Install zlib1g-dev

``sudo apt-get install zlib1g-dev``

## Some nonsense linker error with a warning about a messed up rlib

This issue happens on windows when you try to compile with a difference compiler than the one used to compile LLVM.

Make sure you're using MSVC with Rust, and your MSVC version is the 2022 edition.