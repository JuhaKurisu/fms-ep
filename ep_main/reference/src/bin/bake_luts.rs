// data/luts/ に 4 つの .cube を書く。入力はシェーパー後の 0〜1、出力は表示値
use reference::lut::Lut3d;
use reference::tonemap::{display, shaper_inverse, Operator};
use std::path::Path;

fn main() {
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/luts");
    std::fs::create_dir_all(&out_dir).expect("data/luts を作れません");
    let size = 33;
    for (name, op) in [
        ("00_aces", Operator::Aces),
        ("01_reinhard", Operator::Reinhard),
        ("02_hable", Operator::Hable),
        ("03_agx", Operator::Agx),
    ] {
        let lut = Lut3d::from_fn(size, |t| display(op, [shaper_inverse(t[0]), shaper_inverse(t[1]), shaper_inverse(t[2])]));
        let path = out_dir.join(format!("{}.cube", name));
        std::fs::write(&path, lut.to_cube(name)).expect("書けません");
        println!("{}", path.display());
    }
}
