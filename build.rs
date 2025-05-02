use roff::{Roff, bold, italic, roman};

fn main() {
    let page = Roff::new()
        .control("TH", ["SESH", "1"])
        .control("SH", ["NAME"])
        .text([roman("sesh - Semantic Shell")])
        .control("SH", ["SYNOPSIS"])
        .text([
            bold("sesh"),
            roman(" [options]"),
        ])
        .control("SH", ["DESCRIPTION"])
        .text([
            bold("sesh"),
            roman("is a shell designed to be as semantic to use as possible"),
        ])
        .control("SH", ["OPTIONS"])
        .control("TP", [])
        .text([
            bold("-n"),
            roman(", "),
            bold("--bits"),
            roman("="),
            italic("BITS"),
        ])
        .text([roman(
            "Set the number of bits to modify. Default is one bit.",
        )])
        .render();
    print!("{}", page);
}
