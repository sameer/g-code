use std::{
    env::args,
    io::{copy, stdin, stdout},
    process::exit,
};

use icewrap::{
    decode::{dyn_decoder_builder, PullDecoder, PushDecoder},
    encode::{dyn_encoder_builder, PullEncoder},
};

fn main() {
    let mut it = args().skip(1);
    let mode = it.next();

    match mode.as_deref() {
        Some("encode") => {
            let window = it
                .next()
                .and_then(|w| w.parse().ok())
                .expect("valid window");
            let lookahead = it
                .next()
                .and_then(|l| l.parse().ok())
                .expect("valid lookahead");
            let indexed = it.next().and_then(|i| i.parse().ok()).unwrap_or(true);

            let encoder =
                dyn_encoder_builder(window, lookahead, indexed).expect("valid encoder params")();
            let mut pull_encoder = PullEncoder::new(encoder, stdin());

            copy(&mut pull_encoder, &mut stdout()).expect("I/O succeeds");
            // Flush any remaining data
            copy(&mut pull_encoder.finish(), &mut stdout()).expect("I/O succeeds");
        }
        Some("decode") => {
            let window = it
                .next()
                .and_then(|w| w.parse().ok())
                .expect("valid window");
            let lookahead = it
                .next()
                .and_then(|l| l.parse().ok())
                .expect("valid lookahead");

            let mut decoder =
                dyn_decoder_builder(window, lookahead).expect("valid decoder params")();
            let mut pull_decoder = PullDecoder::new(&mut decoder, stdin());

            copy(&mut pull_decoder, &mut stdout()).expect("I/O succeeds");

            if !decoder.is_finished() {
                eprintln!("Data may be invalid, decoder did not finish!")
            }
        }
        Some("passthrough") => {
            let window = it
                .next()
                .and_then(|w| w.parse().ok())
                .expect("valid window");
            let lookahead = it
                .next()
                .and_then(|l| l.parse().ok())
                .expect("valid lookahead");
            let indexed = it.next().and_then(|i| i.parse().ok()).unwrap_or(true);

            let encoder =
                dyn_encoder_builder(window, lookahead, indexed).expect("valid encoder params")();
            let mut pull_encoder = PullEncoder::new(encoder, stdin());

            let mut decoder =
                dyn_decoder_builder(window, lookahead).expect("valid decoder params")();
            let mut push_decoder = PushDecoder::new(&mut decoder, stdout());

            copy(&mut pull_encoder, &mut push_decoder).expect("I/O succeeds");
            // Flush any remaining data
            copy(&mut pull_encoder.finish(), &mut push_decoder).expect("I/O succeeds");

            assert!(
                decoder.is_finished(),
                "Data may be invalid, decoder did not finish!"
            );
        }
        Some("help") | None => {
            eprintln!(
                "Usage:\n\
                cli [COMMAND]\n\
                \n\
                Commands:\n\
                \tencode [window] [lookahead] [indexed]\t\tencodes stdin to stdout\n\
                \tdecode [window] [lookahead]\t\t\tdecodes stdin to stdout\n\
                \tpassthrough [window] [lookahead] [indexed]\tpipes stdin through encoding and decoding to stdout\n\
                \thelp\n\
            "
            )
        }
        Some(_) => {
            eprintln!("Unknown command");
            exit(1);
        }
    }
}
