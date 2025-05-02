//! Semantic Shell

#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![feature(cfg_match)]

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    path::PathBuf,
    rc::Rc,
    sync::Mutex,
};

use clap::Parser;

mod builtins;
mod escapes;

/// sesh is a shell designed to be as semantic to use as possible
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {}

/// A single shell variable
#[derive(Clone, Debug, PartialEq, Eq)]
struct ShellVar {
    /// The name of it
    name: String,
    /// The value of it
    value: String,
}
/// A lot of [ShellVar]s.
type ShellVars = Vec<ShellVar>;

/// Whether a variable is local or not.
#[derive(Clone, Debug, PartialEq, Eq)]
enum VariableLocality {
    /// A local variable.
    Local,
    /// A nonlocal variable.
    Nonlocal,
}
/// A reference to a variable.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Variable {
    /// A local variable.
    Local(String),
    /// A nonlocal variable.
    Nonlocal(OsString),
}

/// The state of the shell
#[derive(Clone, Debug)]
struct State {
    /// Environment variables
    env: Rc<Mutex<std::env::VarsOs>>,
    /// Shell-local variables only accessible via builtins.
    shell_env: ShellVars,
    /// The focused variable.
    focus: Variable,
    /// The previous history of the states.
    history: Vec<State>,
    /// Current working directory.
    working_dir: PathBuf,
}

fn split_statement(statement: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut i: usize = 0;
    let mut in_str = (false, ' ');
    for ch in statement.chars() {
        if ['"', '\'', '`'].contains(&ch) {
            if in_str.0 && in_str.1 == ch {
                in_str.0 = false
            } else {
                in_str = (true, ch);
            }
            continue;
        }
        if !in_str.0 && ch == ' ' {
            i += 1;
            if i >= out.len() {
                out.push(String::new());
            }
            continue;
        }
        out[i].push(ch);
    }
    out.iter()
        .map(|v| v.trim().to_string())
        .collect::<Vec<String>>()
}

/// Evaluate a statement. May include multiple.
fn eval(statement: &str, state: &mut State) {
    let statement = escapes::interpret_escaped_string(statement);
    if statement.is_err() {
        println!("sesh: invalid escape: {}", statement.unwrap_err());
        return;
    }
    let statements = statement
        .unwrap()
        .split("\n")
        .map(|val| val.split(";").collect::<Vec<&str>>())
        .collect::<Vec<Vec<&str>>>()
        .iter()
        .map(|val| val.iter().map(|val| val.trim()).collect::<Vec<&str>>())
        .collect::<Vec<Vec<&str>>>()
        .concat()
        .iter()
        .map(|val| split_statement(val))
        .collect::<Vec<Vec<String>>>();

    for statement in statements {
        if statement.is_empty() || statement[0].is_empty() {
            continue;
        }
        if let Some(builtin) = builtins::BUILTINS.iter().find(|v| v.0 == statement[0]) {
            let status = builtin.1(statement, state);
            for (i, var) in state.shell_env.clone().into_iter().enumerate() {
                if var.name == "STATUS" {
                    state.shell_env.swap_remove(i);
                }
            }

            state.shell_env.push(ShellVar {
                name: "STATUS".to_string(),
                value: status.to_string(),
            });
            continue;
        }
        match std::process::Command::new(statement[0].clone())
            .args(&statement[1..])
            .current_dir(state.working_dir.clone())
            .spawn()
        {
            Ok(mut child) => {
                for (i, var) in state.shell_env.clone().into_iter().enumerate() {
                    if var.name == "STATUS" {
                        state.shell_env.swap_remove(i);
                    }
                }

                state.shell_env.push(ShellVar {
                    name: "STATUS".to_string(),
                    value: child.wait().unwrap().code().unwrap().to_string(),
                });
                continue;
            }
            Err(error) => {
                println!("sesh: error spawning program: {}", error);
                return;
            }
        }
    }

    state.env = Rc::new(Mutex::new(std::env::vars_os()));
    state.history.push(state.clone());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let interactive = true;
    let mut state = State {
        env: Rc::new(Mutex::new(std::env::vars_os())),
        shell_env: Vec::new(),
        focus: Variable::Local(String::new()),
        history: Vec::new(),
        working_dir: std::env::current_dir()
            .unwrap_or(std::env::home_dir().unwrap_or(PathBuf::from("/"))),
    };
    state.shell_env.push(ShellVar {
        name: "PROMPT".to_string(),
        value: "$u@$h $P> ".to_string(),
    });
    loop {
        let mut prompt = state
            .shell_env
            .iter()
            .find(|var| var.name == "PROMPT")
            .unwrap_or(&ShellVar {
                name: "PROMPT".to_string(),
                value: String::new(),
            })
            .value
            .clone();
        prompt = prompt.replace(
            "$u",
            &users::get_effective_username()
                .unwrap_or(users::get_current_username().unwrap_or("?".into()))
                .to_string_lossy(),
        );
        prompt = prompt.replace(
            "$h",
            &hostname::get().unwrap_or("?".into()).to_string_lossy(),
        );

        prompt = prompt.replace("$p", &state.working_dir.as_os_str().to_string_lossy());
        prompt = prompt.replace(
            "$P",
            &state
                .working_dir
                .file_name()
                .unwrap_or(OsStr::new("?"))
                .to_string_lossy(),
        );

        print!("{}", prompt);
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        eval(&input, &mut state);
    }
}
