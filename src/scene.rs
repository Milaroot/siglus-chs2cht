use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result};

use crate::crypto::{compress, decompress, decrypt1, decrypt2};

struct SceneHeader {
    var_info_offset: u32,
    scene_name_index_offset: u32,
    scene_name_index_count: u32,
    scene_name_offset: u32,
    scene_name_count: u32,
    scene_info_offset: u32,
    scene_info_count: u32,
    scene_data_offset: u32,
    scene_data_count: u32,
    extra_key_use: u32,
}

fn read_u32(cur: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut buf = [0u8; 4];
    cur.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn parse_header(data: &[u8]) -> Result<SceneHeader> {
    let mut cur = Cursor::new(data);
    cur.seek(SeekFrom::Start(4))?;
    let var_info_offset = read_u32(&mut cur)?;
    let _var_info_count = read_u32(&mut cur)?;
    let _var_name_index_offset = read_u32(&mut cur)?;
    let _var_name_index_count = read_u32(&mut cur)?;
    let _var_name_offset = read_u32(&mut cur)?;
    let _var_name_count = read_u32(&mut cur)?;
    let _cmd_info_offset = read_u32(&mut cur)?;
    let _cmd_info_count = read_u32(&mut cur)?;
    let _cmd_name_index_offset = read_u32(&mut cur)?;
    let _cmd_name_index_count = read_u32(&mut cur)?;
    let _cmd_name_offset = read_u32(&mut cur)?;
    let _cmd_name_count = read_u32(&mut cur)?;
    let scene_name_index_offset = read_u32(&mut cur)?;
    let scene_name_index_count = read_u32(&mut cur)?;
    let scene_name_offset = read_u32(&mut cur)?;
    let scene_name_count = read_u32(&mut cur)?;
    let scene_info_offset = read_u32(&mut cur)?;
    let scene_info_count = read_u32(&mut cur)?;
    let scene_data_offset = read_u32(&mut cur)?;
    let scene_data_count = read_u32(&mut cur)?;
    let extra_key_use = read_u32(&mut cur)?;

    Ok(SceneHeader {
        var_info_offset,
        scene_name_index_offset,
        scene_name_index_count,
        scene_name_offset,
        scene_name_count,
        scene_info_offset,
        scene_info_count,
        scene_data_offset,
        scene_data_count,
        extra_key_use,
    })
}

fn decode_utf16le(data: &[u8]) -> String {
    let u16s: Vec<u16> = data
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

pub fn unpack_scene(
    scene_path: &Path,
    output_dir: &Path,
    key: Option<&[u8; 16]>,
) -> Result<Vec<(String, std::path::PathBuf)>> {
    fs::create_dir_all(output_dir)?;
    let data = fs::read(scene_path).context("Failed to read scene file")?;
    let header = parse_header(&data)?;

    let mut name_offsets = Vec::new();
    let mut name_lengths = Vec::new();
    let mut pos = header.scene_name_index_offset as usize;
    for _ in 0..header.scene_name_index_count {
        let off = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into()?) as usize;
        name_offsets.push(off);
        name_lengths.push(len);
        pos += 8;
    }

    let mut scene_names = Vec::new();
    pos = header.scene_name_offset as usize;
    for i in 0..header.scene_name_count as usize {
        let byte_len = name_lengths[i] * 2;
        let name = decode_utf16le(&data[pos..pos + byte_len]);
        scene_names.push(name);
        pos += byte_len;
    }

    let mut data_offsets = Vec::new();
    let mut data_lengths = Vec::new();
    pos = header.scene_info_offset as usize;
    for _ in 0..header.scene_info_count {
        let off = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into()?) as usize;
        data_offsets.push(off);
        data_lengths.push(len);
        pos += 8;
    }

    let mut results = Vec::new();
    for i in 0..header.scene_data_count as usize {
        let start = header.scene_data_offset as usize + data_offsets[i];
        let raw = &data[start..start + data_lengths[i]];

        let decrypted = if header.extra_key_use != 0 {
            let d1 = if let Some(k) = key {
                decrypt1(raw, k)
            } else {
                decrypt1(raw, &[0u8; 16])
            };
            decrypt2(&d1, 0)
        } else {
            decrypt2(raw, 0)
        };

        let decomp_size = u32::from_le_bytes(decrypted[4..8].try_into()?) as usize;
        let ss_data = decompress(&decrypted[8..], decomp_size);

        let filename = format!("{}.ss", &scene_names[i]);
        let filepath = output_dir.join(&filename);
        fs::write(&filepath, &ss_data)?;
        results.push((filename, filepath));
    }

    eprintln!(
        "  Unpacked {} scripts from {}",
        results.len(),
        scene_path.file_name().unwrap().to_string_lossy()
    );
    Ok(results)
}

pub fn pack_scene(
    original_scene_path: &Path,
    ss_dir: &Path,
    output_path: &Path,
    key: Option<&[u8; 16]>,
) -> Result<()> {
    let data = fs::read(original_scene_path)?;
    let header = parse_header(&data)?;

    let mut name_lengths = Vec::new();
    let mut pos = header.scene_name_index_offset as usize;
    for _ in 0..header.scene_name_index_count {
        let _off = u32::from_le_bytes(data[pos..pos + 4].try_into()?);
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into()?) as usize;
        name_lengths.push(len);
        pos += 8;
    }

    let mut scene_names = Vec::new();
    pos = header.scene_name_offset as usize;
    for i in 0..header.scene_name_count as usize {
        let byte_len = name_lengths[i] * 2;
        let name = decode_utf16le(&data[pos..pos + byte_len]);
        scene_names.push(name);
        pos += byte_len;
    }

    let mut data_offsets_orig = Vec::new();
    let mut data_lengths_orig = Vec::new();
    pos = header.scene_info_offset as usize;
    for _ in 0..header.scene_info_count {
        let off = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into()?) as usize;
        data_offsets_orig.push(off);
        data_lengths_orig.push(len);
        pos += 8;
    }

    let mut scene_data_blocks: Vec<Vec<u8>> = Vec::new();
    for i in 0..header.scene_data_count as usize {
        let start = header.scene_data_offset as usize + data_offsets_orig[i];
        scene_data_blocks.push(data[start..start + data_lengths_orig[i]].to_vec());
    }

    let mut new_data_lengths = data_lengths_orig.clone();

    for i in 0..header.scene_data_count as usize {
        let filename = format!("{}.ss", &scene_names[i]);
        let filepath = ss_dir.join(&filename);
        if !filepath.is_file() {
            continue;
        }

        let ss_data = fs::read(&filepath)?;
        let compressed = compress(&ss_data);

        let encrypted = if header.extra_key_use != 0 {
            let d1 = if let Some(k) = key {
                decrypt1(&compressed, k)
            } else {
                decrypt1(&compressed, &[0u8; 16])
            };
            decrypt2(&d1, 0)
        } else {
            decrypt2(&compressed, 0)
        };

        scene_data_blocks[i] = encrypted;
        new_data_lengths[i] = scene_data_blocks[i].len();
    }

    let prefix = &data[..header.scene_info_offset as usize];
    let mut output = Vec::new();
    output.extend_from_slice(prefix);

    let mut offset = 0u32;
    for i in 0..header.scene_info_count as usize {
        output.extend_from_slice(&offset.to_le_bytes());
        output.extend_from_slice(&(new_data_lengths[i] as u32).to_le_bytes());
        offset += new_data_lengths[i] as u32;
    }
    for block in &scene_data_blocks {
        output.extend_from_slice(block);
    }

    fs::write(output_path, &output)?;
    eprintln!(
        "  Packed {} scripts into {}",
        header.scene_data_count,
        output_path.file_name().unwrap().to_string_lossy()
    );
    Ok(())
}
