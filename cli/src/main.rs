use chrono::Utc;
use clap::Parser as _;
use just_message::{JustMessage, Message, Response};
use lib_fichar::State as AppFichar;
use pest::Parser;
use pest_derive::Parser;
use render::Renderer;
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, Write},
    process::Stdio,
};

// TODO: convert to API, with public/private key auth

const UP: &str = "\x1b[1A";
const DOWN: &str = "\x1b[1B";
const START: &str = "\x1b[0G";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const INVERSE: &str = "\x1b[7m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const PURPLE: &str = "\x1b[35m";

#[derive(clap::Parser)]
struct Args {}

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct Command;

#[derive(Debug, Clone, Copy)]
struct Prompt {
    script: bool,
    person: u32,
}

fn success(prompt: Prompt) {
    let color = match prompt.script {
        true => BLUE,
        false => GREEN,
    };
    print!(
        "{START}{UP}{INVERSE}{BOLD}{color} @{} {RESET}{START}{DOWN}",
        prompt.person
    );
    std::io::stdout().flush().unwrap();
}
fn failure(prompt: Prompt) {
    print!(
        "{START}{UP}{INVERSE}{BOLD}{RED} @{} {RESET}{START}{DOWN}",
        prompt.person
    );
    std::io::stdout().flush().unwrap();
}
fn prompts(prompt: Prompt) {
    let color = match prompt.script {
        true => PURPLE,
        false => YELLOW,
    };
    print!("{INVERSE}{BOLD}{color} @{} {RESET} ", prompt.person);
    std::io::stdout().flush().unwrap();
}

fn main() {
    let Args {} = Args::parse();
    let mut person = 0;
    let mut app = AppFichar::default();
    let mut stdin = std::io::stdin().lock();
    // let mut line = String::new();
    let mut queue = VecDeque::new();
    let mut prompt = Prompt {
        script: false,
        person,
    };
    loop {
        let line = queue
            .pop_front()
            .inspect(|s| {
                prompt = Prompt {
                    script: true,
                    person,
                };
                prompts(prompt);
                println!("{s}")
            })
            .unwrap_or_else(|| {
                let mut buffer = String::new();
                prompt = Prompt {
                    script: false,
                    person,
                };
                prompts(prompt);
                stdin.read_line(&mut buffer).unwrap();
                buffer
            });
        let line = line.trim();
        if line.starts_with("/") {
            match Command::parse(Rule::command, line) {
                Ok(mut pairs) => {
                    success(prompt);
                    std::io::stdout().flush().unwrap();
                    let command = pairs.next().unwrap().into_inner().next().unwrap();
                    match command.as_rule() {
                        Rule::command_quit => break,
                        Rule::command_save => {
                            symtree::to_writer_pretty(&app, File::create("gen/state.scm").unwrap())
                                .unwrap();
                        }
                        Rule::command_person => {
                            person = command
                                .into_inner()
                                .next()
                                .unwrap()
                                .as_str()
                                .parse()
                                .unwrap();
                        }
                        Rule::command_load => {
                            app =
                                symtree::from_reader(File::open("gen/state.scm").unwrap()).unwrap();
                        }
                        Rule::command_reset => {
                            app = Default::default();
                        }
                        Rule::command_script => {
                            let script: u32 = command
                                .into_inner()
                                .next()
                                .unwrap()
                                .as_str()
                                .parse()
                                .unwrap();
                            let script =
                                std::fs::read_to_string(format!("gen/script-{script}")).unwrap();
                            queue.extend(script.lines().map(|s| s.to_string()));
                        }
                        _ => unreachable!(),
                    }
                }
                Err(err) => {
                    failure(prompt);
                    std::io::stdout().flush().unwrap();
                    println!("{err:#?}");
                }
            }
        } else {
            let responses = app.message(Message {
                instant: now(),
                content: line.to_string(),
                person,
            });
            if std::fs::exists("gen/cli").unwrap() {
                std::fs::remove_dir_all("gen/cli").unwrap();
            }
            std::fs::create_dir("gen/cli").unwrap();
            let mut doc_index = 0;
            for response in responses {
                match response {
                    Response::Success => {
                        success(prompt);
                        std::io::stdout().flush().unwrap();
                    }
                    Response::Text(response) => println!("{response}"),
                    Response::Failure => {
                        failure(prompt);
                        std::io::stdout().flush().unwrap();
                    }
                    Response::Document {
                        main,
                        bytes,
                        sources,
                    } => {
                        let renderer = Renderer::new();
                        println!("rendering...");
                        let image_png = renderer.render(main, sources, bytes);
                        println!("writing image...");
                        std::fs::write(format!("gen/cli/out-{doc_index}.png"), image_png).unwrap();
                        doc_index += 1;
                    }
                }
            }
            if doc_index != 0 {
                println!("opening image...");
                std::process::Command::new("xdg-open")
                    .arg("gen/cli/out-0.png")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .status()
                    .unwrap();
            }
        }
    }
}

fn now() -> i64 {
    Utc::now().timestamp()
}
