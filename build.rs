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
            bold("sesh"),
            roman("is a shell designed to be as semantic to use as possible"),
        ])
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
