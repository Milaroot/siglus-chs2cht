mod converter;
mod crypto;
mod gameexe;
mod scene;
mod script;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};

use converter::Converter;

fn convert_scene(
    scene_path: &Path,
    output_path: &Path,
    work_dir: &Path,
    converter: &Converter,
) -> Result<()> {
    let ss_dir = work_dir.join("ss_unpacked");
    let ss_converted_dir = work_dir.join("ss_converted");
    fs::create_dir_all(&ss_dir)?;
    fs::create_dir_all(&ss_converted_dir)?;

    eprintln!("[1/3] Unpacking scene.chs ...");
    let scripts = scene::unpack_scene(scene_path, &ss_dir, None)?;

    let scene_names: HashSet<String> = scripts
        .iter()
        .map(|(filename, _)| filename.strip_suffix(".ss").unwrap_or(filename).to_string())
        .collect();

    eprintln!(
        "[2/3] Converting {} scripts (CHS -> CHT) ...",
        scripts.len()
    );
    let mut total_strings = 0u32;
    let mut converted_count = 0u32;

    for (i, (filename, filepath)) in scripts.iter().enumerate() {
        let texts = script::extract_texts(filepath)?;
        if texts.is_empty() {
            fs::copy(filepath, ss_converted_dir.join(filename))?;
            continue;
        }

        let mut new_texts = Vec::new();
        for (idx, text) in &texts {
            if scene_names.contains(text.as_str()) {
                new_texts.push((*idx, text.clone()));
                total_strings += 1;
                continue;
            }
            let converted = converter.convert(text);
            if converted != *text {
                converted_count += 1;
            }
            new_texts.push((*idx, converted));
            total_strings += 1;
        }

        let out_ss = ss_converted_dir.join(filename);
        script::replace_texts(filepath, &new_texts, &out_ss)?;

        if (i + 1) % 50 == 0 || i + 1 == scripts.len() {
            eprintln!("    Progress: {}/{} scripts", i + 1, scripts.len());
        }
    }
    eprintln!(
        "  Processed {} strings, {} changed",
        total_strings, converted_count
    );

    eprintln!("[3/3] Repacking scene.chs ...");
    scene::pack_scene(scene_path, &ss_converted_dir, output_path, None)?;
    Ok(())
}

fn convert_gameexe(
    gameexe_path: &Path,
    output_path: &Path,
    work_dir: &Path,
    converter: &Converter,
) -> Result<()> {
    let ini_path = work_dir.join("Gameexe.ini");
    let ini_converted_path = work_dir.join("Gameexe_cht.ini");

    eprintln!("[1/3] Decrypting Gameexe.chs ...");
    gameexe::unpack_gameexe(gameexe_path, &ini_path, None)?;

    eprintln!("[2/3] Converting Gameexe text ...");
    let ini_data = fs::read(&ini_path)?;
    let converted = gameexe::convert_gameexe_text(&ini_data, converter)?;
    fs::write(&ini_converted_path, &converted)?;

    eprintln!("[3/3] Re-encrypting Gameexe.chs ...");
    let raw = fs::read(gameexe_path)?;
    let need_key = u32::from_le_bytes(raw[4..8].try_into().unwrap()) == 1;
    gameexe::pack_gameexe(&ini_converted_path, output_path, need_key, None)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let input_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        exe_dir.join("input")
    };

    let output_dir = exe_dir.join("output");
    let work_dir = exe_dir.join("_work");
    fs::create_dir_all(&output_dir)?;
    fs::create_dir_all(&work_dir)?;

    let scene_path = input_dir.join("scene.chs");
    let gameexe_path = input_dir.join("Gameexe.chs");

    let has_scene = scene_path.is_file();
    let has_gameexe = gameexe_path.is_file();

    if !has_scene && !has_gameexe {
        eprintln!("Error: scene.chs or Gameexe.chs not found in {}", input_dir.display());
        eprintln!("Place game files in input/ or specify the game directory:");
        eprintln!("  siglus-chs2cht <game_directory>");
        std::process::exit(1);
    }

    eprintln!("{}", "=".repeat(60));
    eprintln!("  SiglusEngine CHS -> CHT Converter (Rust)");
    eprintln!("{}", "=".repeat(60));
    eprintln!("  Input:  {}", input_dir.display());
    eprintln!("  Output: {}", output_dir.display());
    eprintln!();

    let start = Instant::now();

    eprintln!("Loading dictionaries...");
    let converter = Converter::new();
    eprintln!("  Done.");
    eprintln!();

    if has_scene {
        eprintln!("{}", "\u{2501}".repeat(60));
        eprintln!("\u{25B6} Processing scene.chs");
        eprintln!("{}", "\u{2501}".repeat(60));
        let scene_out = output_dir.join("scene.chs");
        convert_scene(&scene_path, &scene_out, &work_dir, &converter)
            .context("Failed to convert scene")?;
        eprintln!();
    }

    if has_gameexe {
        eprintln!("{}", "\u{2501}".repeat(60));
        eprintln!("\u{25B6} Processing Gameexe.chs");
        eprintln!("{}", "\u{2501}".repeat(60));
        let gameexe_out = output_dir.join("Gameexe.chs");
        convert_gameexe(&gameexe_path, &gameexe_out, &work_dir, &converter)
            .context("Failed to convert Gameexe")?;
        eprintln!();
    }

    let elapsed = start.elapsed();
    eprintln!("{}", "=".repeat(60));
    eprintln!("  Done! Elapsed: {:.1}s", elapsed.as_secs_f64());
    eprintln!("  Output: {}", output_dir.display());
    eprintln!();
    eprintln!("  Copy files from output/ back to the game directory.");
    eprintln!("{}", "=".repeat(60));
    Ok(())
}
