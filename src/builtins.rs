//! builtins to sesh
#![allow(clippy::type_complexity)]

/// List of builtins
pub const BUILTINS: [(&str, fn (args: Vec<String>, unsplit_args: String, state: &mut super::State) -> i32); 3] = [("cd", cd), ("exit", exit), ("echo", echo)];

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
    unsplit_args = unsplit_args[5..].to_string();
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
