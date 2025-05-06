#![allow(unused_imports)]
use roff::{Roff, bold, italic, roman};
use std::{env, path::PathBuf};

fn main() {
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );

    let page = Roff::new()
        .control("TH", ["SESH", "1"])
        .control("SH", ["NAME"])
        .text([roman("sesh - Semantic Shell")])
        .control("SH", ["SYNOPSIS"])
        .text([bold("sesh"), roman(" [options]")])
        .control("SH", ["DESCRIPTION"])
        .text([
            bold("Sesh"),
            roman(
                " is a shell designed to be as semantic to use as possible. It isn't completely compatible \
                with sh(yet) however the point is for it to be easily usable and understandable by humans. It can \
                interpret commands from standard input or from a file."
            ),
        ])
        .control("SH", ["OPTIONS"])
        .text([
            bold("-c, --run "), roman("\tIf this option is present, then commands are read from the \
            argument provided to it and executed in a non-interactive environment(a shell will not be opened after \
            they are done executing).\n")
        ])
        .text([
            bold("-b, --before"), roman("\tIf this option is present, then commands are read from the \
            argument provided to it and executed in an interactive environment(a shell WILL be opened after they \
            are done executing).\n")
        ])
        .control("SH", ["ARGUMENTS"])
        .text(
            [
                roman("If arguments remain after option processing and neither -c nor -b have been supplied, \
                the first argument is assumed to be the name of a shell file.")
            ]
        )
        .control("SH", ["FILES"])
        .text(
            [
                bold("Sesh"), roman(" reads from and writes to a couple of files depending on the circumstances:\n")
            ]
        )
        .text(
            [bold(".seshrc"), roman(" - Executed upon startup\n")]
        )
        .text(
            [bold(".sesh_history"), roman(" - Contains commands previously ran, one per line. \
                Read upon startup in an interactive shell and written to after each command.\n")]
        )
        .text(
            [bold("Other files"), roman(" - Scripts may write to files via other methods, \
            including outside tools. Scripts may be read from the path in the first argument of the shell after options.")]
        )
        .render();
    std::fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("sesh.1"),
        page,
    )
    .unwrap();
}
