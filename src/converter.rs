use std::collections::HashMap;

pub struct Converter {
    // Phrase dictionaries applied as conversion chain passes.
    // Each pass: segment input with longest-match, then map matched segments.
    passes: Vec<DictPass>,
}

struct DictPass {
    map: HashMap<String, String>,
    max_key_len: usize,
}

impl DictPass {
    fn from_text(text: &str) -> Self {
        let mut map = HashMap::new();
        let mut max_key_len = 0;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Format: "key\tvalue1 value2 ..." — take first value
            if let Some((key, vals)) = line.split_once('\t') {
                let val = vals.split_whitespace().next().unwrap_or(key);
                let klen = key.chars().count();
                if klen > max_key_len {
                    max_key_len = klen;
                }
                map.insert(key.to_string(), val.to_string());
            }
        }
        DictPass { map, max_key_len }
    }

    fn convert(&self, input: &str) -> String {
        if self.map.is_empty() {
            return input.to_string();
        }
        let chars: Vec<char> = input.chars().collect();
        let len = chars.len();
        let mut result = String::with_capacity(input.len());
        let mut i = 0;
        while i < len {
            let remaining = len - i;
            let max_try = std::cmp::min(remaining, self.max_key_len);
            let mut matched = false;
            // Forward maximum matching
            for try_len in (1..=max_try).rev() {
                let candidate: String = chars[i..i + try_len].iter().collect();
                if let Some(replacement) = self.map.get(&candidate) {
                    result.push_str(replacement);
                    i += try_len;
                    matched = true;
                    break;
                }
            }
            if !matched {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    }
}

impl Converter {
    pub fn new() -> Self {
        // s2twp conversion chain:
        // Pass 1: STPhrases + STCharacters (Simplified -> Traditional)
        // Pass 2: TWPhrases (China phrases -> Taiwan phrases)
        // Pass 3: TWVariants (China variants -> Taiwan variants)

        let st_phrases = include_str!("../dicts/STPhrases.txt");
        let st_chars = include_str!("../dicts/STCharacters.txt");
        let tw_phrases = include_str!("../dicts/TWPhrases.txt");
        let tw_variants = include_str!("../dicts/TWVariants.txt");

        // Merge STPhrases + STCharacters into one pass (phrases take priority via longer match)
        let mut pass1_text = String::with_capacity(st_phrases.len() + st_chars.len());
        pass1_text.push_str(st_phrases);
        pass1_text.push('\n');
        pass1_text.push_str(st_chars);

        let passes = vec![
            DictPass::from_text(&pass1_text),
            DictPass::from_text(tw_phrases),
            DictPass::from_text(tw_variants),
        ];

        Converter { passes }
    }

    pub fn convert(&self, input: &str) -> String {
        let mut text = input.to_string();
        for pass in &self.passes {
            text = pass.convert(&text);
        }
        text
    }
}
