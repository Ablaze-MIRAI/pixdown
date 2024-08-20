use std::fs::File;
use std::io::Write;
use pixdown::compile;

fn main() {
    let text = r###"---
[size]
w = 2
h = 2
scale = 256
frames = 2
rate = 4

[colors]
"A" = "#000000"
"B" = "#ffffff"
---
# 0
BB
BB

# 1
## 0
AB
BA

## 1
BA
AB
"###;
    if let Ok(b) = compile(text) {
        let mut file = File::create("image.png").unwrap();
        file.write_all(&b).unwrap();
        file.flush().unwrap();
    }
}
