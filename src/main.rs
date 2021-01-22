fn main() {
    use gcode_lalrpop::lexer;
    use gcode_lalrpop::parser::FileParser;

    let filename = std::env::args().skip(1).next().expect("specify a filename");

    let gcode: String = std::fs::read_to_string(filename).expect("file isn't readable");
    eprintln!(
        "{:#?}",
        FileParser::new().parse(&gcode, lexer::Lexer::new(&gcode))
    );
}
