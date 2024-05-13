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

Install zstd

``sudo apt-get install zstd``

## /usr/bin/ld: cannot find -lz: No such file or directory

Install zlib1g-dev

``sudo apt-get install zlib1g-dev``
