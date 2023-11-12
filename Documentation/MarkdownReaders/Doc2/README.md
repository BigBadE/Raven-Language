# Installation

The best way to install Raven without building from source is through Magpie.

Raven comes with a helper program called "Magpie Wrapper" that will automatically keep the language up to date.

To install the latest Magpie or Magpie Wrapper, check out https://github.com/BigBadE/Raven-Language/releases/tag/nightly.

Follow the instructions on the release page to install the wrapper and run it.

# Running

Magpie can either run a project or a single file. 

Running a project requires following the project format, a commented example can be found in the lib/test folder.

Running a single file just requires a .rv file with a main function. This file will provide examples you can run by copying them into a file
and running it with Magpie.

An example of running a single file:
```magpie my_file.rv```

This will run the file named my_file.rv.

To confirm your installation is working, try running the following Hello World! example:

```
import stdio;

fn main() {
    printf("Hello World!");
}
```

For more information over how this example works, check out [Chapter 2: Getting Started](2_getting_started.md)