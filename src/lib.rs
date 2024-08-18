use std::collections::HashMap;
use fronma::{parser::parse_with_engine, engines::Toml};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    size: SizeConfig,
    colors: HashMap<char, String>,
}

#[derive(Debug, Deserialize)]
struct SizeConfig {
    w: usize,
    h: usize,
    scale: usize,
    frames: usize
}

#[derive(Debug)]
struct Layer {
    index: usize,
    content: LayerContent
}

#[derive(Debug)]
struct Frame {
    index: usize,
    content: Vec<String>
}

#[derive(Debug)]
enum LayerContent {
    Still(Vec<String>),
    Video(Vec<Frame>)
}

#[derive(Debug, PartialEq)]
enum Token {
    Layer(usize),
    Frame(usize),
    Normal(String)
}

pub fn compile(s: &str) -> Vec<u8> {
    let data = parse_with_engine::<Config, Toml>(s).unwrap();
    let config = data.headers;
    let body = data.body;
    let token = body.lines().map(|c| tokenize(c)).filter(|c| c != &Token::Normal("".to_string())).collect::<Vec<_>>();
    let ast = parse(&token);
    println!("{:#?}", config);
    println!("{:#?}", ast);
    Vec::<u8>::new()
}

fn parse(token: &[Token]) -> Vec<Layer> {
    let mut i = 0usize;
    let mut layers = Vec::<Layer>::new();
    while i < token.len() {
        while let Token::Layer(layern) = token[i] {
            i += 1;
            if let Token::Frame(_) = token[i] {
                let mut frames = Vec::<Frame>::new();
                while let Token::Frame(framen) = token[i]  {
                    i += 1;
                    let mut contents = Vec::<String>::new();
                    while let Token::Normal(s) = &token[i] {
                        contents.push(s.to_owned());
                        i += 1;
                        if i >= token.len() {
                            break;
                        }
                    }
                    frames.push(Frame {
                        index: framen,
                        content: contents
                    });
                    if i >= token.len() {
                        break;
                    }
                }
                layers.push(Layer {
                    index: layern,
                    content: LayerContent::Video(frames)
                });
            } else {
                let mut contents = Vec::<String>::new();
                while let Token::Normal(s) = &token[i] {
                    contents.push(s.to_owned());
                    i += 1;
                    if i >= token.len() {
                        break;
                    }
                }
                layers.push(Layer {
                    index: layern,
                    content: LayerContent::Still(contents)
                });
            }
            if i >= token.len() {
                break;
            }
        }
    }
    layers
} 

fn tokenize(s: &str) -> Token {
    if s.contains("## ") {
        Token::Frame(s.split_at(3).1.parse::<usize>().unwrap())
    } else if s.contains("# ") {
        Token::Layer(s.split_at(2).1.parse::<usize>().unwrap())
    } else {
        Token::Normal(s.to_owned())
    }
}
