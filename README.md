# g-code

[![crates.io](https://img.shields.io/crates/v/g-code.svg)](https://crates.io/crates/g-code) [![g-code](https://docs.rs/g-code/badge.svg)](https://docs.rs/g-code) ![g-code](https://github.com/sameer/g-code/workflows/g-code/badge.svg) [![codecov](https://codecov.io/gh/sameer/g-code/branch/main/graph/badge.svg?token=BXZQBMCAMI)](https://codecov.io/gh/sameer/g-code)

A joint crate for g-code parsing and emission.

## Parsing

The parser is written in Rust using [peg](https://github.com/kevinmehall/rust-peg).

### Demo

```
cargo run --example parse ./tests/vandy_commodores_logo.gcode
```

Output: https://gist.github.com/sameer/5fe20dad6faa6329926df48b82e68581


## Emission

Basic primitives for g-code emission.

Supports formatting, checksum and line number generation.

### Demo

See [svg2gcode](https://github.com/sameer/svg2gcode).

## TODOs

### Parse
* [ ] g-code parameters 
* [ ] g-code infix notation

### Emit
* [ ] Remaining commonly-used commands (open an issue or create a PR if you need one that's missing)

## References

* https://www.reprap.org/wiki/G-code
* NIST Interpreter: https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=823374
* https://en.wikipedia.org/wiki/G-code
