# gcode-lalrpop

GCode parser written in Rust using the [LALRPOP](https://github.com/lalrpop/lalrpop/) parser generator.

![gcode-lalrpop](https://github.com/sameer/gcode-lalrpop/workflows/gcode-lalrpop/badge.svg) [![codecov](https://codecov.io/gh/sameer/gcode-lalrpop/branch/main/graph/badge.svg?token=BXZQBMCAMI)](https://codecov.io/gh/sameer/gcode-lalrpop)

A [custom lexer](https://lalrpop.github.io/lalrpop/lexer_tutorial/002_writing_custom_lexer.html) is used as LALRPOP currently does not handle whitespace well. This would prevent proper checksum computation.

Demo:

```
cargo run --example parse ./tests/vandy_commodores_logo.gcode
```

Output: https://gist.github.com/sameer/5fe20dad6faa6329926df48b82e68581

## TODOs

* Higher level AST


## References

* https://www.reprap.org/wiki/G-code
* NIST Interpreter: https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=823374
* https://en.wikipedia.org/wiki/G-code
