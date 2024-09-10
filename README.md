[![crates.io](https://img.shields.io/crates/v/moon_script.svg)](https://crates.io/crates/moon_script)
[![docs.rs](https://img.shields.io/docsrs/moon_script)](https://docs.rs/moon_script/latest/moon_script/)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/JorgeRicoVivas/moon_script/rust.yml)](https://github.com/JorgeRicoVivas/moon_script/actions)
[![GitHub last commit](https://img.shields.io/github/last-commit/JorgeRicoVivas/moon_script)](https://github.com/JorgeRicoVivas/moon_script)
[![GitHub License](https://img.shields.io/github/license/JorgeRicoVivas/moon_script)](https://github.com/JorgeRicoVivas/moon_script?tab=CC0-1.0-1-ov-file)

MoonScript is a very basic scripting language for simple scripting with some syntax based on
Rust's, the idea of MoonScript it's for those writing MoonScript to find themselves scripts in
the simplest manner possible while still boosting performance.

If you want a tour on MoonScript, feel free to check the
[web book](https://jorgericovivas.github.io/moon_script_book/) out!

## Features
- std (Default): MoonScript will target the Standard library, implementing the Error trait on
error types and using Sync with std::sync mechanisms where possible.
- colorization (Default): Parsing errors will get colorized when printing them in the terminal.
- medium_functions: Functions added to an Engine can be up to 16 parameters, instead of 8.
- big_functions: Functions added to an Engine can be up to 24 parameters, instead of 8.
- massive_functions: Functions added to an Engine can be up to 40 parameters, instead of 8.