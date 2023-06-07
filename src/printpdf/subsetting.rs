use allsorts::{
    binary::read::ReadScope,
    font::read_cmap_subtable,
    font_data::FontData,
    tables::{cmap::Cmap, FontTableProvider},
    tag,
};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

pub(crate) struct FontSubset {
    pub(crate) new_font_bytes: Vec<u8>,
    /// Mapping from old GIDs (in the original font) to the new GIDs (in the new subset font)
    pub(crate) gid_mapping: HashMap<u16, u16>,
}

pub(crate) fn subset(
    font_bytes: &[u8],
    used_glyphs: &mut HashSet<u16>,
) -> Result<FontSubset, Box<dyn Error>> {
    let font_file = ReadScope::new(font_bytes).read::<FontData<'_>>()?;
    let provider = font_file.table_provider(0)?;
    let cmap_data = provider.read_table_data(tag::CMAP)?;
    let cmap = ReadScope::new(&cmap_data).read::<Cmap<'_>>()?;
    let (_, cmap_subtable) =
        read_cmap_subtable(&cmap)?.ok_or(allsorts::error::ParseError::MissingValue)?;

    // Prevent `allsorts` from using MacRoman encoding by using a non supported character
    let gid_eur = cmap_subtable
        .map_glyph('€' as u32)?
        .ok_or(allsorts::error::ParseError::MissingValue)?;
    used_glyphs.insert(0);
    used_glyphs.insert(gid_eur);

    let mut glyph_ids: Vec<u16> = used_glyphs.iter().copied().collect();

    glyph_ids.sort_unstable();

    let new_font_bytes = allsorts::subset::subset(&provider, &glyph_ids)?;

    let mut gid_mapping = HashMap::new();
    for (idx, old_gid) in glyph_ids.into_iter().enumerate() {
        gid_mapping.insert(old_gid, idx as u16);
    }

    Ok(FontSubset {
        new_font_bytes,
        gid_mapping,
    })
}
