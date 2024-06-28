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
    w: u64,
    h: u64,
    scale: u64
}

struct Layer {
    index: usize,
    content: LayerContent
}

struct Frame {
    index: usize,
    content: String
}

enum LayerContent {
    Still(String),
    Video(Vec<Frame>)
}

#[derive(Debug, PartialEq)]
enum LineContent {
    Layer(usize),
    Frame(usize),
    Normal(String)
}

pub fn compile(s: &str) -> Vec<u8> {
    let data = parse_with_engine::<Config, Toml>(s).unwrap();
    let config = data.headers;
    let body = data.body;
    let linecontent = body.lines().map(|c| parseline(c)).filter(|c| c != &LineContent::Normal("".to_string())).collect::<Vec<_>>();
    println!("{:#?}", config);
    println!("{:#?}", linecontent);
    Vec::<u8>::new()
}

fn parseline(s: &str) -> LineContent {
    if s.contains("## ") {
        LineContent::Frame(s.split_at(3).1.parse::<usize>().unwrap())
    } else if s.contains("# ") {
        LineContent::Layer(s.split_at(2).1.parse::<usize>().unwrap())
    } else {
        LineContent::Normal(s.to_owned())
    }
}
