// 2 枚の PNG の差を測る。本体の描画同士（設定を変えて撮ったキャプチャなど）を比べるのに使う。
// 使い方: imgdiff <a.png> <b.png> [--out <diff.png>] [--gain <倍率>]

fn main() {
    let mut args = std::env::args().skip(1);
    let a_path = args.next().expect("使い方: imgdiff <a.png> <b.png> [--out <diff.png>] [--gain <倍率>]");
    let b_path = args.next().expect("2 枚目の PNG がありません");
    let mut out: Option<String> = None;
    let mut gain = 1.0f32;
    while let Some(flag) = args.next() {
        let value = args.next().unwrap_or_else(|| panic!("{} に値がありません", flag));
        match flag.as_str() {
            "--out" => out = Some(value),
            "--gain" => gain = value.parse().expect("--gain"),
            _ => panic!("不明な引数: {}", flag),
        }
    }

    let a = image::open(&a_path).expect("1 枚目を開けません").to_rgb8();
    let b = image::open(&b_path).expect("2 枚目を開けません").to_rgb8();
    if a.dimensions() != b.dimensions() {
        panic!("大きさが違います: {:?} と {:?}", a.dimensions(), b.dimensions());
    }
    let (w, h) = a.dimensions();

    let mut diff = image::RgbImage::new(w, h);
    let mut sum = 0u64;
    let mut counts = [0u64; 4];
    let thresholds = [1u8, 8, 32, 128];
    for (p, (pa, pb)) in a.pixels().zip(b.pixels()).enumerate() {
        let d: Vec<u8> = (0..3).map(|i| pa.0[i].abs_diff(pb.0[i])).collect();
        let m = *d.iter().max().unwrap();
        sum += m as u64;
        for (i, t) in thresholds.iter().enumerate() {
            if m >= *t {
                counts[i] += 1;
            }
        }
        let x = p as u32 % w;
        let y = p as u32 / w;
        let scaled: Vec<u8> = d.iter().map(|v| ((*v as f32 * gain).min(255.0)) as u8).collect();
        diff.put_pixel(x, y, image::Rgb([scaled[0], scaled[1], scaled[2]]));
    }

    let pixels = (w as u64) * (h as u64);
    let percent = |n: u64| 100.0 * n as f64 / pixels as f64;
    println!(
        "mean max-channel |Δ| {:.2}/255, pixels with Δ >= 1: {:.2}%, >= 8: {:.2}%, >= 32: {:.2}%, >= 128: {:.2}%",
        sum as f64 / pixels as f64,
        percent(counts[0]),
        percent(counts[1]),
        percent(counts[2]),
        percent(counts[3])
    );
    if let Some(path) = out {
        diff.save(&path).expect("差分画像を書けません");
        println!("wrote {}", path);
    }
}
