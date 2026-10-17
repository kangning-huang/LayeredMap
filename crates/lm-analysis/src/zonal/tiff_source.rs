//! A GeoTIFF band read a window at a time, so a summary of a raster of any size (R4's 2 GB COG,
//! R5's 4 GB striped file) never loads the whole file.
//!
//! The file stays open; a window decodes only the strips or tiles it touches, and keeps the
//! decoded ones in a bounded first-in-first-out cache (the summary reads each polygon's bounding
//! box in row strips, so neighbouring windows reuse them). Strips and tiles, chunky or planar
//! bands, 8/16/32/64-bit integer and 32/64-bit float samples, and every compression the `tiff`
//! crate decodes (none, Deflate, LZW, PackBits, Zstandard, with or without a predictor). The
//! coordinate system, georeferencing and NoData value (the GDAL_NODATA tag) come from
//! `rustgis_data::raster::geotiff::georeferencing`, the same header parsing the app's reader uses.
//!
//! Chunks that are uncompressed or compressed with LZW, Deflate or PackBits are decoded here
//! (decompressed in one pass, then the predictor and byte order undone); the `tiff` crate
//! (0.11.3) decodes the rest (Zstandard, JPEG…), with its panics caught. Its own chunk reader
//! fails on valid GDAL files that the reader tests reproduce: "no lzw end code found" on some LZW
//! chunks it reads a row at a time, and an assertion panic on an uncompressed tile at the image's
//! edge.
//!
//! Only the full-resolution image is read: a summary never uses overviews. Raw values are kept as
//! stored (converted to `f32`); NoData is decided by [`Grid::is_valid`].
//!
//! Files are hostile input: a strip or tile larger than [`MAX_CHUNK_SAMPLES`] is refused, and the
//! decoder's own limits bound everything else.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tiff::ColorType;
use tiff::decoder::{ChunkType, Decoder, DecodingResult, Limits};
use tiff::tags::Tag;

use super::{Grid, RasterSource, ZonalError};

/// The largest strip or tile read (samples, all bands of a chunky file): 16 M, 64–128 MB decoded.
pub const MAX_CHUNK_SAMPLES: usize = 16 << 20;
/// Decoded samples kept by default: 32 M (128 MB), about three rows of 512 px tiles across R4.
pub const DEFAULT_CACHE_SAMPLES: usize = 32 << 20;

fn read_err(e: impl std::fmt::Display) -> ZonalError {
    ZonalError::Read(e.to_string())
}

/// One decoded strip or tile of the band: `width × rows` values, row by row.
struct Chunk {
    width: usize,
    rows: usize,
    values: Vec<f32>,
}

/// What decoding a chunk here needs.
struct Raw {
    offsets: Vec<u64>,
    counts: Vec<u64>,
    /// The Compression tag: 1 none, 5 LZW, 8 or 32946 Deflate, 32773 PackBits.
    compression: u16,
    /// Bytes per sample (1, 2, 4 or 8) and the SampleFormat tag (1 unsigned, 2 signed, 3 float).
    bytes: usize,
    format: u16,
    predictor: u16,
    big_endian: bool,
    tiled: bool,
}

/// One band of a GeoTIFF on disk.
pub struct TiffSource {
    grid: Grid,
    dec: Decoder<BufReader<File>>,
    /// Full strip or tile size (a strip is the image's width × rows per strip).
    chunk_w: u32,
    chunk_h: u32,
    across: u32,
    chunks_per_plane: u32,
    band: usize,
    samples: usize,
    planar: bool,
    data_type: &'static str,
    /// For the compressions decoded here: where each chunk is, and how to undo its encoding.
    raw: Option<Raw>,
    cache: HashMap<u32, Chunk>,
    order: VecDeque<u32>,
    cached: usize,
    max_cached: usize,
    decoded: u64,
}

impl TiffSource {
    /// Band `band` (0-based) of the GeoTIFF at `path`, with no scale or offset.
    pub fn open(path: &Path, band: usize) -> Result<TiffSource, ZonalError> {
        let open = || File::open(path).map_err(|e| ZonalError::Read(format!("{}: {e}", path.display())));
        let (crs, transform, nodata) =
            rustgis_data::raster::geotiff::georeferencing(BufReader::new(open()?)).map_err(|e| ZonalError::Input(e.to_string()))?;
        let mut limits = Limits::default();
        limits.decoding_buffer_size = MAX_CHUNK_SAMPLES * 8;
        limits.intermediate_buffer_size = 128 << 20;
        limits.ifd_value_size = 32 << 20;
        let mut dec = Decoder::new(BufReader::with_capacity(1 << 16, open()?)).map_err(read_err)?.with_limits(limits);
        let (width, height) = dec.dimensions().map_err(read_err)?;
        let (samples, bits) = match dec.colortype().map_err(read_err)? {
            ColorType::Gray(b) => (1, b),
            ColorType::GrayA(b) => (2, b),
            ColorType::RGB(b) => (3, b),
            ColorType::RGBA(b) => (4, b),
            ColorType::CMYK(b) => (4, b),
            ColorType::Multiband { bit_depth, num_samples } => (usize::from(num_samples), bit_depth),
            other => return Err(ZonalError::Input(format!("a {other:?} image holds colours, not values"))),
        };
        if band >= samples {
            return Err(ZonalError::Input(format!("the raster has no band {}", band + 1)));
        }
        // One SampleFormat per band; a band of a different format isn't supported.
        let formats = dec.find_tag_unsigned_vec::<u16>(Tag::SampleFormat).map_err(read_err)?.unwrap_or_default();
        if formats.windows(2).any(|w| w[0] != w[1]) {
            return Err(ZonalError::Input("bands of different sample formats can't be summarized".into()));
        }
        let format = formats.first().copied().unwrap_or(1);
        let data_type = match (format, bits) {
            (1, 8) => "UInt8",
            (2, 8) => "Int8",
            (1, 16) => "UInt16",
            (2, 16) => "Int16",
            (1, 32) => "UInt32",
            (2, 32) => "Int32",
            (1, 64) => "UInt64",
            (2, 64) => "Int64",
            (3, 32) => "Float32",
            (3, 64) => "Float64",
            (f, b) => return Err(ZonalError::Input(format!("{b}-bit samples of format {f} can't be summarized"))),
        };
        let planar = dec.find_tag_unsigned::<u16>(Tag::PlanarConfiguration).map_err(read_err)?.unwrap_or(1) == 2;
        let (chunk_w, chunk_h) = dec.chunk_dimensions();
        if chunk_w == 0 || chunk_h == 0 {
            return Err(ZonalError::Input("the file's strips or tiles have no size".into()));
        }
        let per_pixel = if planar { 1 } else { samples };
        if (chunk_w as usize).saturating_mul(chunk_h as usize).saturating_mul(per_pixel) > MAX_CHUNK_SAMPLES {
            return Err(ZonalError::Input(format!(
                "strips or tiles of {chunk_w} × {chunk_h} pixels are too large to read a window at a time"
            )));
        }
        let across = width.div_ceil(chunk_w);
        let chunks_per_plane = across.checked_mul(height.div_ceil(chunk_h)).ok_or_else(|| ZonalError::Input("too many tiles".into()))?;
        let compression = dec.find_tag_unsigned::<u16>(Tag::Compression).map_err(read_err)?.unwrap_or(1);
        let tiled = dec.get_chunk_type() == ChunkType::Tile;
        let predictor = dec.find_tag_unsigned::<u16>(Tag::Predictor).map_err(read_err)?.unwrap_or(1);
        let raw = if matches!(compression, 1 | 5 | 8 | 32946 | 32773) && matches!(predictor, 1..=3) {
            let (ot, ct) = if tiled { (Tag::TileOffsets, Tag::TileByteCounts) } else { (Tag::StripOffsets, Tag::StripByteCounts) };
            let offsets = dec.find_tag_unsigned_vec::<u64>(ot).map_err(read_err)?.unwrap_or_default();
            let counts = dec.find_tag_unsigned_vec::<u64>(ct).map_err(read_err)?.unwrap_or_default();
            let planes = if planar { samples as u64 } else { 1 };
            if (offsets.len() as u64) < u64::from(chunks_per_plane) * planes || counts.len() < offsets.len() {
                return Err(ZonalError::Input("the file's strip or tile index is shorter than the image".into()));
            }
            Some(Raw {
                offsets,
                counts,
                compression,
                bytes: usize::from(bits / 8),
                format,
                predictor,
                big_endian: dec.byte_order() == tiff::tags::ByteOrder::BigEndian,
                tiled,
            })
        } else {
            None
        };
        let grid = Grid { width, height, transform, crs, nodata, scale: 1.0, offset: 0.0 };
        Ok(TiffSource {
            grid,
            dec,
            chunk_w,
            chunk_h,
            across,
            chunks_per_plane,
            band,
            samples,
            planar,
            data_type,
            raw,
            cache: HashMap::new(),
            order: VecDeque::new(),
            cached: 0,
            max_cached: DEFAULT_CACHE_SAMPLES,
            decoded: 0,
        })
    }

    /// The band's values are raw × `scale` + `offset`.
    pub fn with_scale_offset(mut self, scale: f64, offset: f64) -> TiffSource {
        self.grid.scale = scale;
        self.grid.offset = offset;
        self
    }

    /// Keep at most `samples` decoded values (at least one strip or tile is always kept).
    pub fn with_cache_samples(mut self, samples: usize) -> TiffSource {
        self.max_cached = samples;
        self
    }

    /// The stored sample type, as GDAL names it ("Float32", "Int16", "UInt8"…).
    pub fn data_type(&self) -> &'static str {
        self.data_type
    }

    /// Strip or tile size in pixels, and whether the file is tiled (a strip is as wide as the image).
    pub fn chunk_size(&self) -> (u32, u32) {
        (self.chunk_w, self.chunk_h)
    }

    /// How many strips or tiles have been decoded so far (cache misses).
    pub fn chunks_decoded(&self) -> u64 {
        self.decoded
    }

    fn chunk(&mut self, index: u32) -> Result<&Chunk, ZonalError> {
        if !self.cache.contains_key(&index) {
            let file_index = if self.planar { index + self.band as u32 * self.chunks_per_plane } else { index };
            let (dw, dh) = (
                (self.grid.width - (index % self.across) * self.chunk_w).min(self.chunk_w) as usize,
                (self.grid.height - (index / self.across) * self.chunk_h).min(self.chunk_h) as usize,
            );
            let per_pixel = if self.planar { 1 } else { self.samples };
            let all = match &self.raw {
                Some(l) => {
                    let rows = if l.tiled { self.chunk_h as usize } else { dh };
                    decode_raw(&mut self.dec, l, file_index, self.chunk_w as usize, per_pixel, rows)?
                }
                None => {
                    let dec = &mut self.dec;
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dec.read_chunk(file_index)))
                        .map_err(|_| ZonalError::Read(format!("strip or tile {index} couldn't be decoded")))?;
                    to_f32(r.map_err(read_err)?)
                }
            };
            // A tile comes back cropped to the image; accept a padded one as well.
            let width =
                if all.len() >= self.chunk_w as usize * dh * per_pixel && dw < self.chunk_w as usize { self.chunk_w as usize } else { dw };
            if all.len() < width * dh * per_pixel {
                return Err(ZonalError::Read(format!("strip or tile {index} is shorter than its size")));
            }
            let values: Vec<f32> = if per_pixel == 1 {
                let mut v = all;
                v.truncate(width * dh);
                v
            } else {
                all.iter().skip(self.band).step_by(per_pixel).take(width * dh).copied().collect()
            };
            self.decoded += 1;
            while self.cached + values.len() > self.max_cached
                && let Some(old) = self.order.pop_front()
            {
                if let Some(c) = self.cache.remove(&old) {
                    self.cached -= c.values.len();
                }
            }
            self.cached += values.len();
            self.order.push_back(index);
            self.cache.insert(index, Chunk { width, rows: dh, values });
        }
        self.cache.get(&index).ok_or_else(|| ZonalError::Read("cache".into()))
    }
}

/// Reads and decodes one chunk of `rows` rows of `cols` pixels of `spp` samples: decompressed in
/// one pass, then the predictor (TIFF 6 §14; Adobe's floating-point predictor) and byte order
/// undone, then each sample converted to `f32`.
fn decode_raw(
    dec: &mut Decoder<BufReader<File>>,
    l: &Raw,
    index: u32,
    cols: usize,
    spp: usize,
    rows: usize,
) -> Result<Vec<f32>, ZonalError> {
    use std::io::{Read, Seek, SeekFrom};
    let i = index as usize;
    let row_samples = cols * spp;
    let (at, len) = (l.offsets.get(i).copied().unwrap_or(0), l.counts.get(i).copied().unwrap_or(0));
    let row_bytes = row_samples * l.bytes;
    let want = row_bytes * rows;
    // A compressed chunk far larger than its data could ever need is refused.
    if len > (want as u64).saturating_mul(2).saturating_add(4096) {
        return Err(ZonalError::Read(format!("strip or tile {index} claims {len} bytes")));
    }
    let mut raw = vec![0u8; len as usize];
    let r = dec.inner();
    r.seek(SeekFrom::Start(at)).map_err(read_err)?;
    r.read_exact(&mut raw).map_err(read_err)?;
    let bad = |e: &dyn std::fmt::Display| ZonalError::Read(format!("strip or tile {index}: {e}"));
    let mut out = match l.compression {
        1 => raw,
        5 => {
            // Into a buffer of exactly the chunk's size, in one call, so a hostile stream can't
            // expand past it.
            let mut out = vec![0u8; want];
            let mut lzw = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
            let (mut read, mut written) = (0, 0);
            while written < want {
                let r = lzw.decode_bytes(&raw[read..], &mut out[written..]);
                read += r.consumed_in;
                written += r.consumed_out;
                match r.status.map_err(|e| bad(&e))? {
                    weezl::LzwStatus::Ok if r.consumed_in + r.consumed_out > 0 => {}
                    _ => break,
                }
            }
            out.truncate(written);
            out
        }
        32773 => unpack_bits(&raw, want),
        _ => miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&raw, want).map_err(|e| bad(&format!("{e:?}")))?,
    };
    if out.len() < want {
        return Err(ZonalError::Read(format!("strip or tile {index} is shorter than its size")));
    }
    let b = l.bytes;
    let mut values = Vec::with_capacity(row_samples * rows);
    let mut bits = vec![0u64; row_samples];
    for row in out[..want].chunks_exact_mut(row_bytes) {
        if l.predictor == 3 {
            // Floating-point predictor: bytes differenced along the row (one pixel apart), and
            // each sample's bytes split into planes, most significant first.
            for k in spp..row.len() {
                row[k] = row[k].wrapping_add(row[k - spp]);
            }
            let n = row_samples;
            for (s, v) in bits.iter_mut().enumerate() {
                *v = (0..b).fold(0u64, |acc, m| (acc << 8) | u64::from(row[m * n + s]));
            }
        } else {
            for (s, v) in bits.iter_mut().enumerate() {
                let c = &row[s * b..(s + 1) * b];
                *v = if l.big_endian {
                    c.iter().fold(0u64, |acc, &x| (acc << 8) | u64::from(x))
                } else {
                    c.iter().rev().fold(0u64, |acc, &x| (acc << 8) | u64::from(x))
                };
            }
            if l.predictor == 2 {
                let mask = if b == 8 { u64::MAX } else { (1u64 << (8 * b)) - 1 };
                for s in spp..row_samples {
                    bits[s] = bits[s].wrapping_add(bits[s - spp]) & mask;
                }
            }
        }
        values.extend(bits.iter().map(|&v| sample_to_f32(v, b, l.format)));
    }
    Ok(values)
}

/// PackBits (TIFF 6 §9), up to `want` bytes.
fn unpack_bits(raw: &[u8], want: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(want);
    let mut i = 0;
    while i < raw.len() && out.len() < want {
        let n = raw[i] as i8;
        i += 1;
        if n >= 0 {
            let end = (i + n as usize + 1).min(raw.len());
            out.extend_from_slice(&raw[i..end]);
            i = end;
        } else if n != -128 {
            if let Some(&b) = raw.get(i) {
                out.extend(std::iter::repeat_n(b, (1 - i32::from(n)) as usize));
            }
            i += 1;
        }
    }
    out
}

fn sample_to_f32(v: u64, bytes: usize, format: u16) -> f32 {
    match (format, bytes) {
        (3, 4) => f32::from_bits(v as u32),
        (3, 8) => f64::from_bits(v) as f32,
        (2, _) => {
            let shift = 64 - 8 * bytes as u32;
            (((v << shift) as i64) >> shift) as f32
        }
        _ => v as f32,
    }
}

fn to_f32(r: DecodingResult) -> Vec<f32> {
    match r {
        DecodingResult::U8(v) => v.into_iter().map(f32::from).collect(),
        DecodingResult::I8(v) => v.into_iter().map(f32::from).collect(),
        DecodingResult::U16(v) => v.into_iter().map(f32::from).collect(),
        DecodingResult::I16(v) => v.into_iter().map(f32::from).collect(),
        DecodingResult::U32(v) => v.into_iter().map(|x| x as f32).collect(),
        DecodingResult::I32(v) => v.into_iter().map(|x| x as f32).collect(),
        DecodingResult::U64(v) => v.into_iter().map(|x| x as f32).collect(),
        DecodingResult::I64(v) => v.into_iter().map(|x| x as f32).collect(),
        DecodingResult::F16(v) => v.into_iter().map(f32::from).collect(),
        DecodingResult::F32(v) => v,
        DecodingResult::F64(v) => v.into_iter().map(|x| x as f32).collect(),
    }
}

impl RasterSource for TiffSource {
    fn grid(&self) -> &Grid {
        &self.grid
    }

    fn read_window(&mut self, col0: u32, row0: u32, cols: u32, rows: u32, out: &mut [f32]) -> Result<(), ZonalError> {
        let (w, h) = (self.grid.width, self.grid.height);
        if cols == 0 || rows == 0 {
            return Ok(());
        }
        let fits = col0.checked_add(cols).is_some_and(|e| e <= w) && row0.checked_add(rows).is_some_and(|e| e <= h);
        if !fits || out.len() < cols as usize * rows as usize {
            return Err(ZonalError::Read("window outside the raster".into()));
        }
        let (cw, ch) = (self.chunk_w, self.chunk_h);
        for ty in row0 / ch..=(row0 + rows - 1) / ch {
            for tx in col0 / cw..=(col0 + cols - 1) / cw {
                let index = ty * self.across + tx;
                let c = self.chunk(index)?;
                let (ra, rb) = ((ty * ch).max(row0), ((ty + 1) * ch).min(row0 + rows));
                let (ca, cb) = ((tx * cw).max(col0), ((tx + 1) * cw).min(col0 + cols));
                for r in ra..rb {
                    let lr = (r - ty * ch) as usize;
                    if lr >= c.rows {
                        return Err(ZonalError::Read(format!("strip or tile {index} is shorter than its size")));
                    }
                    let start = lr * c.width + (ca - tx * cw) as usize;
                    let src = c.values.get(start..start + (cb - ca) as usize).ok_or_else(|| ZonalError::Read("short tile".into()))?;
                    let o = (r - row0) as usize * cols as usize + (ca - col0) as usize;
                    out[o..o + src.len()].copy_from_slice(src);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
