//! builtins to sesh
#![allow(clippy::type_complexity)]

/// List of builtins
pub const BUILTINS: [(&str, fn (args: Vec<String>, state: &mut super::State) -> i32); 2] = [("cd", cd), ("exit", exit)];

/// Change the directory
pub fn cd(args: Vec<String>, state: &mut super::State) -> i32 {
    if args[1] == ".." {
        state.working_dir.pop();
        return 0;
    }
    state.working_dir.push(args[1].clone());
    0
}

/// Exit the shell
pub fn exit(_: Vec<String>, _: &mut super::State) -> i32 {
    std::process::exit(0);
}
