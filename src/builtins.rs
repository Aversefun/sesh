//! builtins to sesh
#![allow(clippy::type_complexity)]

use std::hint::unreachable_unchecked;

/// List of builtins
pub const BUILTINS: [(
    &str,
    fn(args: Vec<String>, unsplit_args: String, state: &mut super::State) -> i32,
    &str,
    &str,
); 20] = [
    (
        "cd",
        cd,
        "[dir]",
        "Change the current directory into the specified one. If unspecified, change the directory into the user's home directory.",
    ),
    ("exit", exit, "", "Exit the shell."),
    (
        "echo",
        echo,
        "[-e] [text ...]",
        "Output the specified text. If -e is passed, parse escape characters.",
    ),
    (
        "alias",
        alias,
        "name=value [name=value ...]",
        "Create one or more command aliases. Command line arguments may be passed to the value.",
    ),
    (
        "help",
        help,
        "[command]",
        "Hey, that's me! Get help on a specified builtin or without arguments list all of the available builtin commands.",
    ),
    (
        "source",
        eval,
        "filename [arguments]",
        "Evaluate the contents of a file, optionally passing arguments in variables $1 and up.",
    ),
    (
        "loadf",
        loadf,
        "filename [...]",
        "Load the contents of a file into the focus.",
    ),
    (
        "splitf",
        splitf,
        "[character] [-e]",
        "Split the contents of the focus. If -e is passed, parse escapes.",
    ),
    (
        "set",
        set,
        "name=value [name=value ...]",
        "Set one or more variables to values.",
    ),
    ("dumpvars", dumpvars, "", "List all variables."),
    (
        "unset",
        unset,
        "var [var ...]",
        "Unset one or more variables.",
    ),
    (
        "copyf",
        copyf,
        "",
        "Copy the contents of the focus to your clipboard.",
    ),
    (
        "pastef",
        pastef,
        "",
        "Paste the contents of your clipboard into the focus.",
    ),
    (
        "setf",
        setf,
        "var [var ...]",
        "Set one or more variables to the contents of the focus.",
    ),
    (
        "getf",
        getf,
        "var",
        "Set the focus to the contents of a variable.",
    ),
    ("()", nop, "", "Do nothing and return a status code of 0."),
    ("nop", nop, "", "Do nothing and return a status code of 0."),
    (
        "if",
        _if,
        "condition (statement) [ (else_statement) ]",
        "If [condition] returns a status of 0, do (statement). Else, do (else_statement).",
    ),
    (
        "while",
        _while,
        "condition (statement)",
        "While [condition] returns a status of 0, do (statement).",
    ),
    (
        "gay",
        gay,
        "",
        "Change the colors of the terminal to cycle through the pride flag colors!",
    ),
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
pub fn exit(_: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if let Some(raw_term) = state.raw_term.clone() {
        let writer = raw_term.write().unwrap();
        let _ = writer.suspend_raw_mode();
        state.raw_term = None;
    }
    std::process::exit(0);
}

/// Echo a string
pub fn echo(args: Vec<String>, mut unsplit_args: String, _: &mut super::State) -> i32 {
    if args.len() == 1 {
        println!();
        return 0;
    }
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
pub fn help(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() >= 2 {
        for builtin in BUILTINS {
            if builtin.0 == args[1] {
                println!("{} {}: {}", builtin.0, builtin.2, builtin.3);
            }
        }
        return 0;
    }
    println!(
        "{}sesh, version {} ({})",
        if state.in_mode { "\x1b[31;1m" } else { "" },
        env!("CARGO_PKG_VERSION"),
        env!("TARGET")
    );
    println!(
        "{}This provides a list of built-in shell commands.",
        if state.in_mode {
            "\x1b[38;2;255;165;0m"
        } else {
            ""
        }
    );
    println!(
        "{}Use `man sesh` to find out more about the shell in general.",
        if state.in_mode { "\x1b[33;1m" } else { "" }
    );
    println!(
        "{}Use `man -k' or `info' to find out more about commands not in this list.",
        if state.in_mode { "\x1b[32;1m" } else { "" }
    );
    println!();
    let mut builtins = BUILTINS;
    builtins.sort_by(|v1, v2| v1.0.cmp(v2.0));

    for (i, builtin) in builtins.iter().enumerate() {
        if builtin.0 == "gay" {
            continue;
        }
        if state.in_mode {
            let table = [
                "\x1b[34;1m",
                "\x1b[36;1m",
                "\x1b[35;1m",
                "\x1b[31;1m",
                "\x1b[38;2;255;165;0m",
                "\x1b[33;1m",
                "\x1b[32;1m",
            ];
            let idx = i % table.len();
            print!("{}", table[idx]);
        }
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

    for (i, arg) in args[1..].iter().enumerate() {
        state2.shell_env.push(super::ShellVar {
            name: format!("{}", i),
            value: arg.clone(),
        });
    }

    super::eval(&file, &mut state2);

    0
}

/// Load a file into the focused variable.
pub fn loadf(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 2 {
        println!("sesh: {}: filename argument required", args[0]);
        println!("sesh: {0}: usage: {0} filename", args[0]);
        return 1;
    }
    let path = args[1..].concat().clone();

    let file = std::fs::read(path);
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

    state.focus = super::Focus::Str(file);

    0
}

/// Split the focus on a character.
pub fn splitf(mut args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() >= 3 && args[2] == "-e" {
        let unescaped = super::escapes::interpret_escaped_string(&args[1]);
        if unescaped.is_err() {
            println!("sesh: splitf: invalid escape: {}", unescaped.unwrap_err());
            return 1;
        }
        args[1] = unescaped.unwrap();
    }
    let split = args.get(1).unwrap_or(&" ".to_string()).clone();

    fn split_into(focus: super::Focus, split: String) -> super::Focus {
        match focus {
            super::Focus::Str(s) => super::Focus::Vec(
                s.split(&split)
                    .map(|v| super::Focus::Str(v.to_string()))
                    .collect::<Vec<super::Focus>>(),
            ),
            super::Focus::Vec(v) => super::Focus::Vec(
                v.iter()
                    .map(|v| split_into(v.clone(), split.clone()))
                    .collect::<Vec<super::Focus>>(),
            ),
        }
    }

    state.focus = split_into(state.focus.clone(), split);

    0
}

/// Set variable(s)
pub fn set(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 2 {
        println!("sesh: {}: at least one variable required", args[0]);
        println!("sesh: {0}: usage: {0} name=value [name=value ...]", args[0]);
        return 1;
    }
    for var in &args[1..] {
        let split = var.split_once("=");
        if split.is_none() {
            println!("sesh: {}: var=name pairs required", args[0]);
            println!("sesh: {0}: usage: {0} name=value [name=value ...]", args[0]);
            return 2;
        }
        let (name, value) = split.unwrap();
        state.shell_env.push(super::ShellVar {
            name: name.to_string(),
            value: value.to_string(),
        });
    }

    0
}

/// Dump all variables.
pub fn dumpvars(_: Vec<String>, _: String, state: &mut super::State) -> i32 {
    for super::ShellVar { name, value } in &state.shell_env {
        println!("{}: \"{}\"", name, value);
    }
    0
}

/// Unset variable(s)
pub fn unset(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 2 {
        println!("sesh: {}: at least one variable required", args[0]);
        println!("sesh: {0}: usage: {0} name [name ...]", args[0]);
        return 1;
    }
    for (i, ele) in state.shell_env.clone().into_iter().enumerate() {
        if args[1..].contains(&ele.name) {
            state.shell_env.remove(i);
        }
    }

    0
}

/// Copy the focus to the clipboard.
pub fn copyf(_: Vec<String>, _: String, state: &mut super::State) -> i32 {
    let mut clipboard = arboard::Clipboard::new().unwrap();
    clipboard
        .set_text(match &state.focus {
            super::Focus::Str(s) => s.clone(),
            super::Focus::Vec(_) => format!("{}", state.focus),
        })
        .unwrap();
    0
}

/// Paste from the clipboard into the focus.
pub fn pastef(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    let mut clipboard = arboard::Clipboard::new().unwrap();
    let text = clipboard.get_text();
    if let Err(e) = text {
        println!("sesh: {}: get clipboard text error: {}", args[0], e);
        1
    } else if let Ok(text) = text {
        state.focus = super::Focus::Str(text);
        0
    } else {
        unsafe {
            unreachable_unchecked();
        }
    }
}

/// Set a variable to the contents of the focus.
pub fn setf(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 2 {
        println!("sesh: {}: at least one variable required", args[0]);
        println!("sesh: {0}: usage: {0} var [var ...]", args[0]);
        return 1;
    }
    for var in &args[1..] {
        state.shell_env.push(super::ShellVar {
            name: var.to_string(),
            value: match &state.focus {
                super::Focus::Str(s) => s.clone(),
                super::Focus::Vec(_) => format!("{}", state.focus),
            },
        });
    }
    0
}

/// Set the focus to the contents of a variable
pub fn getf(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() != 2 {
        println!("sesh: {}: exactly one variable required", args[0]);
        println!("sesh: {0}: usage: {0} var", args[0]);
        return 1;
    }
    let mut val = String::new();
    for var in &state.shell_env {
        if var.name == args[1].clone() {
            val = var.value.clone();
            break;
        }
    }
    state.focus = super::Focus::Str(val);
    0
}

/// Empty function that does nothing. Mainly used for benchmarking evaluating.
pub fn nop(_: Vec<String>, _: String, _: &mut super::State) -> i32 {
    0
}

/// if statement
pub fn _if(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 3 {
        println!(
            "sesh: {0}: usage: {0} condition (statement) [ (else_statement) ]",
            args[0]
        );
        return 1;
    }
    super::eval(&args[1].clone(), state);
    state.shell_env.reverse();
    let mut status = 0i32;
    for var in &state.shell_env {
        if var.name == "STATUS" {
            status = var.value.parse().unwrap();
        }
    }
    state.shell_env.sort_by(|v1, v2| v1.name.cmp(&v2.name));
    if status == 0 {
        super::eval(&args[2].clone(), state);
    } else if args.len() == 8 {
        super::eval(&args[3].clone(), state);
    }

    0
}

/// loop while a condition is true
pub fn _while(args: Vec<String>, _: String, state: &mut super::State) -> i32 {
    if args.len() < 3 {
        println!("sesh: {0}: usage: {0} condition (statement)", args[0]);
        return 1;
    }

    fn test(condition: String, state: &mut super::State) -> bool {
        super::eval(&condition, state);
        state.shell_env.reverse();
        let mut status = 0i32;
        for var in &state.shell_env {
            if var.name == "STATUS" {
                status = var.value.parse().unwrap();
            }
        }
        state.shell_env.sort_by(|v1, v2| v1.name.cmp(&v2.name));
        status == 0
    }

    while test(args[1].clone(), state) {
        super::eval(&args[2].clone(), state);
    }

    0
}

/// shh
pub fn gay(_: Vec<String>, _: String, state: &mut super::State) -> i32 {
    state.in_mode = true;
    0
}
