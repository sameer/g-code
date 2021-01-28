use codespan_reporting::term::{
    emit,
    termcolor::{ColorChoice, StandardStream},
};

use gcode_lalrpop::lexer;
use gcode_lalrpop::parser::FileParser;

fn main() {
    let filename = std::env::args().skip(1).next().expect("specify a filename");

    let gcode: String = std::fs::read_to_string(&filename).expect("file isn't readable");

    match FileParser::new().parse(&gcode, lexer::Lexer::new(&gcode)) {
        Ok(ast) => {
            eprintln!("Success!");
            println!("{:#?}", ast);
        }
        Err(err) => {
            let mut writer = StandardStream::stderr(ColorChoice::Auto);
            let config = codespan_reporting::term::Config::default();
            emit(
                &mut writer,
                &config,
                &codespan_reporting::files::SimpleFile::new(filename, &gcode),
                &gcode_lalrpop::into_diagnostic(&err),
            )
            .unwrap();
        }
    }
}
