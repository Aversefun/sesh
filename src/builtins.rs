//! builtins to sesh
#![allow(clippy::type_complexity)]

use std::sync::{Arc, Mutex};

/// List of builtins
pub const BUILTINS: [(
    &str,
    fn(args: Vec<String>, unsplit_args: String, state: &mut super::State) -> i32,
    &str,
); 6] = [
    ("cd", cd, "[dir]"),
    ("exit", exit, ""),
    ("echo", echo, "[-e] [text ...]"),
    ("alias", alias, "[name] [value]"),
    ("help", help, ""),
    ("source", eval, "filename [arguments]"),
];

/// Change the directory
pub fn cd(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() == 1 {
        state.working_dir = std::env::home_dir().unwrap();
        return 0;
    }
    if args[1] == ".." {
        state.working_dir.pop();
        return 0;
    }
    state.working_dir.push(args[1].clone());
    0
}

/// Exit the shell
pub fn exit(_: Vec<String>, _: String, _: &mut super::State) -> i32 {
    std::process::exit(0);
}

/// Echo a string
pub fn echo(args: Vec<String>, mut unsplit_args: String, _: &mut super::State) -> i32 {
    unsplit_args = unsplit_args[(args[0].len() + 1)..].to_string();
    if args.len() != 1 && args[1] == "-e" {
        unsplit_args = unsplit_args[3..].to_string();
        let escaped = crate::escapes::interpret_escaped_string(&unsplit_args);
        if escaped.is_err() {
            println!("sesh: echo: invalid escape: {}", escaped.unwrap_err());
            return 1;
        }
        unsplit_args = escaped.unwrap();
    }
    println!("{}", unsplit_args);
    0
}

/// Add an alias
pub fn alias(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() == 1 {
        for alias in &state.aliases {
            println!("`{}`: `{}`", alias.name, alias.to);
        }
        return 0;
    }
    if args.len() == 2 {
        for alias in &state.aliases {
            if alias.name != args[1] {
                continue;
            }
            println!("`{}`: `{}`", alias.name, alias.to);
        }
        return 0;
    }

    state.aliases.push(super::Alias {
        name: args[1].clone(),
        to: args[2].clone(),
    });

    0
}

/// Output help on builtins.
pub fn help(_: Vec<String>, _: String, _: &mut super::State) -> i32 {
    println!(
        "sesh, version {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("TARGET")
    );
    println!("This provides a list of built-in shell commands.");
    println!("Use `man sesh` to find out more about the shell in general.");
    println!("Use `man -k' or `info' to find out more about commands not in this list.");
    println!();
    let mut builtins = BUILTINS.clone();
    builtins.sort_by(|v1, v2| v1.0.cmp(v2.0));

    for builtin in builtins {
        println!("{} {}", builtin.0, builtin.2);
    }
    0
}

/// Run a file.
pub fn eval(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 2 {
        println!("sesh: {}: filename argument required", args[0]);
        println!("sesh: {0}: usage: {0} filename [arguments]", args[0]);
        return 1;
    }

    let file = std::fs::read(args[1].clone());
    if file.is_err() {
        println!(
            "sesh: {}: error opening file: {}",
            args[0],
            file.unwrap_err()
        );
        return 2;
    }
    let file = String::from_utf8(file.unwrap());
    if file.is_err() {
        println!("sesh: {}: invalid UTF-8: {}", args[0], file.unwrap_err());
        return 3;
    }
    let file = file.unwrap();

    let mut state2 = state.clone();

    let mut i = 0usize;
    for arg in &args[1..] {
        state2.shell_env.push(super::ShellVar {
            name: format!("{}", i),
            value: arg.clone(),
        });
        i += 1;
    }

    super::eval(&file, &mut state2);

    0
}
