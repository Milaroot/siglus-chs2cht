# siglus-chs2cht

> SiglusEngine 簡體中文 → 繁體中文（臺灣用語）轉換工具

將 SiglusEngine 遊戲的 **簡體中文** 文本自動轉換為 **繁體中文**。

![](img\demo.png)

純 Rust 實作，零外部依賴，單一執行檔即可運行。
```
關於圖像處理的說明：
圖像部分的翻譯因涉及字型選擇與配色細節，必須透過人工細緻處理與驗證。但在執行過程中，意外接觸到尚未體驗的劇情內容，對本人造成了不小的心理衝擊。為了維持對這部作品的開發熱情與最佳的處理狀態，決定暫時給予自己一點緩衝時間，稍作休整後再行恢復專案進度。
```
---

## 功能

| 檔案 | 處理內容 |
|------|----------|
| `scene.chs` | 所有腳本對白、角色名、選項、系統文字 |
| `Gameexe.chs` | UI 選單文字（保存→儲存、加载→載入 等） |

## 轉換方式

採用與 [OpenCC](https://github.com/BYVoid/OpenCC) 相同的 `s2twp` 轉換鏈：

1. **簡→繁字元 + 詞組**（STCharacters + STPhrases）
2. **中國用語→臺灣用語**（TWPhrases）
3. **異體字變體**（TWVariants）

字典檔內嵌於執行檔中，不需要額外安裝 OpenCC。

## 安裝

### 從原始碼編譯

需要 [Rust](https://rustup.rs/) 工具鏈（1.70+）。

```bash
git clone https://github.com/Milaroot/siglus-chs2cht.git
cd siglus-chs2cht
cargo build --release
```

編譯完成後，執行檔位於 `target/release/siglus-chs2cht`（Windows 為 `.exe`）。


## 使用方法

### 方法一：使用 input 目錄

1. 在執行檔旁建立 `input/` 目錄
2. 將 `scene.chs` 和 `Gameexe.chs` 放入 `input/`
3. 執行：

```bash
./siglus-chs2cht
```

4. 轉換後的檔案在 `output/` 目錄

### 方法二：指定遊戲目錄

```bash
./siglus-chs2cht "C:\path\to\game"
```

### 轉換完成後

將 `output/` 中的 `scene.chs` 和 `Gameexe.chs` 複製回遊戲目錄，覆蓋原檔即可。

## 技術說明

- **純 Rust 實作** — SiglusEngine 的 XOR 加密/解密與 LZSS 壓縮/解壓全部以 Rust 實作
- **不需要外部工具** — 不需要 `Decryption.dll`、`skf.exe` 或 OpenCC
- **字典內嵌** — OpenCC `s2twp` 字典檔（STCharacters、STPhrases、TWPhrases、TWVariants）直接編譯進執行檔
- **前向最大匹配** — 文本轉換使用 Forward Maximum Matching 演算法，與 OpenCC 的 mmseg 分詞一致

## TODO

### Game-Specific Key 支援

目前工具僅使用引擎通用的 256-byte XOR key table（`KEY_TABLE_0` / `KEY_TABLE_1`）進行解密。

SiglusEngine 的加密分為兩層：

| 層級 | Key | 範圍 | 本工具支援 |
|------|-----|------|-----------|
| `decrypt2` / `decrypt4` | `KEY_TABLE_0` / `KEY_TABLE_1`（256-byte，寫死在SiglusEngine 裡） | 所有 SiglusEngine 遊戲通用 |  已實作 |
| `decrypt1` | 16-byte game-specific key（從各遊戲執行檔提取） | 每款遊戲不同 |  尚未實作 |

當 `scene.chs` header 中的 `extra_key_use != 0` 時，資料會額外套一層 `decrypt1`（16-byte XOR），此 key 為遊戲專屬，需從該遊戲的 `.exe` 中提取。目前未提供 game key 時會以全零 key 解密，對有此加密的遊戲將無法正確處理。


### 圖檔文字處理

`g00` type 2 格式的按鈕圖片（如 `g00/*.g00`、`*.chg`）中的文字是直接弄到圖檔中的，無法透過字串替換轉換。處理流程需要：

1. 解碼 g00 type 2 複合圖片，拆出各 region 的 BGRA tile
2. 辨識含有簡體中文文字的 region
3. 以對應的繁體中文重新渲染文字覆蓋（涉及字型選擇、配色、描邊等細節）
4. 重新編碼為 g00 格式

此部分因涉及字型與排版的人工調整，暫時擱置。

## 未涵蓋

- `dat/*.ttf` 字型檔（建議替換為 TC 版本）

## 授權

本工具以 MIT 授權發佈。

OpenCC 字典資料來自 [BYVoid/OpenCC](https://github.com/BYVoid/OpenCC)，以 Apache License 2.0 授權。
