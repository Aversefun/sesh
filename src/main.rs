//! Semantic Shell

#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![feature(cfg_match)]
#![feature(slice_concat_trait)]
#![feature(test)]
#![feature(let_chains)]

use std::{
    ffi::OsStr,
    fmt::Display,
    io::{Read, Write},
    os::fd::FromRawFd,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use clap::Parser;
use termion::raw::IntoRawMode;

mod builtins;
mod escapes;
#[cfg(test)]
mod tests;

/// sesh is a shell designed to be as semantic to use as possible
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Run an expression. This will not open an interactive shell. Takes precedence over --before
    #[arg(long="run", short='c', default_value_t=("".to_string()))]
    run_expr: String,
    /// Run an expression before opening an interactive shell.
    #[arg(long="before", short='b', default_value_t=("".to_string()))]
    run_before: String,
}

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

/// A single alias
#[derive(Clone, Debug, PartialEq, Eq)]
struct Alias {
    /// alias from
    name: String,
    /// to
    to: String,
}

/// A focus.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Focus {
    /// A string focus
    Str(String),
    /// A vec focus
    Vec(Vec<Focus>),
}

impl Display for Focus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Str(s) => {
                f.write_fmt(format_args!("str:\"{}\"", s.clone().replace("\n", "\\n")))?;
            }
            Self::Vec(v) => {
                f.write_fmt(format_args!(
                    "list:[{}]",
                    v.clone()
                        .iter()
                        .map(|v| format!("{}", v))
                        .collect::<Vec<String>>()
                        .join(", ")
                ))?;
            }
        }
        Ok(())
    }
}

/// The state of the shell
#[derive(Clone)]
struct State {
    /// Shell-local variables only accessible via builtins.
    shell_env: ShellVars,
    /// Current working directory.
    working_dir: PathBuf,
    /// A list of aliases from name to actual.
    aliases: Vec<Alias>,
    /// The focused variable
    focus: Focus,
    /// Raw terminal.
    raw_term: Option<Arc<RwLock<termion::raw::RawTerminal<std::io::Stdout>>>>,
    /// sh
    in_mode: bool,
    /// sh
    entries: usize,
    /// The history
    history: Vec<String>,
}

unsafe impl Sync for State {}
unsafe impl Send for State {}

/// Split a statement.
fn split_statement(statement: &str) -> Vec<Result<IndirectRes, &str>> {
    let mut out = vec![String::new()];
    let mut i = 0usize;
    let mut in_str = (false, ' ');
    let mut escape = false;
    let mut f = 0usize;
    let mut str_idx = usize::MAX;
    for ch in statement.chars() {
        if ch == '\\' && !in_str.0 {
            escape = true;
        }
        if in_str.0 && in_str.1 == ch {
            in_str.0 = false;
            if ch == ']' {
                out[i].push(ch);
            }
            if ch == ')' && f == str_idx + 1 {
                out[i].push('(');
                out[i].push(ch);
            }
            escape = false;
            f += 1;
            continue;
        }
        if !(!['"', '\'', '`', '(', '['].contains(&ch) || escape || in_str.0 || ch == '[' && f <= 1)
        {
            in_str = (true, ch);
            if ch == '(' {
                in_str.1 = ')';
            }
            if ch == '[' {
                in_str.1 = ']';
            }
            if ch == '[' {
                out[i].push(ch);
            }
            escape = false;
            str_idx = f;
            f += 1;
            continue;
        }
        if !in_str.0 && ch == ' ' {
            i += 1;
            if i >= out.len() {
                out.push(String::new());
            }
            escape = false;
            f += 1;
            continue;
        }
        out[i].push(ch);
        escape = false;
        f += 1;
    }
    out.iter()
        .map(|v| v.trim().to_string())
        .map(|v| is_indirect(v))
        .collect::<Vec<Result<IndirectRes, &str>>>()
}

/// An indirect to the value.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Indirect {
    /// default to
    #[default]
    Default,
    /// Redirect to stdout(not for stdin!)
    Stdout,
    /// Redirect to stderr(not for stdin!)
    Stderr,
    /// Redirect to/from a file descriptor
    Fd(i32),
    /// Redirect to/from a path
    Path(PathBuf),
    /// Redirect to the next statement
    NextStatement,
    /// Redirect from the previous statement
    PrevStatement,
}

/// A result from [is_indirect]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum IndirectRes {
    /// Isn't a indirect; the part
    Statement(String),
    /// TO stdin
    Stdin(Indirect),
    /// FROM stdout
    Stdout(Indirect),
    /// FROM stderr
    Stderr(Indirect),
}

impl IndirectRes {
    /// get the statement value or panic if not
    fn unwrap_statement(self) -> String {
        if let Self::Statement(v) = self {
            v
        } else {
            panic!("IndirectRes is not statement");
        }
    }

    /// return whether self is a statement
    fn is_statement(&self) -> bool {
        matches!(self, Self::Statement(_))
    }
}

/// Return whether a statement is a indirect pointer and if it is what to.
fn is_indirect(statement: String) -> Result<IndirectRes, &'static str> {
    fn is_indirect_inner(i: (&str, &str)) -> Indirect {
        if i.1.is_empty() {
            if i.0 == "0" {
                Indirect::PrevStatement
            } else {
                Indirect::NextStatement
            }
        } else if i.0 == "0" {
            if let Ok(n) = i.1.parse::<std::os::fd::RawFd>() {
                Indirect::Fd(n)
            } else {
                Indirect::Path(PathBuf::from(i.1))
            }
        } else {
            match i.1 {
                "1" => Indirect::Stdout,
                "2" => Indirect::Stderr,
                v => {
                    if let Ok(n) = v.parse::<std::os::fd::RawFd>() {
                        Indirect::Fd(n)
                    } else {
                        Indirect::Path(PathBuf::from(v))
                    }
                }
            }
        }
    }
    if let Some(i) = statement.split_once("@") {
        match i.0 {
            "0" => Ok(IndirectRes::Stdin(is_indirect_inner(i))),
            "1" => Ok(IndirectRes::Stdout(is_indirect_inner(i))),
            "2" => Ok(IndirectRes::Stderr(is_indirect_inner(i))),
            _ => Err("unknown indirect from"),
        }
    } else {
        Ok(IndirectRes::Statement(statement))
    }
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
            continue;
        }
        if ch == '\\' {
            escape_line = true;
        }
        while i >= out.len() {
            out.push(String::new());
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
        .map(|val| {
            val.iter()
                .map(|val| val.trim().to_string())
                .collect::<Vec<String>>()
        })
        .collect::<Vec<Vec<String>>>()
        .concat()
}

/// Substitute in shell variables
fn substitute_vars(statement: &str, state: State) -> String {
    let mut out = statement.to_string();
    for ShellVar { name, value } in state.shell_env {
        out = out.replace(&("$".to_owned() + &name), &value);
    }
    out = out.replace("!FOCUS", &format!("{}", state.focus));
    out
}

/// remove duplicates, keeping later ones
fn garbage_collect_vars(state: &mut State) {
    state.shell_env.reverse();
    let mut seen = vec![];
    let mut remove_indexes = vec![];
    let mut i = 0usize;
    for var in &mut state.shell_env {
        if seen.contains(&var.name) {
            remove_indexes.push(i);
            i += 1;
            continue;
        }
        seen.push(var.name.clone());
        i += 1;
    }
    for i in remove_indexes {
        state.shell_env.remove(i);
    }
    state.shell_env.sort_by(|v1, v2| v1.name.cmp(&v2.name));
}

#[allow(clippy::arc_with_non_send_sync)]
/// Evaluate a statement. May include multiple.
fn eval(statement: &str, state: &mut State) {
    let statement = remove_comments(statement);
    let statements = split_statements(&substitute_vars(&statement, state.clone()));

    for statement in statements {
        let statement_split = split_statement(&statement);
        if let Some(e) = statement_split.iter().find(|v| v.is_err()) {
            println!("sesh: {}\r", e.clone().unwrap_err());
            return;
        }
        let statement_split = statement_split
            .iter()
            .map(|v| v.clone().unwrap())
            .collect::<Vec<IndirectRes>>();

        if !statement_split[0].is_statement() {
            println!("sesh: program name is indirect\r");
            return;
        }

        let mut indirects = statement_split
            .clone()
            .into_iter()
            .filter(|v| !v.is_statement())
            .collect::<Vec<IndirectRes>>();
        indirects.sort_by(|v1, v2| {
            if matches!(v1, IndirectRes::Stderr(_)) && matches!(v2, IndirectRes::Stderr(_)) {
                return std::cmp::Ordering::Equal;
            }
            if matches!(v1, IndirectRes::Stdout(_)) && matches!(v2, IndirectRes::Stdout(_)) {
                return std::cmp::Ordering::Equal;
            }
            if matches!(v1, IndirectRes::Stdin(_)) && matches!(v2, IndirectRes::Stdin(_)) {
                return std::cmp::Ordering::Equal;
            }
            v1.cmp(v2)
        });
        indirects.dedup();

        let mut statement_split = statement_split
            .into_iter()
            .filter(|v| v.is_statement())
            .map(|v| v.unwrap_statement())
            .collect::<Vec<String>>();

        if statement.is_empty() || statement_split[0].is_empty() {
            continue;
        }
        let mut program_name = statement_split[0].clone();

        for alias in &state.aliases {
            if program_name == alias.name {
                let to_split = split_statement(&alias.to)
                    .iter()
                    .filter_map(|v| v.clone().ok())
                    .filter(|v| v.is_statement())
                    .map(|v| v.unwrap_statement())
                    .collect::<Vec<String>>();

                for (i, item) in to_split[1..].iter().enumerate() {
                    statement_split.insert(i + 1, (*item).clone());
                }
                program_name = to_split[0].clone();
                continue;
            }
        }

        if let Some(builtin) = builtins::BUILTINS.iter().find(|v| v.0 == program_name) {
            if let Some(raw_term) = state.raw_term.clone() {
                let writer = raw_term.write().unwrap();
                let _ = writer.suspend_raw_mode();
            }
            if indirects.len() > 1 {
                println!("sesh: warning: indirects ignored for builtin")
            }
            let status = builtin.1(statement_split, statement.to_string(), state);
            garbage_collect_vars(state);
            if let Some(raw_term) = state.raw_term.clone() {
                let writer = raw_term.write().unwrap();
                let _ = writer.activate_raw_mode();
            }
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
        if let Some(raw_term) = state.raw_term.clone() {
            let writer = raw_term.write().unwrap();
            let _ = writer.suspend_raw_mode();
        }
        for env in &state.shell_env {
            unsafe {
                std::env::set_var(env.name.clone(), env.value.clone());
            }
        }
        let mut command = std::process::Command::new(program_name.clone());
        command
            .args(&statement_split[1..])
            .current_dir(state.working_dir.clone());
        for indirect in indirects {
            match indirect {
                IndirectRes::Statement(_) => (),
                IndirectRes::Stderr(i) => match i {
                    Indirect::Default => (),
                    Indirect::Fd(fd) => {
                        command.stderr(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) });
                    }
                    Indirect::NextStatement => todo!(),
                    Indirect::Path(p) => {
                        command.stderr(
                            std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(p)
                                .unwrap(),
                        );
                    },
                    Indirect::PrevStatement => todo!(),
                    Indirect::Stderr => (),
                    Indirect::Stdout => {
                        command.stderr(std::io::stdout());
                    }
                },
                IndirectRes::Stdout(i) => match i {
                    Indirect::Default => (),
                    Indirect::Fd(fd) => {
                        command.stdout(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) });
                    }
                    Indirect::NextStatement => todo!(),
                    Indirect::Path(p) => {
                        command.stdout(
                            std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(p)
                                .unwrap(),
                        );
                    },
                    Indirect::PrevStatement => todo!(),
                    Indirect::Stderr => {
                        command.stdout(std::io::stderr());
                    },
                    Indirect::Stdout => ()
                },
                IndirectRes::Stdin(i) => match i {
                    Indirect::Default => (),
                    Indirect::Fd(fd) => {
                        command.stdin(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) });
                    }
                    Indirect::NextStatement => todo!(),
                    Indirect::Path(p) => {
                        command.stdin(
                            std::fs::OpenOptions::new()
                                .read(true)
                                .open(p)
                                .unwrap(),
                        );
                    },
                    Indirect::PrevStatement => todo!(),
                    Indirect::Stderr => (),
                    Indirect::Stdout => ()
                }
            }
        }

        match command.spawn() {
            Ok(mut child) => {
                for (i, var) in state.shell_env.clone().into_iter().enumerate() {
                    if var.name == "STATUS" {
                        state.shell_env.swap_remove(i);
                    }
                }

                state.shell_env.push(ShellVar {
                    name: "STATUS".to_string(),
                    value: child.wait().unwrap().code().unwrap_or(255i32).to_string(),
                });
                if let Some(raw_term) = state.raw_term.clone() {
                    let writer = raw_term.write().unwrap();
                    let _ = writer.activate_raw_mode();
                }
                continue;
            }
            Err(error) => {
                println!("sesh: error spawning program: {}", error);
                for (i, var) in state.shell_env.clone().into_iter().enumerate() {
                    if var.name == "STATUS" {
                        state.shell_env.swap_remove(i);
                    }
                }

                state.shell_env.push(ShellVar {
                    name: "STATUS".to_string(),
                    value: "127".to_string(),
                });
                if let Some(raw_term) = state.raw_term.clone() {
                    let writer = raw_term.write().unwrap();
                    let _ = writer.activate_raw_mode();
                }
                return;
            }
        }
    }
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
    if state.in_mode {
        let table = [
            "\x1b[31;1m",
            "\x1b[38;2;255;165;0;1m",
            "\x1b[33;1m",
            "\x1b[32;1m",
            "\x1b[34;1m",
            "\x1b[36;1m",
            "\x1b[35;1m",
        ];
        let idx = state.entries % table.len();
        prompt += table[idx];
    }

    print!("{}", prompt);
    std::io::stdout().flush()?;
    Ok(())
}

/// log data to a file
#[allow(dead_code)]
fn log_file(value: &str) {
    let value = value.to_string() + "\n";
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::current_dir().unwrap().join("sesh.log"))
        .unwrap()
        .write_all(value.as_bytes())
        .unwrap();
}

#[allow(clippy::arc_with_non_send_sync)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = Args::parse();

    let mut args = std::env::args();
    let _ = args.next();

    if let Some(filename) = args.next()
        && options.run_before.is_empty()
        && options.run_expr.is_empty()
    {
        let rc = std::fs::read(filename.clone());
        if rc.is_err() {
            println!("sesh: reading {} failed: {}", filename, rc.unwrap_err());
            println!("sesh: exiting");
            return Ok(());
        } else {
            let rc = String::from_utf8(rc.unwrap());
            if rc.is_err() {
                println!("sesh: reading {} failed: not valid UTF-8", filename);
                println!("sesh: exiting");
                return Ok(());
            } else {
                let rc = rc.unwrap();
                options.run_expr = rc;
            }
        }
    }

    let mut state = State {
        shell_env: Vec::new(),
        focus: Focus::Str(String::new()),
        working_dir: std::env::current_dir()
            .unwrap_or(std::env::home_dir().unwrap_or(PathBuf::from("/"))),
        aliases: Vec::new(),
        raw_term: None,
        in_mode: false,
        entries: 0,
        history: std::fs::read_to_string(std::env::home_dir().unwrap().join(".sesh_history"))
            .unwrap_or_default()
            .split("\n")
            .map(|v| v.trim_matches(|ch: char| ch.is_control()))
            .map(|v| v.to_string())
            .filter(|v| !v.is_empty())
            .collect(),
    };
    state.shell_env.push(ShellVar {
        name: "PROMPT1".to_string(),
        value: "\x1b[32m$u@$h\x1b[39m \x1b[34m$P\x1b[39m> ".to_string(),
    });
    state.shell_env.push(ShellVar {
        name: "PROMPT2".to_string(),
        value: "> ".to_string(),
    });

    let mut interactive = true;

    if !options.run_expr.is_empty() {
        interactive = false;
        state.shell_env.push(ShellVar {
            name: "INTERACTIVE".to_string(),
            value: "false".to_string(),
        });
    } else {
        state.shell_env.push(ShellVar {
            name: "INTERACTIVE".to_string(),
            value: "true".to_string(),
        });
    }
    let _ = ctrlc::set_handler(|| println!());

    let rc = std::fs::read(std::env::home_dir().unwrap().join(".seshrc"));
    if rc.is_err() {
        println!("sesh: reading ~/.seshrc failed: {}", rc.unwrap_err());
        println!("sesh: not running .seshrc")
    } else {
        let rc = String::from_utf8(rc.unwrap());
        if rc.is_err() {
            println!("sesh: reading ~/.seshrc failed: not valid UTF-8");
            println!("sesh: not running .seshrc")
        } else {
            let rc = rc.unwrap();
            eval(&rc, &mut state);
        }
    }

    if !interactive {
        eval(&options.run_expr, &mut state);
        return Ok(());
    } else if !options.run_before.is_empty() {
        eval(&options.run_before, &mut state)
    }

    let mut hist_ptr: usize = state.history.len();

    state.raw_term = Some(Arc::new(RwLock::new(std::io::stdout().into_raw_mode()?)));

    'mainloop: loop {
        write_prompt(state.clone())?;

        let mut input = String::new();

        let mut i0 = [0u8];
        let mut line_escape = false;
        let mut arrow_seq = [0u8; 2];
        let mut in_arrow = (false, 0usize);
        let mut curr_inp_hist = String::new();
        let mut line_cursor = 0usize;
        while i0[0] != b'\x0D' || line_escape {
            if i0[0] == 27 {
                in_arrow = (true, 0);
            }
            if i0[0] == b'\x0D' {
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
            if i0[0] == 3 {
                // ctrl+c
                input.clear();
                println!("\x0D");
                std::io::stdout().flush()?;
                continue 'mainloop;
            }
            let amount = std::io::stdin().read(&mut i0).unwrap();
            if amount == 0 {
                continue;
            }
            if in_arrow.0 {
                arrow_seq[in_arrow.1] = i0[0];
                in_arrow.1 += 1;
                if in_arrow.1 > 1 {
                    in_arrow.0 = false;
                    match arrow_seq {
                        [91, 65] => {
                            // up arrow
                            if hist_ptr.checked_sub(1).is_some() {
                                hist_ptr -= 1;
                                let writer = state.raw_term.clone().unwrap();
                                let mut writer = writer.write().unwrap();

                                writer.write_all(b"\x0D")?;
                                write_prompt(state.clone())?;
                                writer.write_all(b"\x1b[0K")?;

                                curr_inp_hist = input;

                                input = state.history[hist_ptr].clone();
                                writer.write_all(input.as_bytes())?;
                                writer.flush()?;
                            }
                        }
                        [91, 66] => {
                            // down arrow
                            if hist_ptr + 1 < state.history.len() {
                                hist_ptr += 1;
                                let writer = state.raw_term.clone().unwrap();
                                let mut writer = writer.write().unwrap();

                                writer.write_all(b"\x0D")?;
                                write_prompt(state.clone())?;
                                writer.write_all(b"\x1b[0K")?;

                                input = state.history[hist_ptr].clone();
                                writer.write_all(input.as_bytes())?;
                                writer.flush()?;
                            } else {
                                hist_ptr = state.history.len();
                                let writer = state.raw_term.clone().unwrap();
                                let mut writer = writer.write().unwrap();

                                writer.write_all(b"\x0D")?;
                                write_prompt(state.clone())?;
                                writer.write_all(b"\x1b[0K")?;

                                input = curr_inp_hist.clone();
                                writer.write_all(input.as_bytes())?;
                                writer.flush()?;
                            }
                        }
                        [91, 68] => {
                            // left arrow
                            if line_cursor.checked_sub(1).is_some() {
                                let writer = state.raw_term.clone().unwrap();
                                let mut writer = writer.write().unwrap();
                                line_cursor -= 1;
                                writer.write_all(b"\x1b[1D")?;
                            } else {
                                print!("\x07");
                            }
                        }
                        [91, 67] => {
                            // right arrow
                            if line_cursor + 1 < input.len() {
                                let writer = state.raw_term.clone().unwrap();
                                let mut writer = writer.write().unwrap();
                                line_cursor += 1;
                                writer.write_all(b"\x1b[1C")?;
                            } else {
                                print!("\x07");
                            }
                        }
                        _ => {
                            continue;
                        }
                    }
                }
                continue;
            }
            if i0[0] != b'\x0D' {
                line_escape = false;
            }
            if i0[0] == b'\\' {
                line_escape = true;
            }
            let raw_term = state.raw_term.clone().unwrap();
            let mut raw_term = raw_term.write().unwrap();
            if i0[0] == b'\x7F' {
                if input.pop().is_none() {
                    raw_term.write_all(b"\x07")?;
                } else {
                    raw_term.write_all(b"\x08 \x08")?;
                }
            } else {
                input.push(char::from_u32(i0[0] as u32).unwrap());
                raw_term.write_all(&i0)?;
            }
            raw_term.flush()?;
        }

        println!("\x0D");
        input = input.clone().trim().to_string();
        state.history.push(input.clone());

        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::env::home_dir().unwrap().join(".sesh_history"))
            .unwrap()
            .write_all((input.clone() + "\n").into_bytes().as_slice())
            .unwrap();

        hist_ptr = state.history.len();

        state.entries += 1;
        eval(&input, &mut state);
    }
}
