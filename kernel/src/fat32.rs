use crate::ModuleRange;

const FAT32_EOC: u32 = 0x0FFFFFF8;

#[derive(Copy, Clone)]
pub struct DirEntry {
    pub name: [u8; 24],
    pub name_len: usize,
    pub is_dir: bool,
    pub cluster: u32,
    pub size: u32,
}

impl DirEntry {
    pub const EMPTY: DirEntry = DirEntry {
        name: [0u8; 24],
        name_len: 0,
        is_dir: false,
        cluster: 0,
        size: 0,
    };
}

pub struct Fat32<'a> {
    data: &'a [u8],
    bytes_per_sector: usize,
    sectors_per_cluster: usize,
    fat_start: usize,
    fat_size: usize,
    data_start: usize,
    root_cluster: u32,
    total_clusters: u32,
}

impl<'a> Fat32<'a> {
    pub fn new(range: ModuleRange) -> Option<Self> {
        let len = range.end.saturating_sub(range.start);
        if len < 512 {
            return None;
        }
        let data = unsafe { core::slice::from_raw_parts(range.start as *const u8, len) };
        if data[510] != 0x55 || data[511] != 0xAA {
            return None;
        }
        let bytes_per_sector = read_u16_le(data, 11) as usize;
        if bytes_per_sector == 0 || (bytes_per_sector & (bytes_per_sector - 1)) != 0 {
            return None;
        }
        let sectors_per_cluster = data[13] as usize;
        if sectors_per_cluster == 0 {
            return None;
        }
        let reserved_sectors = read_u16_le(data, 14) as usize;
        let fats = data[16] as usize;
        if fats == 0 {
            return None;
        }
        let fat_size16 = read_u16_le(data, 22) as usize;
        let fat_size32 = read_u32_le(data, 36) as usize;
        let fat_size = if fat_size16 != 0 {
            fat_size16
        } else {
            fat_size32
        };
        if fat_size == 0 {
            return None;
        }
        let total16 = read_u16_le(data, 19) as usize;
        let total32 = read_u32_le(data, 32) as usize;
        let total_sectors = if total16 != 0 { total16 } else { total32 };
        if total_sectors == 0 {
            return None;
        }
        let root_cluster = read_u32_le(data, 44);
        if root_cluster < 2 {
            return None;
        }
        let fat_start = reserved_sectors * bytes_per_sector;
        let fat_bytes = fat_size * bytes_per_sector;
        let data_start = fat_start + fat_bytes * fats;
        if data_start >= len {
            return None;
        }
        let data_sectors = total_sectors.saturating_sub(reserved_sectors + fat_size * fats);
        let total_clusters = (data_sectors / sectors_per_cluster) as u32;
        if total_clusters == 0 {
            return None;
        }
        Some(Self {
            data,
            bytes_per_sector,
            sectors_per_cluster,
            fat_start,
            fat_size,
            data_start,
            root_cluster,
            total_clusters,
        })
    }

    pub fn root_cluster(&self) -> u32 {
        self.root_cluster
    }

    pub fn list_dir(&self, start_cluster: u32, out: &mut [DirEntry]) -> usize {
        let mut count = 0usize;
        let mut cluster = start_cluster;
        if cluster < 2 {
            return 0;
        }
        let cluster_size = self.cluster_size();
        loop {
            let offset = match self.cluster_offset(cluster) {
                Some(v) => v,
                None => break,
            };
            let end = offset + cluster_size;
            if end > self.data.len() {
                break;
            }
            let buf = &self.data[offset..end];
            let mut pos = 0usize;
            while pos + 32 <= buf.len() {
                let entry = &buf[pos..pos + 32];
                let first = entry[0];
                if first == 0x00 {
                    return count;
                }
                if first != 0xE5 {
                    let attr = entry[11];
                    if attr != 0x0F && (attr & 0x08) == 0 {
                        let mut name = [0u8; 24];
                        let name_len = sfn_to_name(entry, &mut name);
                        if name_len > 0 && !is_dot_entry(&name, name_len) {
                            if count < out.len() {
                                let cluster_hi = read_u16_le(entry, 20);
                                let cluster_lo = read_u16_le(entry, 26);
                                let cluster_val =
                                    ((cluster_hi as u32) << 16) | cluster_lo as u32;
                                let size = read_u32_le(entry, 28);
                                out[count] = DirEntry {
                                    name,
                                    name_len,
                                    is_dir: (attr & 0x10) != 0,
                                    cluster: cluster_val,
                                    size,
                                };
                                count += 1;
                            }
                        }
                    }
                }
                pos += 32;
            }
            let next = self.fat_entry(cluster);
            if next < 2 || next >= FAT32_EOC {
                break;
            }
            cluster = next;
        }
        count
    }

    pub fn read_file(&self, start_cluster: u32, file_size: usize, out: &mut [u8]) -> usize {
        if start_cluster < 2 || out.is_empty() || file_size == 0 {
            return 0;
        }

        let mut cluster = start_cluster;
        let cluster_size = self.cluster_size();
        let mut written = 0usize;
        let target = file_size.min(out.len());

        while written < target {
            let offset = match self.cluster_offset(cluster) {
                Some(v) => v,
                None => break,
            };
            let end = offset.saturating_add(cluster_size);
            if end > self.data.len() {
                break;
            }
            let src = &self.data[offset..end];
            let remain = target - written;
            let to_copy = remain.min(src.len());
            out[written..written + to_copy].copy_from_slice(&src[..to_copy]);
            written += to_copy;

            if written >= target {
                break;
            }
            let next = self.fat_entry(cluster);
            if next < 2 || next >= FAT32_EOC {
                break;
            }
            cluster = next;
        }

        written
    }

    fn cluster_size(&self) -> usize {
        self.bytes_per_sector * self.sectors_per_cluster
    }

    fn cluster_offset(&self, cluster: u32) -> Option<usize> {
        if cluster < 2 {
            return None;
        }
        let idx = (cluster - 2) as usize;
        let offset = self.data_start + idx * self.cluster_size();
        if offset >= self.data.len() {
            return None;
        }
        Some(offset)
    }

    fn fat_entry(&self, cluster: u32) -> u32 {
        if cluster > self.total_clusters + 1 {
            return FAT32_EOC;
        }
        let offset = self.fat_start + (cluster as usize) * 4;
        if offset + 4 > self.data.len() {
            return FAT32_EOC;
        }
        read_u32_le(self.data, offset) & 0x0FFF_FFFF
    }
}

fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
    if offset + 2 > buf.len() {
        return 0;
    }
    (buf[offset] as u16) | ((buf[offset + 1] as u16) << 8)
}

fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    if offset + 4 > buf.len() {
        return 0;
    }
    (buf[offset] as u32)
        | ((buf[offset + 1] as u32) << 8)
        | ((buf[offset + 2] as u32) << 16)
        | ((buf[offset + 3] as u32) << 24)
}

fn sfn_to_name(entry: &[u8], out: &mut [u8; 24]) -> usize {
    let mut base_end = 8usize;
    while base_end > 0 && entry[base_end - 1] == b' ' {
        base_end -= 1;
    }
    let mut ext_end = 3usize;
    while ext_end > 0 && entry[8 + ext_end - 1] == b' ' {
        ext_end -= 1;
    }
    if base_end == 0 {
        return 0;
    }
    let mut len = 0usize;
    for i in 0..base_end {
        let mut b = entry[i];
        if b == 0x05 {
            b = 0xE5;
        }
        if len < out.len() {
            out[len] = b;
            len += 1;
        }
    }
    if ext_end > 0 && len + 1 < out.len() {
        out[len] = b'.';
        len += 1;
        for i in 0..ext_end {
            if len < out.len() {
                out[len] = entry[8 + i];
                len += 1;
            }
        }
    }
    len
}

fn is_dot_entry(name: &[u8; 24], len: usize) -> bool {
    if len == 1 && name[0] == b'.' {
        return true;
    }
    len == 2 && name[0] == b'.' && name[1] == b'.'
}
