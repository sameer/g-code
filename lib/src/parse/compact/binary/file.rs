use nom::{
    bytes::streaming::tag,
    combinator::{iterator, opt},
    number::streaming::le_u16,
    Compare, IResult, Input, Parser,
};

use crate::parse::compact::binary::blocks::{
    parse_block, BlockHeader, BlockType, EncodingType, GCodeEncodingType,
    ThumbnailParameters,
};


/// Ordered representation of the contents of a binary g-code file.
#[derive(Debug)]
pub struct File<I> {
    pub header: FileHeader,
    pub file_meta: Option<(BlockHeader, EncodingType, I, Option<u32>)>,
    pub printer_meta: (BlockHeader, EncodingType, I, Option<u32>),
    pub thumbnails: Vec<(BlockHeader, ThumbnailParameters, I, Option<u32>)>,
    pub print_meta: (BlockHeader, EncodingType, I, Option<u32>),
    pub slicer_meta: (BlockHeader, EncodingType, I, Option<u32>),
    pub gcode_blocks: Vec<(BlockHeader, GCodeEncodingType, I, Option<u32>)>,
}

impl<I: Input<Item = u8> + for<'a> Compare<&'a [u8]>> File<I> {
    /// Parse a complete file into its expected format.
    pub fn parse(input: I) -> IResult<I, File<I>> {
        let stage = FileHeaderParser;
        let (input, (header, stage)) = stage.parser().parse(input)?;
        let (input, (file_meta, stage)) = stage.parser().parse(input)?;
        let (input, (printer_meta, stage)) = stage.parser().parse(input)?;
        let (input, (thumbnails, stage)) = stage.parser().parse(input)?;
        let (input, (print_meta, stage)) = stage.parser().parse(input)?;
        let (input, (slicer_meta, stage)) = stage.parser().parse(input)?;
        let (input, gcode_blocks) = stage.parser().parse(input)?;

        Ok((
            input,
            Self {
                header,
                file_meta,
                printer_meta,
                thumbnails,
                print_meta,
                slicer_meta,
                gcode_blocks,
            },
        ))
    }
}

pub struct FileHeaderParser;

impl FileHeaderParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<I, Output = (FileHeader, FileMetaParser), Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        FileHeader::parser().map(move |header| {
            let checksum_type = header.checksum_type;
            (header, FileMetaParser(checksum_type))
        })
    }
}

pub struct FileMetaParser(ChecksumType);

impl FileMetaParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<
        I,
        Output = (
            Option<(BlockHeader, EncodingType, I, Option<u32>)>,
            PrinterMetaParser,
        ),
        Error = nom::error::Error<I>,
    >
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let checksum_type = self.0;
        opt(move |input| {
            parse_block(
                BlockType::FileMetadata,
                checksum_type,
                EncodingType::parser(),
                input,
            )
        })
        .map(move |opt| (opt, PrinterMetaParser(checksum_type)))
    }
}

pub struct PrinterMetaParser(ChecksumType);

impl PrinterMetaParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<
        I,
        Output = (
            (BlockHeader, EncodingType, I, Option<u32>),
            ThumbnailsParser,
        ),
        Error = nom::error::Error<I>,
    >
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let checksum_type = self.0;
        (move |input| {
            parse_block(
                BlockType::PrinterMetadata,
                checksum_type,
                EncodingType::parser(),
                input,
            )
        })
        .map(move |block| (block, ThumbnailsParser(checksum_type)))
    }
}

pub struct ThumbnailsParser(ChecksumType);

impl ThumbnailsParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<
        I,
        Output = (
            Vec<(BlockHeader, ThumbnailParameters, I, Option<u32>)>,
            PrintMetaParser,
        ),
        Error = nom::error::Error<I>,
    >
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let checksum_type = self.0;
        move |input| {
            let mut it = iterator(input, move |input| {
                parse_block(
                    BlockType::Thumbnail,
                    checksum_type,
                    ThumbnailParameters::parser(),
                    input,
                )
            });

            let thumbnails = (&mut it).collect::<Vec<_>>();
            it.finish()
                .map(|(input, ())| (input, (thumbnails, PrintMetaParser(checksum_type))))
        }
    }
}

pub struct PrintMetaParser(ChecksumType);

impl PrintMetaParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<
        I,
        Output = (
            (BlockHeader, EncodingType, I, Option<u32>),
            SlicerMetaParser,
        ),
        Error = nom::error::Error<I>,
    >
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let checksum_type = self.0;
        (move |input| {
            parse_block(
                BlockType::PrintMetadata,
                checksum_type,
                EncodingType::parser(),
                input,
            )
        })
        .map(move |block| (block, SlicerMetaParser(checksum_type)))
    }
}

pub struct SlicerMetaParser(ChecksumType);

impl SlicerMetaParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<
        I,
        Output = (
            (BlockHeader, EncodingType, I, Option<u32>),
            GCodeBlocksParser,
        ),
        Error = nom::error::Error<I>,
    >
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let checksum_type = self.0;
        (move |input| {
            parse_block(
                BlockType::SlicerMetadata,
                checksum_type,
                EncodingType::parser(),
                input,
            )
        })
        .map(move |block| (block, GCodeBlocksParser(checksum_type)))
    }
}

pub struct GCodeBlocksParser(ChecksumType);

impl GCodeBlocksParser {
    pub fn parser<I>(
        &self,
    ) -> impl Parser<
        I,
        Output = Vec<(BlockHeader, GCodeEncodingType, I, Option<u32>)>,
        Error = nom::error::Error<I>,
    >
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        let checksum_type = self.0;
        move |input| {
            let mut it = iterator(input, move |input| {
                parse_block(
                    BlockType::Thumbnail,
                    checksum_type,
                    GCodeEncodingType::parser(),
                    input,
                )
            });

            let gcode_blocks = (&mut it).collect::<Vec<_>>();
            it.finish().map(|(input, ())| (input, gcode_blocks))
        }
    }
}

/// 10 bytes at the beginning of each file.
///
/// <https://github.com/prusa3d/libbgcode/blob/main/doc/specifications.md#file-header>
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    pub checksum_type: ChecksumType,
}

impl FileHeader {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        const MAGIC_NUMBER: &[u8; 4] = b"GCDE";
        const VERSION_BYTES: [u8; 4] = 1u32.to_le_bytes();

        tag(MAGIC_NUMBER.as_slice())
            .and(tag(VERSION_BYTES.as_slice()))
            .and(ChecksumType::parser())
            .map(|((_magic_number, _version), checksum)| Self {
                checksum_type: checksum,
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ChecksumType {
    None = 0,
    /// 32-bit checksum at the end of each block.
    ///
    /// <https://en.wikipedia.org/wiki/Computation_of_cyclic_redundancy_checks#CRC-32_example>
    CRC32 = 1,
}

impl ChecksumType {
    pub fn parser<I>() -> impl Parser<I, Output = Self, Error = nom::error::Error<I>>
    where
        I: Input<Item = u8> + for<'a> Compare<&'a [u8]>,
    {
        le_u16.map_res(|value| match value {
            0 => Ok(Self::None),
            1 => Ok(Self::CRC32),
            _other => Err(()),
        })
    }
}
