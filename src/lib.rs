use std::collections::HashMap;
use fronma::{parser::parse_with_engine, engines::Toml};
use serde::Deserialize;
use regex::{Regex, Match};
use itertools::Itertools;
use num_rational::Rational64;
use micro_png::{build_apng_u8, APNGBuilder, ImageData};

#[derive(Debug, Deserialize, Clone)]
struct Config {
    size: SizeConfig,
    colors: HashMap<char, String>,
}

#[derive(Debug, Deserialize, Clone)]
struct SizeConfig {
    w: usize,
    h: usize,
    scale: usize,
    frames: usize,
    rate: Option<u16>
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

#[derive(Debug)]
enum LayerPixmap { // (R, G, B, A)
    Still(Vec<Vec<(u8, u8, u8, u8)>>),
    Video(Vec<Vec<Vec<(u8, u8, u8, u8)>>>),
}

#[derive(Debug, PartialEq)]
enum Token {
    Layer(usize),
    Frame(usize),
    Normal(String)
}

pub fn compile(s: &str) -> Result<Vec<u8>, String> {
    let data = parse_with_engine::<Config, Toml>(s).unwrap();
    let config = data.headers;
    let body = data.body;
    println!("Parsing...");
    let token = body.lines().map(|c| tokenize(c)).filter(|c| c != &Token::Normal("".to_string())).collect::<Vec<_>>();
    let ast = parse(&token);
    println!("Putting color data...");
    let layers = generate_layers(&config.clone(), ast);
    println!("Merging layers...");
    let frames = generate_frames(&config, layers);
    println!("Scaling up...");
    let scaled = scaleup(&config, frames);
    println!("Generating (A)PNG...");
    let builder = APNGBuilder::new("", ImageData::RGBA(scaled)).set_def_dur((1, config.size.rate.unwrap_or(24)));
    let result = build_apng_u8(builder);
    result
}

fn scaleup(conf: &Config, frames: Vec<Vec<Vec<(u8, u8, u8, u8)>>>) -> Vec<Vec<Vec<(u8, u8, u8, u8)>>> {
    let mut result: Vec<Vec<Vec<(u8, u8, u8, u8)>>> = vec![];
    for f in &frames {
        let mut frame: Vec<Vec<(u8, u8, u8, u8)>> = vec![];
        for l in f {
            frame.push(l.iter().map(|&c| vec![c; conf.size.scale]).concat());
        }
        result.push(frame.into_iter().map(|c| vec![c; conf.size.scale]).concat());
    }
    result
}

fn generate_frames(conf: &Config, layers: Vec<LayerPixmap>) -> Vec<Vec<Vec<(u8, u8, u8, u8)>>> {
    let mut frames: Vec<Vec<Vec<(u8, u8, u8, u8)>>> = vec![];
    let mix = |cf: (u8, u8, u8, u8), cb: (Rational64, Rational64, Rational64, Rational64)| -> (Rational64, Rational64, Rational64, Rational64) {
        let c_f = (Rational64::new(cf.0 as i64, 1), Rational64::new(cf.1 as i64, 1), Rational64::new(cf.2 as i64, 1));
        let c_b = (cb.0, cb.1, cb.2);
        let one = Rational64::new(1, 1);
        let a_f = Rational64::new(cf.3 as i64, 255);
        let a_b = cb.3 / Rational64::new(255, 1);
        let a = a_f * a_b + a_f * (one - a_b) + (one - a_f) * a_b;
        let k_f = a_f * a_b + a_f * (one - a_b);
        let k_b = (one - a_f) * (one - a_b) + (one - a_f) * a_b;
        let c = (k_f * c_f.0 + k_b * c_b.0, k_f * c_f.1 + k_b * c_b.1, k_f * c_f.2 + k_b * c_b.2);
        (c.0, c.1, c.2, a * Rational64::new(255, 1))
    };
    for f in 0..conf.size.frames {
        let mut frame = vec![vec![(Rational64::new(0, 1), Rational64::new(0, 1), Rational64::new(0, 1), Rational64::new(0, 1)); conf.size.w];conf.size.h];
        for l in layers.iter() {
            if let LayerPixmap::Still(v) = l {
                v.iter().enumerate().for_each(|(y, c)| {
                    c.iter().enumerate().for_each(|(x, &d)| {
                        let b = frame[y % conf.size.h][x % conf.size.w];
                        frame[y % conf.size.h][x % conf.size.w] = mix(d, b);
                    });
                });
            }
            if let LayerPixmap::Video(vs) = l {
                let v = vs[f % vs.len()].clone();
                v.iter().enumerate().for_each(|(y, c)| {
                    c.iter().enumerate().for_each(|(x, &d)| {
                        let b = frame[y % conf.size.h][x % conf.size.w];
                        frame[y % conf.size.h][x % conf.size.w] = mix(d, b);
                    });
                });
            }
        }
        frames.push(frame.into_iter().map(|c| c.into_iter().map(|d| (d.0.to_integer() as u8, d.1.to_integer() as u8, d.2.to_integer() as u8, d.3.to_integer() as u8)).collect::<Vec<_>>()).collect::<Vec<_>>());
    }
    frames
}

fn generate_layers(conf: &Config, ast: Vec<Layer>) -> Vec<LayerPixmap> {
    let mut layers: Vec<LayerPixmap> = vec![];
    for l in ast.iter().sorted_by_key(|c| c.index) {
        if let LayerContent::Still(s) = &l.content {
            let pixmap = s.iter().map(|c| c.chars().map(|d| to_rgba(conf.colors.get(&d).unwrap_or(&"#000000".to_string()).to_string())).collect::<Vec<_>>()).collect::<Vec<_>>();
            layers.push(LayerPixmap::Still(pixmap));
        }
        if let LayerContent::Video(fs) = &l.content  {
            let mut pixmaps: Vec<Vec<Vec<(u8, u8, u8, u8)>>> = vec![];
            for f in fs.iter().sorted_by_key(|c| c.index) {
                let pixmap = f.content.iter().map(|c| c.chars().map(|d| to_rgba(conf.colors.get(&d).unwrap_or(&"#000000".to_string()).to_string())).collect::<Vec<_>>()).collect::<Vec<_>>();
                pixmaps.push(pixmap);
            }
            layers.push(LayerPixmap::Video(pixmaps));
        }
    }
    layers
}

fn to_rgba(hex: String) -> (u8, u8, u8, u8) {
    let unwrapstr = |r: Option<Match>| -> String {
        if let Some(m) = r {
            m.as_str().to_string()
        } else {
            "00".to_string()
        }
    };
    let unwrapstr_or = |r: Option<Match>, e: &str| -> String {
        if let Some(m) = r {
            m.as_str().to_string()
        } else {
            e.to_string()
        }
    };
    let rgba_p = Regex::new(r"^#(?<r>[0-9a-fA-F]{2})(?<g>[0-9a-fA-F]{2})(?<b>[0-9a-fA-F]{2})(?<a>[0-9a-fA-F]{2})?$").unwrap();
    let caps = rgba_p.captures(&hex).unwrap();
    let result_m = (caps.name("r"), caps.name("g"), caps.name("b"), caps.name("a"));
    let result_s = (&unwrapstr(result_m.0), &unwrapstr(result_m.1), &unwrapstr(result_m.2), &unwrapstr_or(result_m.3, "FF"));
    (
        u8::from_str_radix(result_s.0, 16).unwrap(),
        u8::from_str_radix(result_s.1, 16).unwrap(),
        u8::from_str_radix(result_s.2, 16).unwrap(),
        u8::from_str_radix(result_s.3, 16).unwrap()
    )
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
