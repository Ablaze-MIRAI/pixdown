use std::env::args;
use std::fs::{File, read_to_string};
use std::io::Write;
use pixdown::compile;

fn main() {
    let args = args().collect::<Vec<String>>();
    let text = read_to_string(args.get(1).unwrap()).unwrap();
    if let Ok(b) = compile(&text) {
        let mut file = File::create(args.get(2).unwrap()).unwrap();
        file.write_all(&b).unwrap();
        file.flush().unwrap();
    }
}
