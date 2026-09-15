use rayon::prelude::*;
use reference::ibl::{EnvMaps, Equirect};
use reference::lut::Lut3d;
use reference::prefs::{Prefs, Snapshot};
use reference::scene::Scene;
use reference::shade;
use reference::tonemap::shaper;
use reference::trace::{Outcome, Ray, Stats, Tracer};
use reference::vec::{Quat, Vec3};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct Args {
    prefs: PathBuf,
    snapshot: PathBuf,
    width: Option<u32>,
    height: Option<u32>,
    out: PathBuf,
    screen: Option<PathBuf>,
    diff: Option<PathBuf>,
    diff_gain: f64,
    max_steps: usize,
    refine_edge: f64,
    env: PathBuf,
    luts: PathBuf,
}

// 引数なしなら captures/ の最新フォルダ。--capture <dir> でフォルダ指定。--prefs / --snapshot / --out は個別に上書き
fn parse_args() -> Result<Args, String> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut capture: Option<PathBuf> = None;
    let mut prefs = None;
    let mut snapshot = None;
    let mut out = None;
    let mut screen = None;
    let mut diff = None;
    let mut diff_gain = 1.0;
    let mut width = None;
    let mut height = None;
    let mut max_steps = 20000;
    let mut refine_edge = 0.5;
    let mut env = base.join("../data/env.hdr");
    let mut luts = base.join("../data/luts");
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{} に値がありません", flag))?;
        match flag.as_str() {
            "--capture" => capture = Some(value.into()),
            "--prefs" => prefs = Some(PathBuf::from(value)),
            "--snapshot" => snapshot = Some(PathBuf::from(value)),
            "--width" => width = Some(value.parse().map_err(|e| format!("--width: {}", e))?),
            "--height" => height = Some(value.parse().map_err(|e| format!("--height: {}", e))?),
            "--out" => out = Some(PathBuf::from(value)),
            "--screen" => screen = Some(PathBuf::from(value)),
            "--diff" => diff = Some(PathBuf::from(value)),
            "--diff-gain" => diff_gain = value.parse().map_err(|e| format!("--diff-gain: {}", e))?,
            "--max-steps" => max_steps = value.parse().map_err(|e| format!("--max-steps: {}", e))?,
            "--refine-edge" => refine_edge = value.parse().map_err(|e| format!("--refine-edge: {}", e))?,
            "--env" => env = PathBuf::from(value),
            "--luts" => luts = PathBuf::from(value),
            _ => return Err(format!("不明な引数: {}", flag)),
        }
    }
    let capture = match capture {
        Some(dir) => Some(dir),
        None if prefs.is_some() && snapshot.is_some() => None,
        None => Some(latest_capture(&base.join("../captures"))?),
    };
    if let Some(dir) = &capture {
        println!("capture: {}", dir.display());
    }
    let in_capture = |name: &str| capture.as_ref().map(|d| d.join(name));
    Ok(Args {
        prefs: prefs.or_else(|| in_capture("prefs.json")).unwrap_or_else(|| base.join("../data/prefs.json")),
        snapshot: snapshot.or_else(|| in_capture("snapshot.json")).unwrap_or_else(|| base.join("../data/snapshot.json")),
        width,
        height,
        out: out.or_else(|| in_capture("reference.png")).unwrap_or_else(|| base.join("reference.png")),
        screen: screen.or_else(|| in_capture("screen.png")),
        diff: diff.or_else(|| in_capture("diff.png")),
        diff_gain,
        max_steps,
        refine_edge,
        env,
        luts,
    })
}

fn latest_capture(captures: &std::path::Path) -> Result<PathBuf, String> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(captures)
        .map_err(|e| format!("{}: {}", captures.display(), e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.join("snapshot.json").is_file())
        .collect();
    dirs.sort();
    dirs.pop().ok_or_else(|| format!("{} に snapshot.json を持つフォルダがありません", captures.display()))
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let prefs = Prefs::load(&args.prefs)?;
    let snapshot = Snapshot::load(&args.snapshot)?;
    let width = args.width.unwrap_or(snapshot.view_width).max(1);
    let height = args.height.unwrap_or(snapshot.view_height).max(1);

    let observer = Vec3::new(snapshot.camera_position[0], snapshot.camera_position[1], snapshot.camera_position[2]);
    let velocity = if prefs.camera_velocity_zero {
        Vec3::default()
    } else {
        Vec3::new(snapshot.camera_velocity[0], snapshot.camera_velocity[1], snapshot.camera_velocity[2])
    };
    let r = snapshot.camera_rotation;
    let rotation = Quat::new(r[0], r[1], r[2], r[3]);
    let time = snapshot.scene_time;
    if let Some(render) = &snapshot.render_camera {
        let same = render.position == snapshot.camera_position && render.rotation == snapshot.camera_rotation;
        if !same {
            println!("注意: 自由カメラで撮られています。参照は観測者の位置・向きから描きます");
        }
    }
    if !snapshot.thrown_spheres.is_empty() {
        println!("注意: 投げた球が {} 個ありますが参照では描きません", snapshot.thrown_spheres.len());
    }

    let started = Instant::now();
    let mut scene = Scene::from_prefs(&prefs, &snapshot, args.refine_edge);
    if prefs.ibl_enabled {
        let started = Instant::now();
        let source = Equirect::load_hdr(&args.env)?;
        scene.env = Some(EnvMaps::build(source));
        println!("env {} prepare {:.2}s", args.env.display(), started.elapsed().as_secs_f64());
    }
    let tonemap = if prefs.tonemap_enabled {
        match std::fs::read_dir(&args.luts) {
            Err(e) => {
                println!("注意: {} が読めないのでトーンマップなし ({})", args.luts.display(), e);
                None
            }
            Ok(entries) => {
                let mut files: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().map(|x| x == "cube").unwrap_or(false))
                    .collect();
                files.sort();
                if files.is_empty() {
                    println!("注意: {} に .cube がないのでトーンマップなし", args.luts.display());
                    None
                } else {
                    let index = prefs.tonemap_lut.min(files.len() - 1);
                    let path = &files[index];
                    println!("lut {}", path.display());
                    Some(Lut3d::parse_cube(&std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?)?)
                }
            }
        }
    } else {
        None
    };
    scene.prepare(observer, time);
    println!("c = {}, objects = {}, prepare {:.2}s", scene.c, scene.objects.len(), started.elapsed().as_secs_f64());
    for o in &scene.objects {
        if !o.name.starts_with("wave[") || o.name == "wave[0,0,0]" {
            println!(
                "  {:12} triangles {:5}  k {:.4}  bound center ({:.2}, {:.2}, {:.2}) radius {:.2}",
                o.name,
                o.mesh.triangle_count(),
                o.k,
                o.bound_center.x,
                o.bound_center.y,
                o.bound_center.z,
                o.bound_radius
            );
        }
    }

    let beta = velocity * (1.0 / scene.c);
    let aspect = width as f64 / height as f64;
    let f = 1.0 / (prefs.camera_fov.to_radians() * 0.5).tan();

    let started = Instant::now();
    let pixel_count = width as usize * height as usize;
    let chunk = 64usize;
    println!("threads {}", rayon::current_num_threads());
    let chunk_count = pixel_count.div_ceil(chunk);
    let done_chunks = AtomicUsize::new(0);
    let report_every = (chunk_count / 200).max(1);
    let limited = AtomicUsize::new(0);
    let stats = std::sync::Mutex::new(Stats::default());
    let mut pixels = vec![0u8; pixel_count * 3];
    pixels.par_chunks_mut(chunk * 3).enumerate().for_each_init(Tracer::new, |tracer, (ci, out)| {
        let mut chunk_limited = 0;
        for (k, px) in out.chunks_mut(3).enumerate() {
            let index = ci * chunk + k;
            let i = (index % width as usize) as u32;
            let j = (index / width as usize) as u32;
            let ndc_x = (i as f64 + 0.5) / width as f64 * 2.0 - 1.0;
            let ndc_y = 1.0 - (j as f64 + 0.5) / height as f64 * 2.0;
            let dir_view = Vec3::new(ndc_x * aspect / f, ndc_y / f, 1.0).normalize();
            let dir_lab = shade::aberrate(rotation.rotate(dir_view), -beta);
            let ray = Ray { origin: observer, dir: dir_lab, time };
            // StepLimit のマゼンタは目印なのでトーンマップを掛けない
            let tonemap_color = |linear: [f64; 3]| match &tonemap {
                Some(lut) => lut.sample([shaper(linear[0] * prefs.exposure), shaper(linear[1] * prefs.exposure), shaper(linear[2] * prefs.exposure)]),
                None => linear,
            };
            let color = match tracer.trace(&scene, &ray, None, args.max_steps) {
                Outcome::Hit(hit) => tonemap_color(shade::shade(&scene, tracer, &hit, observer, args.max_steps)),
                Outcome::Miss => tonemap_color(match &scene.env {
                    Some(env) if scene.ibl_enabled => env.background(dir_lab),
                    _ => [1.0, 1.0, 1.0],
                }),
                Outcome::StepLimit => {
                    chunk_limited += 1;
                    [1.0, 0.0, 1.0]
                }
            };
            for (dst, ch) in px.iter_mut().zip(color) {
                *dst = (ch.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        limited.fetch_add(chunk_limited, Ordering::Relaxed);
        let done = done_chunks.fetch_add(1, Ordering::Relaxed) + 1;
        if done % report_every == 0 || done == chunk_count {
            let elapsed = started.elapsed().as_secs_f64();
            let remaining = elapsed / done as f64 * (chunk_count - done) as f64;
            eprint!("\r{:5.1}%  {:.0}s elapsed, {:.0}s left   ", done as f64 * 100.0 / chunk_count as f64, elapsed, remaining);
        }
        stats.lock().unwrap().add(&std::mem::take(&mut tracer.stats));
    });
    eprintln!();
    let limited = limited.into_inner();
    let stats = stats.into_inner().unwrap();
    println!(
        "rays {}  steps/ray {:.1}  sphere steps/ray {:.1}  group checks/ray {:.1}  triangle measures/ray {:.1}",
        stats.rays,
        stats.steps as f64 / stats.rays as f64,
        stats.sphere_steps as f64 / stats.rays as f64,
        stats.group_checks as f64 / stats.rays as f64,
        stats.triangle_measures as f64 / stats.rays as f64
    );
    image::save_buffer(&args.out, &pixels, width, height, image::ColorType::Rgb8).map_err(|e| e.to_string())?;
    if let (Some(screen), Some(diff)) = (&args.screen, &args.diff) {
        if screen.is_file() {
            write_diff(screen, diff, &pixels, width, height, args.diff_gain)?;
        }
    }
    println!(
        "{}x{} render {:.2}s, step limit pixels {}, wrote {}",
        width,
        height,
        started.elapsed().as_secs_f64(),
        limited,
        args.out.display()
    );
    Ok(())
}

// 本体の画面と参照画像の画素ごとの差(チャンネル差の最大)を白黒で書き、差の大きさの分布を出す
fn write_diff(screen: &std::path::Path, out: &std::path::Path, reference: &[u8], width: u32, height: u32, gain: f64) -> Result<(), String> {
    let image = image::open(screen).map_err(|e| format!("{}: {}", screen.display(), e))?.into_rgb8();
    if image.width() != width || image.height() != height {
        println!(
            "diff: {} は {}x{} で参照画像 {}x{} と大きさが違うので作りません",
            screen.display(),
            image.width(),
            image.height(),
            width,
            height
        );
        return Ok(());
    }
    let screen_pixels = image.as_raw();
    let count = (width * height) as usize;
    let mut diff = vec![0u8; count];
    let mut sum = 0u64;
    let mut over = [0usize; 3];
    let thresholds = [8u8, 32, 128];
    for (px, d) in diff.iter_mut().enumerate() {
        let m = (0..3).map(|k| reference[px * 3 + k].abs_diff(screen_pixels[px * 3 + k])).max().unwrap();
        *d = (m as f64 * gain).min(255.0) as u8;
        sum += m as u64;
        for (n, &t) in over.iter_mut().zip(&thresholds) {
            if m >= t {
                *n += 1;
            }
        }
    }
    image::save_buffer(out, &diff, width, height, image::ColorType::L8).map_err(|e| e.to_string())?;
    let count = count as f64;
    println!(
        "diff: mean max-channel |Δ| {:.2}/255, pixels with Δ >= 8: {:.2}%, >= 32: {:.2}%, >= 128: {:.2}%, wrote {}",
        sum as f64 / count,
        over[0] as f64 * 100.0 / count,
        over[1] as f64 * 100.0 / count,
        over[2] as f64 * 100.0 / count,
        out.display()
    );
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
