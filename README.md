# gcode-lalrpop

A GCode parser written in Rust using the LALRPOP parser generator.

A handwritten lexer is used as LALRPOP currently does not handle whitespace well. This would prevent parsing comments into the AST.