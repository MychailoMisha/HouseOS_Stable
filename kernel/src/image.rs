// Власний JPEG-декодер для no_std без alloc.
// Підтримує Baseline JPEG, 8 біт, YCbCr -> BGRA, з урахуванням субсемплінгу (4:4:4, 4:2:2, 4:2:0).

const MAX_WIDTH: usize = 1024;
const MAX_HEIGHT: usize = 768;
const MAX_PIXELS: usize = MAX_WIDTH * MAX_HEIGHT;

// Глобальні буфери (static mut для no_std середовища)
static mut JPEG_BUF: [u8; 8 + MAX_PIXELS * 4] = [0; 8 + MAX_PIXELS * 4];
static mut BUF_LEN: usize = 0;

static mut Y_PLANE: [u8; MAX_PIXELS] = [0; MAX_PIXELS];
static mut CB_PLANE: [u8; MAX_PIXELS] = [0; MAX_PIXELS];
static mut CR_PLANE: [u8; MAX_PIXELS] = [0; MAX_PIXELS];

// ----------------------------- BitReader -----------------------------
// Оригінальна логіка: Byte Stuffing 0xFF 0x00, без RST
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bit_pos: i8,
    current_byte: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            bit_pos: -1,
            current_byte: 0,
        }
    }

    fn read_bit(&mut self) -> Option<u8> {
        if self.bit_pos < 0 {
            if self.pos >= self.data.len() { return None; }
            self.current_byte = self.data[self.pos];
            self.pos += 1;

            // JPEG Byte Stuffing
            if self.current_byte == 0xFF {
                if self.pos < self.data.len() && self.data[self.pos] == 0x00 {
                    self.pos += 1;
                } else {
                    // зустріли реальний маркер (Restart/EOI/інший) — кінець скану
                    return None;
                }
            }
            self.bit_pos = 7;
        }
        let bit = (self.current_byte >> self.bit_pos) & 1;
        self.bit_pos -= 1;
        Some(bit)
    }

    fn read_bits(&mut self, n: u8) -> Option<u16> {
        let mut val: u16 = 0;
        for _ in 0..n {
            val = (val << 1) | (self.read_bit()? as u16);
        }
        Some(val)
    }

    fn align_to_byte(&mut self) {
        self.bit_pos = -1;
    }
}

// ----------------------------- Huffman -----------------------------
struct HuffmanTable {
    max_code: [i32; 17],
    offset: [i32; 17],
    values: [u8; 256],
}

impl HuffmanTable {
    fn new(bits: &[u8; 16], values: &[u8]) -> Self {
        let mut h = HuffmanTable {
            max_code: [-1; 17],
            offset: [0; 17],
            values: [0; 256],
        };
        let mut code = 0i32;
        let mut val_idx = 0;
        for i in 1..=16 {
            let count = bits[i - 1] as usize;
            if count > 0 {
                h.offset[i] = val_idx as i32 - code;
                code += count as i32;
                h.max_code[i] = code - 1;
                h.values[val_idx..val_idx + count].copy_from_slice(&values[val_idx..val_idx + count]);
                val_idx += count;
            }
            code <<= 1;
        }
        h
    }

    fn decode(&self, reader: &mut BitReader) -> Option<u8> {
        let mut code = 0i32;
        for len in 1..=16 {
            code = (code << 1) | (reader.read_bit()? as i32);
            if code <= self.max_code[len] {
                return Some(self.values[(code + self.offset[len]) as usize]);
            }
        }
        None
    }
}

// ----------------------------- IDCT (виправлена) -----------------------------
// Точна таблиця для 1D IDCT (8x8).
// Коефіцієнти: u=0 -> 4096; u>0 -> round( sqrt(2) * cos((2*x+1)*u*PI/16) * 4096 )
const IIDCT_TABLE: [[i32; 8]; 8] = [
    [4096,  5681,  5352,  4816,  4096,  3218,  2217,  1130],
    [4096,  4816,  2217, -1130, -4096, -5681, -5352, -3218],
    [4096,  3218, -2217, -5681, -4096,  1130,  5352,  4816],
    [4096,  1130, -5352, -3218,  4096,  4816, -2217, -5681],
    [4096, -1130, -5352,  3218,  4096, -4816, -2217,  5681],
    [4096, -3218, -2217,  5681, -4096, -1130,  5352, -4816],
    [4096, -4816,  2217,  1130, -4096,  5681, -5352,  3218],
    [4096, -5681,  5352, -4816,  4096, -3218,  2217, -1130],
];

#[inline]
fn idct_1d(d: &mut [i32; 8]) {
    let mut tmp = [0i32; 8];
    for x in 0..8 {
        let mut sum = 0i32;
        let row = &IIDCT_TABLE[x];
        for u in 0..8 {
            sum += d[u] * row[u];
        }
        // округлення та зсув на 12 (4096) після множення
        tmp[x] = (sum + 2048) >> 12;
    }
    *d = tmp;
}

fn idct(block: &mut [i32; 64]) {
    // 1D IDCT по рядках
    for i in 0..8 {
        let start = i * 8;
        let row = &mut block[start..start + 8];
        let mut arr = [0i32; 8];
        arr.copy_from_slice(row);
        idct_1d(&mut arr);
        row.copy_from_slice(&arr);
    }

    // 1D IDCT по стовпцях
    for i in 0..8 {
        let mut col = [0i32; 8];
        for j in 0..8 {
            col[j] = block[j * 8 + i];
        }
        idct_1d(&mut col);
        for j in 0..8 {
            block[j * 8 + i] = col[j];
        }
    }
}

// ----------------------------- Декодер -----------------------------
struct JpegDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    width: u16,
    height: u16,
    qtables: [[u16; 64]; 4],
    dc_huff: [Option<HuffmanTable>; 2],
    ac_huff: [Option<HuffmanTable>; 2],
    components: [Option<Component>; 3],
    mc_w: u8,
    mc_h: u8,
}

#[derive(Clone, Copy)]
struct Component {
    h: u8,
    v: u8,
    qt: u8,
    dc: u8,
    ac: u8,
}

impl<'a> JpegDecoder<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 { return None; }
        Some(JpegDecoder {
            data,
            pos: 2,
            width: 0,
            height: 0,
            qtables: [[0; 64]; 4],
            dc_huff: [None, None],
            ac_huff: [None, None],
            components: [None; 3],
            mc_w: 0,
            mc_h: 0,
        })
    }

    fn read_u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn read_u16(&mut self) -> Option<u16> {
        Some(((self.read_u8()? as u16) << 8) | self.read_u8()? as u16)
    }

    fn parse(&mut self) -> Option<()> {
        loop {
            if self.pos >= self.data.len() { return None; }
            if self.read_u8()? != 0xFF { continue; }
            let marker = self.read_u8()?;
            match marker {
                0xDB => { // DQT
                    let len = self.read_u16()? as usize;
                    let end = self.pos + len - 2;
                    while self.pos < end {
                        let info = self.read_u8()?;
                        let id = (info & 0x0F) as usize;
                        for i in 0..64 {
                            self.qtables[id][ZIGZAG[i]] = self.read_u8()? as u16;
                        }
                    }
                }
                0xC0..=0xC2 => { // SOF
                    let _len = self.read_u16()?;
                    self.pos += 1; // precision
                    self.height = self.read_u16()?;
                    self.width = self.read_u16()?;
                    let n = self.read_u8()?;
                    for _ in 0..n {
                        let id = self.read_u8()?;
                        let s = self.read_u8()?;
                        let q = self.read_u8()?;
                        let (h, v) = (s >> 4, s & 0x0F);
                        self.components[(id - 1) as usize] = Some(Component { h, v, qt: q, dc: 0, ac: 0 });
                        if h > self.mc_w { self.mc_w = h; }
                        if v > self.mc_h { self.mc_h = v; }
                    }
                }
                0xC4 => { // DHT
                    let len = self.read_u16()? as usize;
                    let end = self.pos + len - 2;
                    while self.pos < end {
                        let info = self.read_u8()?;
                        let mut bits = [0u8; 16];
                        for b in &mut bits { *b = self.read_u8()?; }
                        let count: usize = bits.iter().map(|&b| b as usize).sum();
                        let mut vals = [0u8; 256];
                        for i in 0..count { vals[i] = self.read_u8()?; }
                        let tab = Some(HuffmanTable::new(&bits, &vals[..count]));
                        if info >> 4 == 0 {
                            self.dc_huff[(info & 0xF) as usize] = tab;
                        } else {
                            self.ac_huff[(info & 0xF) as usize] = tab;
                        }
                    }
                }
                0xDA => { // SOS
                    let _len = self.read_u16()?;
                    let n = self.read_u8()?;
                    for _ in 0..n {
                        let id = self.read_u8()?;
                        let t = self.read_u8()?;
                        if let Some(c) = &mut self.components[(id - 1) as usize] {
                            c.dc = t >> 4;
                            c.ac = t & 0xF;
                        }
                    }
                    self.pos += 3; // skip Ss, Se, Ah/Al
                    return self.decode_scan();
                }
                0xD9 => return Some(()), // EOI
                _ => {
                    if let Some(l) = self.read_u16() {
                        self.pos += l as usize - 2;
                    }
                }
            }
        }
    }

    fn decode_scan(&mut self) -> Option<()> {
        let mut reader = BitReader::new(&self.data[self.pos..]);
        let mut prev_dc = [0i32; 3];
        let (mcu_w, mcu_h) = (self.mc_w as usize * 8, self.mc_h as usize * 8);
        let (cols, rows) = (
            (self.width as usize + mcu_w - 1) / mcu_w,
            (self.height as usize + mcu_h - 1) / mcu_h,
        );

        for my in 0..rows {
            for mx in 0..cols {
                for i in 0..3 {
                    let comp = self.components[i]?;
                    for v in 0..comp.v {
                        for h in 0..comp.h {
                            let mut block = [0i32; 64];
                            self.decode_block(&mut reader, i, &mut prev_dc[i], &mut block)?;
                            idct(&mut block);
                            self.render_to_plane(i, mx, my, h, v, &block);
                        }
                    }
                }
            }
        }
        self.finalize_rgb()
    }

    fn decode_block(&self, r: &mut BitReader, idx: usize, dc: &mut i32, out: &mut [i32; 64]) -> Option<()> {
        let c = self.components[idx]?;
        let dc_t = self.dc_huff[c.dc as usize].as_ref()?;
        let ac_t = self.ac_huff[c.ac as usize].as_ref()?;
        let q = &self.qtables[c.qt as usize];

        let cat = dc_t.decode(r)?;
        if cat > 0 {
            let bits = r.read_bits(cat)? as i32;
            *dc += if bits < (1 << (cat - 1)) {
                bits - (1 << cat) + 1
            } else {
                bits
            };
        }
        out[0] = *dc * q[0] as i32;

        let mut k = 1;
        while k < 64 {
            let sym = ac_t.decode(r)?;
            if sym == 0 { break; }
            k += (sym >> 4) as usize;
            let cat = sym & 0xF;
            if cat > 0 && k < 64 {
                let bits = r.read_bits(cat)? as i32;
                let val = if bits < (1 << (cat - 1)) {
                    bits - (1 << cat) + 1
                } else {
                    bits
                };
                out[ZIGZAG[k]] = val * q[ZIGZAG[k]] as i32;
            }
            k += 1;
        }
        Some(())
    }

    // Врахування субсемплінгу: кожен коефіцієнт Cb/Cr розтягується на hfac×vfac пікселів
    fn render_to_plane(&self, c_idx: usize, mx: usize, my: usize, h: u8, v: u8, block: &[i32; 64]) {
        let comp = self.components[c_idx].unwrap();
        let max_h = self.mc_w as usize;
        let max_v = self.mc_h as usize;
        let h_samp = comp.h as usize;
        let v_samp = comp.v as usize;

        let hfac = max_h / h_samp;
        let vfac = max_v / v_samp;

        let mcu_px_w = max_h * 8;
        let mcu_px_h = max_v * 8;

        let x_off = mx * mcu_px_w + (h as usize * 8 * hfac);
        let y_off = my * mcu_px_h + (v as usize * 8 * vfac);

        let w = self.width as usize;
        let h_img = self.height as usize;

        for y in 0..8 {
            for x in 0..8 {
                let val = ((block[y * 8 + x] >> 3) + 128).clamp(0, 255) as u8;

                for yy in 0..vfac {
                    for xx in 0..hfac {
                        let px = x_off + x * hfac + xx;
                        let py = y_off + y * vfac + yy;
                        if px < w && py < h_img {
                            let idx = py * w + px;
                            unsafe {
                                if c_idx == 0 {
                                    Y_PLANE[idx] = val;
                                } else if c_idx == 1 {
                                    CB_PLANE[idx] = val;
                                } else {
                                    CR_PLANE[idx] = val;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn finalize_rgb(&self) -> Option<()> {
        let (w, h) = (self.width as usize, self.height as usize);
        unsafe {
            JPEG_BUF[0..4].copy_from_slice(&(w as u32).to_le_bytes());
            JPEG_BUF[4..8].copy_from_slice(&(h as u32).to_le_bytes());
            for i in 0..(w * h) {
                let y = Y_PLANE[i] as i32;
                let cb = CB_PLANE[i] as i32 - 128;
                let cr = CR_PLANE[i] as i32 - 128;

                let r = (y + (1402 * cr >> 10)).clamp(0, 255) as u8;
                let g = (y - (344 * cb >> 10) - (714 * cr >> 10)).clamp(0, 255) as u8;
                let b = (y + (1772 * cb >> 10)).clamp(0, 255) as u8;

                let o = 8 + i * 4;
                JPEG_BUF[o..o + 4].copy_from_slice(&[b, g, r, 255]);
            }
            BUF_LEN = 8 + w * h * 4;
        }
        Some(())
    }
}

pub fn decode_jpeg(data: &[u8]) -> bool {
    if let Some(mut dec) = JpegDecoder::new(data) {
        dec.parse().is_some()
    } else {
        false
    }
}

pub fn get_bgra_ptr() -> *const u8 {
    unsafe { JPEG_BUF.as_ptr() }
}

pub fn get_bgra_len() -> usize {
    unsafe { BUF_LEN }
}

static ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5,
    12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];