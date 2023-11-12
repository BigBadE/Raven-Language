# Core
The core is the base compiler of Raven (written in Rust), most work is fixing issues with the language itself and adding new features.

This is in the language folder of the base repository

# Magpie
Magpie is Raven's build tool, dependency manager, and version control. It is currently lacking most functionality.

This is in the tools/magpie folder of the base repository

# IDE Extensions / Raven Language Server
This provides support to various IDEs such as VSCode and implementing features like syntax highlighting. It's a mix of Rust (Raven Language Server), NodeJS (VSCode), and Java (IntelliJ)

This is in the tools/ide-plugins folder of the base repository

# Website
This is the main page for Raven.

The source is in the website folder of the base repository The website is on the gh-pages branch of the base repository

# Standard Library
This is the standard library of Raven for every platform it supports. It's divided into a platform-specific std, and a universal std, which calls the platform std.

This is in the lib folder of the base repository