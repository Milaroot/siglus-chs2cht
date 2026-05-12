use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::converter::Converter;
use crate::crypto::{compress, decompress, decrypt1, decrypt4};

pub fn unpack_gameexe(
    gameexe_path: &Path,
    output_path: &Path,
    key: Option<&[u8; 16]>,
) -> Result<()> {
    let raw = fs::read(gameexe_path).context("Failed to read Gameexe")?;
    let need_key = u32::from_le_bytes(raw[4..8].try_into()?) == 1;
    let mut data = decrypt4(&raw[8..]);

    if need_key {
        data = if let Some(k) = key {
            decrypt1(&data, k)
        } else {
            decrypt1(&data, &[0u8; 16])
        };
    }

    let decomp_size = u32::from_le_bytes(data[4..8].try_into()?) as usize;
    let plaintext = decompress(&data[8..], decomp_size);

    let mut output = Vec::with_capacity(plaintext.len() + 2);
    output.extend_from_slice(&[0xFF, 0xFE]); // UTF-16LE BOM
    output.extend_from_slice(&plaintext);

    fs::write(output_path, &output)?;
    eprintln!(
        "  Decrypted {} -> {} ({} bytes)",
        gameexe_path.file_name().unwrap().to_string_lossy(),
        output_path.file_name().unwrap().to_string_lossy(),
        decomp_size
    );
    Ok(())
}

pub fn pack_gameexe(
    ini_path: &Path,
    output_path: &Path,
    need_key: bool,
    key: Option<&[u8; 16]>,
) -> Result<()> {
    let mut data = fs::read(ini_path)?;
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xFE {
        data = data[2..].to_vec();
    }

    let compressed = compress(&data);

    let encrypted = if need_key {
        let d1 = if let Some(k) = key {
            decrypt1(&compressed, k)
        } else {
            decrypt1(&compressed, &[0u8; 16])
        };
        decrypt4(&d1)
    } else {
        decrypt4(&compressed)
    };

    let mut output = Vec::new();
    output.extend_from_slice(&[0u8; 4]); // header padding
    if need_key {
        output.extend_from_slice(&1u32.to_le_bytes());
    } else {
        output.extend_from_slice(&0u32.to_le_bytes());
    }
    output.extend_from_slice(&encrypted);

    fs::write(output_path, &output)?;
    eprintln!(
        "  Encrypted {} -> {}",
        ini_path.file_name().unwrap().to_string_lossy(),
        output_path.file_name().unwrap().to_string_lossy()
    );
    Ok(())
}

fn decode_utf16le(data: &[u8]) -> String {
    let u16s: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn encode_utf16le(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 2);
    for c in s.encode_utf16() {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out
}

pub fn convert_gameexe_text(ini_data: &[u8], converter: &Converter) -> Result<Vec<u8>> {
    let mut raw = ini_data;
    if raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xFE {
        raw = &raw[2..];
    }
    let text = decode_utf16le(raw);
    let mut diff_count = 0u32;
    let mut converted_lines = Vec::new();

    for line in text.split('\n') {
        if let Some(eq_pos) = line.find('=') {
            let prefix_end = eq_pos + 1;
            // Check if line starts with #KEY_NAME =
            if line.starts_with('#') {
                let prefix = &line[..prefix_end];
                let rest = &line[prefix_end..];
                let mut parts = Vec::new();
                let mut in_quote = false;
                let mut buf = String::new();

                for ch in rest.chars() {
                    if ch == '"' {
                        if in_quote {
                            let is_file_ext = buf.ends_with(".ttf")
                                || buf.ends_with(".otf")
                                || buf.ends_with(".png")
                                || buf.ends_with(".bmp")
                                || buf.ends_with(".ogg")
                                || buf.ends_with(".wav");
                            if is_file_ext {
                                parts.push(format!("\"{}\"", buf));
                            } else {
                                let conv = converter.convert(&buf);
                                if conv != buf {
                                    diff_count += 1;
                                }
                                parts.push(format!("\"{}\"", conv));
                            }
                            buf.clear();
                            in_quote = false;
                        } else {
                            in_quote = true;
                        }
                    } else if in_quote {
                        buf.push(ch);
                    } else {
                        parts.push(ch.to_string());
                    }
                }
                let mut result = prefix.to_string();
                for p in &parts {
                    result.push_str(p);
                }
                converted_lines.push(result);
                continue;
            }
        }
        converted_lines.push(line.to_string());
    }

    eprintln!("  {} quoted text segments changed", diff_count);
    let result = converted_lines.join("\n");
    Ok(encode_utf16le(&result))
}
