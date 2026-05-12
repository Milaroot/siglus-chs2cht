use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::crypto::decrypt_string;

struct SSHeader {
    header_length: u32,
    header_list: [u32; 32],
}

impl SSHeader {
    fn unknown_offset(&self) -> u32 {
        self.header_list[0]
    }
    fn index_offset(&self) -> u32 {
        self.header_list[2]
    }
    fn count(&self) -> u32 {
        self.header_list[3]
    }
    fn data_offset(&self) -> u32 {
        self.header_list[4]
    }
}

fn parse_ss_header(data: &[u8]) -> SSHeader {
    let header_length = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let mut header_list = [0u32; 32];
    for i in 0..32 {
        let off = 4 + i * 4;
        header_list[i] = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
    }
    SSHeader {
        header_length,
        header_list,
    }
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

fn has_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        let cp = ch as u32;
        // CJK Unified Ideographs + common fullwidth ranges
        (0x3000..=0x9FFF).contains(&cp)
            || (0xF900..=0xFAFF).contains(&cp)
            || (0xFF00..=0xFFEF).contains(&cp)
            || (0x20000..=0x2FA1F).contains(&cp)
    })
}

pub fn extract_texts(ss_path: &Path) -> Result<Vec<(usize, String)>> {
    let data = fs::read(ss_path)?;
    let header = parse_ss_header(&data);

    let mut offsets = Vec::new();
    let mut lengths = Vec::new();
    let mut pos = header.index_offset() as usize;
    for _ in 0..header.count() {
        let off = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into()?) as usize;
        offsets.push(off);
        lengths.push(len);
        pos += 8;
    }

    let mut texts = Vec::new();
    for i in 0..header.count() as usize {
        if lengths[i] == 0 {
            continue;
        }
        let start = header.data_offset() as usize + offsets[i] * 2;
        let byte_len = lengths[i] * 2;
        let raw = &data[start..start + byte_len];
        let decrypted = decrypt_string(raw, lengths[i], i);
        let text = decode_utf16le(&decrypted);
        if has_cjk(&text) {
            texts.push((i, text));
        }
    }
    Ok(texts)
}

pub fn replace_texts(
    ss_path: &Path,
    new_texts: &[(usize, String)],
    output_path: &Path,
) -> Result<()> {
    let data = fs::read(ss_path)?;
    let header = parse_ss_header(&data);

    let mut offsets = Vec::new();
    let mut lengths = Vec::new();
    let mut pos = header.index_offset() as usize;
    for _ in 0..header.count() {
        let off = u32::from_le_bytes(data[pos..pos + 4].try_into()?) as usize;
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into()?) as usize;
        offsets.push(off);
        lengths.push(len);
        pos += 8;
    }

    let count = header.count() as usize;
    let mut strings: Vec<Vec<u8>> = Vec::with_capacity(count);
    for i in 0..count {
        if lengths[i] == 0 {
            strings.push(Vec::new());
            continue;
        }
        let start = header.data_offset() as usize + offsets[i] * 2;
        let byte_len = lengths[i] * 2;
        strings.push(data[start..start + byte_len].to_vec());
    }

    let ss_data_start = header.unknown_offset() as usize;
    let file_size = data.len();
    let ss_data = &data[ss_data_start..];

    for &(idx, ref text) in new_texts {
        if idx >= count {
            continue;
        }
        let encoded = encode_utf16le(text);
        let new_len = text.encode_utf16().count();
        lengths[idx] = new_len;
        strings[idx] = decrypt_string(&encoded, new_len, idx);
    }

    let mut output = Vec::new();
    output.extend_from_slice(&header.header_length.to_le_bytes());
    output.extend_from_slice(&[0u8; 128]);

    let mut new_offset = 0u32;
    for i in 0..count {
        output.extend_from_slice(&new_offset.to_le_bytes());
        output.extend_from_slice(&(lengths[i] as u32).to_le_bytes());
        new_offset += lengths[i] as u32;
    }

    let offset_dev = ss_data_start as i64 - output.len() as i64;
    output.extend_from_slice(ss_data);

    for s in &strings {
        output.extend_from_slice(s);
    }

    let mut new_header_list = header.header_list;
    new_header_list[0] = (new_header_list[0] as i64 - offset_dev) as u32;
    new_header_list[4] = (file_size as i64 - offset_dev) as u32;
    for i in (6..32).step_by(2) {
        new_header_list[i] = (new_header_list[i] as i64 - offset_dev) as u32;
    }

    // Write header list at offset 4
    for (i, &val) in new_header_list.iter().enumerate() {
        let off = 4 + i * 4;
        output[off..off + 4].copy_from_slice(&val.to_le_bytes());
    }

    fs::write(output_path, &output)?;
    Ok(())
}
