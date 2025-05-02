//! Semantic Shell

#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![feature(cfg_match)]

use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
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

/// A single alias
#[derive(Clone, Debug, PartialEq, Eq)]
struct Alias {
    /// alias from
    name: String,
    /// to
    to: String
}
/// The state of the shell
#[derive(Clone, Debug)]
struct State {
    /// Environment variables
    env: Arc<Mutex<std::env::VarsOs>>,
    /// Shell-local variables only accessible via builtins.
    shell_env: ShellVars,
    /// The focused variable.
    focus: Variable,
    /// The previous history of the states.
    history: Vec<State>,
    /// Current working directory.
    working_dir: PathBuf,
    /// A list of aliases from name to actual
    aliases: Vec<Alias>
}

unsafe impl Sync for State {}
unsafe impl Send for State {}

/// Split a statement.
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

/// Removes comments from a statement
fn remove_comments(statement: &str) -> String {
    let mut out = String::new();
    let mut in_comment = false;
    for ch in statement.chars() {
        if in_comment {
            if ch == '\n' {
                out.push(ch);
                in_comment = false
            }
            continue;
        }
        if ch == '#' {
            in_comment = true;
            continue;
        }
        out.push(ch);
    }
    out
}

/// Split something into lines
fn split_lines(lines: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut i: usize = 0;
    let mut escape_line = false;
    for ch in lines.chars() {
        if ch == '\n' && !escape_line {
            i += 1;
            if i > out.len() {
                out.push(String::new());
            }
            continue;
        }
        if ch == '\\' {
            escape_line = true;
            continue;
        }
        out[i].push(ch);
    }
    out
}

/// Split a string into statements
fn split_statements(statement: &str) -> Vec<String> {
    split_lines(statement)
        .into_iter()
        .map(|val| {
            val.split(";")
                .map(|val| val.to_string())
                .collect::<Vec<String>>()
        })
        .collect::<Vec<Vec<String>>>()
        .iter()
        .map(|val| val.iter().map(|val| val.trim().to_string()).collect::<Vec<String>>())
        .collect::<Vec<Vec<String>>>()
        .concat()
}

#[allow(clippy::arc_with_non_send_sync)]
/// Evaluate a statement. May include multiple.
fn eval(statement: &str, state: &mut State) {
    let statement = remove_comments(statement);
    let statements = split_statements(&statement);

    for statement in statements {
        let mut statement_split = split_statement(&statement);
        if statement.is_empty() || statement_split[0].is_empty() {
            continue;
        }
        let mut program_name = statement_split[0].clone();

        for alias in &state.aliases {
            if program_name == alias.name {
                let to_split = split_statement(&alias.to);
                for (i, item) in to_split[1..].iter().enumerate() {
                    statement_split.insert(i+1, (*item).clone());
                }
                program_name = to_split[0].clone();
                continue;
            }
        }

        if let Some(builtin) = builtins::BUILTINS
            .iter()
            .find(|v| v.0 == program_name)
        {
            let status = builtin.1(statement_split, statement.to_string(), state);
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
        match std::process::Command::new(program_name.clone())
            .args(&statement_split[1..])
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

    state.env = Arc::new(Mutex::new(std::env::vars_os()));
    let s = state.clone();
    state.history.push(s);
}

/// Write the prompt to the screen.
fn write_prompt(state: State) -> Result<(), Box<dyn std::error::Error>> {
    let mut prompt = state
        .shell_env
        .iter()
        .find(|var| var.name == "PROMPT1")
        .unwrap_or(&ShellVar {
            name: "PROMPT1".to_string(),
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
    Ok(())
}

#[allow(clippy::arc_with_non_send_sync)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = State {
        env: Arc::new(Mutex::new(std::env::vars_os())),
        shell_env: Vec::new(),
        focus: Variable::Local(String::new()),
        history: Vec::new(),
        working_dir: std::env::current_dir()
            .unwrap_or(std::env::home_dir().unwrap_or(PathBuf::from("/"))),
        aliases: Vec::new(),
    };
    state.shell_env.push(ShellVar {
        name: "PROMPT1".to_string(),
        value: "\x1b[32m$u@$h\x1b[39m \x1b[34m$P\x1b[39m> ".to_string(),
    });
    state.shell_env.push(ShellVar {
        name: "PROMPT2".to_string(),
        value: "> ".to_string(),
    });

    let ctrlc_cont = Arc::new(RwLock::new(false));
    let cc2 = ctrlc_cont.clone();

    ctrlc::set_handler(move || {
        (*cc2.write().unwrap()) = true;
    })
    .expect("Error setting Ctrl-C handler");
    'mainloop: loop {
        write_prompt(state.clone())?;

        let mut input = String::new();

        let mut i0 = [0u8];
        let mut line_escape = false;
        while i0[0] != b'\n' || line_escape {
            if i0[0] == b'\n' {
                let prompt2 = state
                    .shell_env
                    .iter()
                    .find(|var| var.name == "PROMPT2")
                    .unwrap_or(&ShellVar {
                        name: "PROMPT2".to_string(),
                        value: String::new(),
                    })
                    .value
                    .clone();
                print!("{}", prompt2);
                std::io::stdout().flush()?;
            }
            if ctrlc_cont.read().unwrap().to_owned() {
                input.clear();
                (*ctrlc_cont.write().unwrap()) = false;
                println!();
                continue 'mainloop;
            }
            let amount = std::io::stdin().read(&mut i0).unwrap();
            if amount == 0 {
                continue;
            }
            if i0[0] != b'\n' {
                line_escape = false;
            }
            if i0[0] == b'\\' {
                line_escape = true;
            }
            input.push(char::from_u32(i0[0] as u32).unwrap());
        }

        eval(&input, &mut state);
    }
}
