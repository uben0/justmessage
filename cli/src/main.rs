use chrono::Utc;
use clap::Parser;
use just_message::{JustMessage, Message, Response};
use lib_fichar::State as AppFichar;
use render::Renderer;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::Stdio,
};

#[derive(Parser)]
struct Args {
    #[arg(long, short)]
    reset: bool,
}

fn main() {
    let Args { reset } = Args::parse();
    let mut app = if reset {
        AppFichar::default()
    } else {
        symtree::from_reader(File::open("gen/state.scm").unwrap()).unwrap()
    };
    let renderer = Renderer::new();
    let mut stdin = BufReader::new(std::io::stdin().lock());
    let mut buffer = String::new();
    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();
        buffer.clear();
        stdin.read_line(&mut buffer).unwrap();
        let line = buffer.trim();
        if line == "quit" {
            break;
        }
        let responses = app.message(Message {
            instant: now(),
            content: line.to_string(),
            person: 0,
        });
        for response in responses {
            match response {
                Response::Success => println!("success"),
                Response::Text(response) => println!("{}", response),
                Response::Failure => println!("fail"),
                Response::Document {
                    main,
                    bytes,
                    sources,
                } => {
                    println!("rendering...");
                    let image_png = renderer.render(main, sources, bytes);
                    println!("writing image...");
                    std::fs::write("gen/out.png", image_png).unwrap();
                    println!("opening image...");
                    std::process::Command::new("xdg-open")
                        .arg("gen/out.png")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .status()
                        .unwrap();
                }
            }
        }
        println!();
    }
    symtree::to_writer_pretty(&app, File::create("gen/state.scm").unwrap()).unwrap();
}

// fn date_time(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
//     Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
//         .unwrap()
//         .timestamp()
// }

fn now() -> i64 {
    Utc::now().timestamp()
}
