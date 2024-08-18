use pixdown::compile;

fn main() {
    let text = r###"---
[size]
w = 2
h = 2
scale = 256
frames = 2

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
    compile(text);
}
