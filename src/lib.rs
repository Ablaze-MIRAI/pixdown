use std::collections::HashMap;
use fronma::{engines::Toml, parser::parse_with_engine};
use serde::Deserialize;
use regex::{Regex, Match};
use itertools::Itertools;
use num_rational::Rational64;
use wasm_bindgen::prelude::*;
use crc32fast::hash;
use deflate::{deflate_bytes_zlib_conf, Compression};

#[derive(Deserialize, Clone)]
struct Config {
    size: SizeConfig,
    colors: HashMap<char, String>,
    options: Option<Options>,
    meta: Option<HashMap<String, String>>
}

#[derive(Deserialize, Clone)]
struct SizeConfig {
    w: usize,
    h: usize,
    scale: usize,
    frames: usize,
    rate: Option<[u16; 2]>
}

#[derive(Deserialize, Clone)]
struct Options {
    order: Option<Vec<usize>>
}

struct Layer {
    index: usize,
    content: LayerContent
}

struct Frame {
    index: usize,
    content: Vec<String>
}

enum LayerContent {
    Still(Vec<String>),
    Video(Vec<Frame>)
}

enum LayerPixmap { // (R, G, B, A)
    Still(Vec<Vec<(u8, u8, u8, u8)>>),
    Video(Frames),
}

#[derive(PartialEq)]
enum Token {
    Layer(usize),
    Frame(usize),
    Normal(String)
}

type Frames = Vec<Vec<Vec<(u8, u8, u8, u8)>>>;

#[wasm_bindgen]
pub fn compile(s: &str) -> Result<Vec<u8>, String> {
    let data = parse_with_engine::<Config, Toml>(s).unwrap();
    let config = data.headers;
    let body = data.body;
    println!("[1/5] Parsing...");
    let token = body.lines().map(|c| tokenize(c)).filter(|c| c != &Token::Normal("".to_string())).collect::<Vec<_>>();
    let ast = parse(&token);
    println!("[2/5] Putting color data...");
    let layers = generate_layers(&config.clone(), ast);
    println!("[3/5] Merging layers...");
    let frames = generate_frames(&config, layers);
    println!("[4/5] Applying options...");
    let applyed = if let Some(option) = config.clone().options {
        applyoption(frames, option)
    } else {
        frames
    };
    print!("[5/5] Generating (A)PNG...");
    let result = generate_image(&applyed, &config);
    result
}

fn generate_image(frames: &Frames, conf: &Config) -> Result<Vec<u8>, String> {
    let mut result: Vec<u8> = vec![];
    if frames.is_empty() {
        return Err("Image is empty".to_string());
    }
    if frames.len() != conf.size.frames {
        return Err("Frame counts does not match.".to_string());
    }
    let heights = frames.iter().map(|c| c.len());
    let widths = frames.iter().map(|c| c.iter().map(|d| d.len()).collect::<Vec<_>>()).concat();
    if heights.clone().min().unwrap() == 0 {
        return Err("Height is zero".to_string());
    }
    if widths.iter().min().unwrap() == &0 {
        return Err("Width is zero".to_string());
    }
    if heights.clone().min().unwrap() != heights.max().unwrap() {
        return Err("Unaligned heights.".to_string());
    }
    if widths.iter().min().unwrap() != widths.iter().max().unwrap() {
        return Err("Unaligned widths.".to_string());
    }
    result.extend([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    { // IHDR
        let mut ihdr: Vec<u8> = vec![];
        ihdr.extend(b"IHDR");
        ihdr.extend(((conf.size.w * conf.size.scale) as u32).to_be_bytes());
        ihdr.extend(((conf.size.h * conf.size.scale) as u32).to_be_bytes());
        ihdr.extend([0x08, 0x06, 0x00, 0x00, 0x00]);
        result.extend(write_chunk(&ihdr));
    }
    result.extend(write_chunk(b"tEXtGenerator\x00Pixdown")); // tEXT
    if let Some(meta) = conf.meta.clone() {
        meta.iter().for_each(|(k, v)| {
            let mut itxt: Vec<u8> = vec![];
            itxt.extend(b"iTXt");
            itxt.extend(k.as_bytes());
            itxt.extend([0x00, 0x01, 0x00, 0x00, 0x00]); // [Null separator, Compression flag: true, Compression method: deflate-zlib, Null separator, Null separator]
            itxt.extend(deflate_bytes_zlib_conf(&v.as_bytes(), Compression::Best));
            result.extend(write_chunk(&itxt));
        });
    }
    if conf.size.frames > 1 { // acTL
        let mut actl: Vec<u8> = vec![];
        actl.extend(b"acTL");
        actl.extend((conf.size.frames as u32).to_be_bytes());
        actl.extend([0x00, 0x00, 0x00, 0x00]);
        result.extend(write_chunk(&actl));
    }
    let mut sequence = 0u32;
    frames.iter().enumerate().for_each(|(f, fd)| {
        if conf.size.frames > 1 { // fcTL
            let mut fctl: Vec<u8> = vec![];
            fctl.extend(b"fcTL");
            fctl.extend(sequence.to_be_bytes());
            sequence += 1;
            fctl.extend(((conf.size.w * conf.size.scale) as u32).to_be_bytes());
            fctl.extend(((conf.size.h * conf.size.scale) as u32).to_be_bytes());
            fctl.extend(0u32.to_be_bytes());
            fctl.extend(0u32.to_be_bytes());
            let rate = conf.size.rate.unwrap_or([1, 24]);
            fctl.extend(rate[0].to_be_bytes());
            fctl.extend(if rate[1] == 0 { 24 } else {rate[1] }.to_be_bytes());
            fctl.extend([0x00u8, 0x01u8]);
            result.extend(write_chunk(&fctl));
        }
        let mut fdat: Vec<u8> = vec![]; // fdAT
        let mut pixmap: Vec<u8> = vec![];
        if f == 0 {
            fdat.extend(b"IDAT");
        } else {
            fdat.extend(b"fdAT");
            fdat.extend(sequence.to_be_bytes());
            sequence += 1;
        }
        fd.iter().for_each(|yd| {
            for _ in 0..conf.size.scale {
                pixmap.push(0);
                yd.iter().for_each(|xd| {
                    for _ in 0..conf.size.scale {
                        pixmap.extend([xd.0, xd.1, xd.2, xd.3]);
                    }
                });
            }
        });
        fdat.extend(deflate_bytes_zlib_conf(&pixmap, Compression::Best));
        result.extend(write_chunk(&fdat));
        println!("Generated frame {}", f);
    });
    result.extend(write_chunk(b"IEND"));
    Ok(result)
}

fn write_chunk(data: &[u8]) -> Vec<u8> {
    let mut result: Vec<u8> = vec![];
    result.extend((data.len() as u32 - 4).to_be_bytes());
    result.extend(data);
    result.extend(hash(data).to_be_bytes());
    result
}

fn applyoption(frames: Frames, option: Options) -> Frames {
    let mut result: Frames = frames;
    if let Some(args) = option.order {
        result = options::order(result, args);
    }
    result
}

fn generate_frames(conf: &Config, layers: Vec<LayerPixmap>) -> Frames {
    let mut frames: Frames = vec![];
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
            let mut pixmaps: Frames = vec![];
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

mod options {
    use crate::Frames;
    pub fn order(frames: Frames, order: Vec<usize>) -> Frames {
        let mut result: Frames = vec![];
        for &i in &order {
            result.push((&frames)[i].clone());
        }
        result
    }
}
