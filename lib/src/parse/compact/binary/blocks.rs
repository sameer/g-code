use nom::{
    bytes::streaming::{tag, take},
    combinator::cond,
    number::streaming::{le_u16, le_u32},
    Compare, IResult, Input, Parser,
};

use crate::parse::compact::binary::file::ChecksumType;

/// 8 or 12 bytes at the start of each block.
///
/// <https://github.com/prusa3d/libbgcode/blob/main/doc/specifications.md#block-header>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    pub compression: Compression,
    pub uncompressed_size: u32,
    pub compressed_size: Option<u32>,
}

impl BlockHeader {
    pub fn parse<I>(ty: BlockType, input: I) -> IResult<I, BlockHeader>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let bytes = (ty as u16).to_le_bytes();

        let (input, ((_ty, compression), uncompressed_size)) = tag(bytes.as_slice())
            .and(Compression::parser())
            .and(le_u32)
            .parse(input)?;

        cond(compression != Compression::None, le_u32)
            .map(move |compressed_size| Self {
                compression,
                uncompressed_size,
                compressed_size,
            })
            .parse(input)
    }
}

pub fn parse_block<I, P, T>(
    ty: BlockType,
    checksum_type: ChecksumType,
    parameters_parser: P,
    input: I,
) -> IResult<I, (BlockHeader, T, I, Option<u32>)>
where
    I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    P: Parser<I, Output = T, Error = nom::error::Error<I>>,
{
    let (input, block_header) = BlockHeader::parse(ty, input)?;
    let size = block_header
        .compressed_size
        .unwrap_or(block_header.uncompressed_size);
    parameters_parser
        .and(take(size))
        .and(cond(checksum_type == ChecksumType::CRC32, le_u32))
        .map(move |((block_parameters, block_bytes), block_checksum)| {
            (block_header, block_parameters, block_bytes, block_checksum)
        })
        .parse(input)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlockType {
    FileMetadata = 0,
    GCode = 1,
    SlicerMetadata = 2,
    PrinterMetadata = 3,
    PrintMetadata = 4,
    Thumbnail = 5,
}

impl BlockType {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        le_u16.map_res(|value| match value {
            0 => Ok(Self::FileMetadata),
            1 => Ok(Self::GCode),
            2 => Ok(Self::SlicerMetadata),
            3 => Ok(Self::PrinterMetadata),
            4 => Ok(Self::PrintMetadata),
            5 => Ok(Self::Thumbnail),
            _other => Err(()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Compression {
    None = 0,
    Deflate = 1,
    /// See [icewrap].
    Heatshrink11_4 = 2,
    /// See [icewrap].
    Heatshrink12_4 = 3,
}

impl Compression {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        le_u16.map_res(|value| match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Deflate),
            2 => Ok(Self::Heatshrink11_4),
            3 => Ok(Self::Heatshrink12_4),
            _other => Err(()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EncodingType {
    Ini = 0,
}

impl EncodingType {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        le_u16.map_res(|value| match value {
            0 => Ok(Self::Ini),
            _other => Err(()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ThumbnailParameters {
    pub format: ThumbnailFormat,
    pub width: u16,
    pub height: u16,
}

impl ThumbnailParameters {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        (ThumbnailFormat::parser(), le_u16, le_u16).map(|(format, width, height)| Self {
            format,
            width,
            height,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ThumbnailFormat {
    Png = 0,
    Jpg = 1,
    /// <https://qoiformat.org>
    Qoi = 2,
}

impl ThumbnailFormat {
    fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        le_u16.map_res(|value| match value {
            0 => Ok(Self::Png),
            1 => Ok(Self::Jpg),
            2 => Ok(Self::Qoi),
            _other => Err(()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum GCodeEncodingType {
    None = 0,
    /// See [super::super::meatpack].
    Meatpack = 1,
    /// See [super::super::meatpack].
    MeatpackWithComments = 2,
}

impl GCodeEncodingType {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        le_u16.map_res(|value| match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Meatpack),
            2 => Ok(Self::MeatpackWithComments),
            _other => Err(()),
        })
    }
}
