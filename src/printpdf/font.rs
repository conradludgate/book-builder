//! Embedding fonts in 2D for Pdf
use cosmic_text::fontdb::FaceInfo;
use cosmic_text::rustybuzz::ttf_parser::GlyphId;
use cosmic_text::Font;
use lopdf;
use lopdf::StringFormat;
use lopdf::{Dictionary as LoDictionary, Stream as LoStream};
use std::collections::{BTreeMap, HashMap};
use std::iter::FromIterator;

pub struct ExternalFont<'a> {
    pub font: &'a Font,
    pub face_info: FaceInfo,
}

/// The text rendering mode determines how a text is drawn
/// The default rendering mode is `Fill`. The color of the
/// fill / stroke is determine by the current pages outline /
/// fill color.
///
/// See PDF Reference 1.7 Page 402
#[derive(Debug, Copy, Clone)]
pub enum TextRenderingMode {
    Fill,
    Stroke,
    FillStroke,
    Invisible,
    FillClip,
    StrokeClip,
    FillStrokeClip,
    Clip,
}

impl From<TextRenderingMode> for i64 {
    fn from(val: TextRenderingMode) -> Self {
        use crate::printpdf::TextRenderingMode::*;
        match val {
            Fill => 0,
            Stroke => 1,
            FillStroke => 2,
            Invisible => 3,
            FillClip => 4,
            StrokeClip => 5,
            FillStrokeClip => 6,
            Clip => 7,
        }
    }
}

impl ExternalFont<'_> {
    /// Takes the font and adds it to the document and consumes the font.
    ///
    /// Returns None if the font doesn't need to be embedded
    pub(crate) fn into_with_document(self, doc: &mut lopdf::Document) -> Option<LoDictionary> {
        use lopdf::Object;
        use lopdf::Object::*;

        let font = self.font;
        let face_info = self.face_info;

        let font_stream = LoStream::new(
            LoDictionary::from_iter(vec![("Length1", Integer(font.data().len() as i64))]),
            font.data().to_owned(),
        )
        .with_compression(false); /* important! font stream must not be compressed! */

        // Begin setting required font attributes
        let mut font_vec: Vec<(::std::string::String, Object)> = vec![
            ("Type".into(), Name("Font".into())),
            ("Subtype".into(), Name("Type0".into())),
            (
                "BaseFont".into(),
                Name(face_info.post_script_name.clone().into_bytes()),
            ),
            // Identity-H for horizontal writing, Identity-V for vertical writing
            ("Encoding".into(), Name("Identity-H".into())),
            // Missing DescendantFonts and ToUnicode
        ];

        let mut font_descriptor_vec: Vec<(::std::string::String, Object)> = vec![
            ("Type".into(), Name("FontDescriptor".into())),
            (
                "FontName".into(),
                Name(face_info.post_script_name.clone().into_bytes()),
            ),
            (
                "Ascent".into(),
                Integer(i64::from(font.rustybuzz().ascender())),
            ),
            (
                "Descent".into(),
                Integer(i64::from(font.rustybuzz().descender())),
            ),
            (
                "CapHeight".into(),
                Integer(i64::from(font.rustybuzz().ascender())),
            ),
            ("ItalicAngle".into(), Integer(0)),
            ("Flags".into(), Integer(32)),
            ("StemV".into(), Integer(80)),
        ];

        // End setting required font arguments

        // Maximum height of a single character in the font
        let mut max_height = 0;
        // Total width of all characters
        let mut total_width = 0;
        // Widths (or heights, depends on self.vertical_writing)
        // of the individual characters, indexed by glyph id
        let mut widths = Vec::<(u32, u32)>::new();

        // Glyph IDs - (Unicode IDs - character width, character height)
        let mut cmap = BTreeMap::<u32, (u32, u32, u32)>::new();
        cmap.insert(0, (0, 1000, 1000));

        for (glyph_id, c) in glyph_ids(font) {
            if let Some(glyph_metrics) = glyph_metrics(font, glyph_id) {
                if glyph_metrics.height > max_height {
                    max_height = glyph_metrics.height;
                }

                total_width += glyph_metrics.width;
                cmap.insert(
                    glyph_id as u32,
                    (c as u32, glyph_metrics.width, glyph_metrics.height),
                );
            }
        }

        // Maps the character index to a unicode value - add this to the "ToUnicode" dictionary!
        //
        // To explain this structure: Glyph IDs have to be in segments where the first byte of the
        // first and last element have to be the same. A range from 0x1000 - 0x10FF is valid
        // but a range from 0x1000 - 0x12FF is not (0x10 != 0x12)
        // Plus, the maximum number of Glyph-IDs in one range is 100
        //
        // Since the glyph IDs are sequential, all we really have to do is to enumerate the vector
        // and create buckets of 100 / rest to 256 if needed

        let mut cur_first_bit: u16 = 0_u16; // current first bit of the glyph id (0x10 or 0x12) for example

        let mut all_cmap_blocks = Vec::new();

        {
            let mut current_cmap_block = Vec::new();

            for (glyph_id, unicode_width_tuple) in &cmap {
                if (*glyph_id >> 8) as u16 != cur_first_bit || current_cmap_block.len() >= 100 {
                    // end the current (beginbfchar endbfchar) block
                    all_cmap_blocks.push(current_cmap_block.clone());
                    current_cmap_block = Vec::new();
                    cur_first_bit = (*glyph_id >> 8) as u16;
                }

                let (unicode, width, _) = *unicode_width_tuple;
                current_cmap_block.push((*glyph_id, unicode));
                widths.push((*glyph_id, width));
            }

            all_cmap_blocks.push(current_cmap_block);
        }

        let cid_to_unicode_map =
            generate_cid_to_unicode_map(face_info.post_script_name.clone(), all_cmap_blocks);

        let cid_to_unicode_map_stream =
            LoStream::new(LoDictionary::new(), cid_to_unicode_map.as_bytes().to_vec());
        let cid_to_unicode_map_stream_id = doc.add_object(cid_to_unicode_map_stream);

        // encode widths / heights so that they fit into what PDF expects
        // see page 439 in the PDF 1.7 reference
        // basically widths_list will contain objects like this:
        // 20 [21, 99, 34, 25]
        // which means that the character with the GID 20 has a width of 21 units
        // and the character with the GID 21 has a width of 99 units
        let mut widths_list = Vec::<Object>::new();
        let mut current_low_gid = 0;
        let mut current_high_gid = 0;
        let mut current_width_vec = Vec::<Object>::new();

        // scale the font width so that it sort-of fits into an 1000 unit square
        let percentage_font_scaling = 1000.0 / (font.rustybuzz().units_per_em() as f64);

        for gid in 0..font.rustybuzz().number_of_glyphs() {
            if let Some(GlyphMetrics { width, .. }) = glyph_metrics(font, gid) {
                if gid == current_high_gid {
                    current_width_vec
                        .push(Integer((width as f64 * percentage_font_scaling) as i64));
                    current_high_gid += 1;
                } else {
                    widths_list.push(Integer(current_low_gid as i64));
                    widths_list.push(Array(current_width_vec.drain(..).collect()));

                    current_width_vec
                        .push(Integer((width as f64 * percentage_font_scaling) as i64));
                    current_low_gid = gid;
                    current_high_gid = gid + 1;
                }
            } else {
                continue;
            }
        }
        // push the last widths, because the loop is delayed by one iteration
        widths_list.push(Integer(current_low_gid as i64));
        widths_list.push(Array(current_width_vec.drain(..).collect()));

        let w = { ("W", Array(widths_list)) };

        // default width for characters
        let dw = { ("DW", Integer(1000)) };

        let mut desc_fonts = LoDictionary::from_iter(vec![
            ("Type", Name("Font".into())),
            ("Subtype", Name("CIDFontType2".into())),
            ("BaseFont", Name(face_info.post_script_name.into())),
            (
                "CIDSystemInfo",
                Dictionary(LoDictionary::from_iter(vec![
                    ("Registry", String("Adobe".into(), StringFormat::Literal)),
                    ("Ordering", String("Identity".into(), StringFormat::Literal)),
                    ("Supplement", Integer(0)),
                ])),
            ),
            w,
            dw,
        ]);

        let font_bbox = vec![
            Integer(0),
            Integer(max_height as i64),
            Integer(total_width as i64),
            Integer(max_height as i64),
        ];
        font_descriptor_vec.push(("FontFile2".into(), Reference(doc.add_object(font_stream))));

        // although the following entry is technically not needed, Adobe Reader needs it
        font_descriptor_vec.push(("FontBBox".into(), Array(font_bbox)));

        let font_descriptor_vec_id = doc.add_object(LoDictionary::from_iter(font_descriptor_vec));

        desc_fonts.set("FontDescriptor", Reference(font_descriptor_vec_id));

        font_vec.push((
            "DescendantFonts".into(),
            Array(vec![Dictionary(desc_fonts)]),
        ));
        font_vec.push(("ToUnicode".into(), Reference(cid_to_unicode_map_stream_id)));

        Some(LoDictionary::from_iter(font_vec))
    }
}

// type GlyphId = u32;
type UnicodeCodePoint = u32;
type CmapBlock = Vec<(u32, UnicodeCodePoint)>;

/// Generates a CMAP (character map) from valid cmap blocks
fn generate_cid_to_unicode_map(face_name: String, all_cmap_blocks: Vec<CmapBlock>) -> String {
    let mut cid_to_unicode_map = format!(
        include_str!("../../assets/gid_to_unicode_beg.txt"),
        face_name
    );

    for cmap_block in all_cmap_blocks
        .into_iter()
        .filter(|block| !block.is_empty() || block.len() < 100)
    {
        cid_to_unicode_map.push_str(format!("{} beginbfchar\r\n", cmap_block.len()).as_str());
        for (glyph_id, unicode) in cmap_block {
            cid_to_unicode_map.push_str(format!("<{glyph_id:04x}> <{unicode:04x}>\n").as_str());
        }
        cid_to_unicode_map.push_str("endbfchar\r\n");
    }

    cid_to_unicode_map.push_str(include_str!("../../assets/gid_to_unicode_end.txt"));
    cid_to_unicode_map
}

// impl PartialEq for ExternalFont {
//     /// Two fonts are equal if their names are equal, the contents aren't checked
//     fn eq(&self, other: &ExternalFont) -> bool {
//         self.face_name == other.face_name
//     }
// }

/// Indexed reference to a font that was added to the document
/// This is a "reference by postscript name"
#[derive(Debug, Hash, Eq, Ord, Clone, PartialEq, PartialOrd)]
pub struct IndirectFontRef {
    /// Name of the font (postscript name)
    pub(crate) name: String,
}

impl IndirectFontRef {
    /// Creates a new IndirectFontRef from an index
    pub fn new<S>(name: S) -> Self
    where
        S: Into<String>,
    {
        Self { name: name.into() }
    }
}

/// The metrics for a glyph provided by a [`FontData`](trait.FontData.html) implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphMetrics {
    /// The width of the glyph, typically the horizontal advance.
    pub width: u32,
    /// The height of the glyph, typically the difference between the ascent and the descent.
    pub height: u32,
}

fn glyph_ids(ttf: &Font) -> HashMap<u16, char> {
    let face = ttf.rustybuzz();
    let subtables = face
        .tables()
        .cmap
        .unwrap()
        .subtables
        .into_iter()
        .filter(|s| s.is_unicode());
    let mut map = HashMap::with_capacity(face.number_of_glyphs().into());
    for subtable in subtables {
        subtable.codepoints(|c| {
            use std::convert::TryFrom as _;

            if let Ok(ch) = char::try_from(c) {
                if let Some(idx) = subtable.glyph_index(c).filter(|idx| idx.0 > 0) {
                    map.entry(idx.0).or_insert(ch);
                }
            }
        })
    }
    map
}

fn glyph_metrics(ttf: &Font, glyph_id: u16) -> Option<GlyphMetrics> {
    let glyph_id = GlyphId(glyph_id);

    let face = ttf.rustybuzz();

    if let Some(width) = face.glyph_hor_advance(glyph_id) {
        let width = width as u32;
        let height = face
            .glyph_bounding_box(glyph_id)
            .map(|bbox| bbox.y_max - bbox.y_min - face.descender())
            .unwrap_or(1000) as u32;
        Some(GlyphMetrics { width, height })
    } else {
        None
    }
}
