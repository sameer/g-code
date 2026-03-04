use std::io::Read;

use codespan_reporting::term::{
    emit_to_io_write,
    termcolor::{ColorChoice, StandardStream},
};
use g_code::parse::file_parser;

fn main() {
    let filename = std::env::args().skip(1).next().expect("specify a filename");

    let gcode: String = match filename.as_ref() {
        "-" => {
            let mut acc = String::default();
            std::io::stdin().read_to_string(&mut acc).unwrap();
            acc
        }
        filename => std::fs::read_to_string(&filename).expect("file isn't readable"),
    };

    match file_parser(&gcode) {
        Ok(ast) => {
            println!("{:#?}", ast);
            eprintln!("Success!");
        }
        Err(err) => {
            let mut writer = StandardStream::stderr(ColorChoice::Auto);
            let config = codespan_reporting::term::Config::default();
            emit_to_io_write(
                &mut writer,
                &config,
                &codespan_reporting::files::SimpleFile::new(
                    if filename == "-" {
                        "<stdin>"
                    } else {
                        filename.as_str()
                    },
                    &gcode,
                ),
                &g_code::parse::into_diagnostic(&err),
            )
            .unwrap();
        }
    }
}
