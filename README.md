# Pixdown
ドットアニメーションに特化したラスタ画像軽量マークアップ言語

## How to write
Pixdownのデータ構造はヘッダーと内容の2つに分けられます
```
---
(ヘッダ部分)
---
(内容)
```

### ヘッダー
```toml
[size]
w = 2 # 幅
h = 2 # 高さ
scale = 256 # 拡大(縦, 横ともにscale倍)
frames = 8 # フレームの数
rate = 4 # fps

[colors] # 色の定義
"0" = "#000000"
"1" = "#ffffff"

[[options]] # オプション(なくてもよい)
order = [1, 0, 1, 0, 0, 1, 0, 0] # 順序指定
```

### 内容
```md
# 0
## 0
10
01

## 1
01
10
```
`#`: レイヤー番号

`##`: フレーム番号

### サンプル
![サンプル](example/example.png)

[ソースコード](example/example.pixdown)

## How to compile
リファレンス実装が動かせます
```sh
cargo run -- [Pixdownファイル] [出力先]
```

## Donation
後で書く

## License
[BRONSEELE-WARE LICENSE](LICENSE.md)で公開しています
