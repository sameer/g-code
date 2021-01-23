# gcode-lalrpop

GCode parser written in Rust using the LALRPOP parser generator.

![gcode-lalrpop](https://github.com/sameer/gcode-lalrpop/workflows/gcode-lalrpop/badge.svg) [![codecov](https://codecov.io/gh/sameer/gcode-lalrpop/branch/master/graph/badge.svg)](https://codecov.io/gh/sameer/gcode-lalrpop)

A handwritten lexer is used as LALRPOP currently does not handle whitespace well. This would prevent parsing comments into the AST.

Demo:

```
cargo run ./tests/vandy_commodores_logo.gcode
```

Output: https://gist.github.com/sameer/5fe20dad6faa6329926df48b82e68581

## TODOs

* Higher level AST
