use std::{collections::HashSet, ops::Div};

use crate::printpdf::{
    ImageTransform, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerIndex, PdfPageIndex, Point, Pt,
    Rgb,
};
use cosmic_text::{
    fontdb, Attrs, AttrsList, Color, Family, FontSystem, LayoutLine, ShapeLine, Weight,
};
use image::DynamicImage;
use syntect::{
    highlighting::{HighlightState, Highlighter, RangedHighlightIterator, ThemeSet},
    parsing::{ParseState, ScopeStack, SyntaxSet},
};

const MM_PER_INCH: f32 = 25.4;
const DOTS_PER_INCH: f32 = 96.0;
const MM_PER_DOTS: f32 = MM_PER_INCH / DOTS_PER_INCH;
const DOTS_PER_MM: f32 = DOTS_PER_INCH / MM_PER_INCH;

struct Dots(f32);

impl From<Mm> for Dots {
    fn from(value: Mm) -> Self {
        Dots(value.0 * DOTS_PER_MM)
    }
}
impl From<Pt> for Dots {
    fn from(value: Pt) -> Self {
        Mm::from(value).into()
    }
}
impl From<Dots> for Mm {
    fn from(value: Dots) -> Self {
        Mm(value.0 * MM_PER_DOTS)
    }
}
impl From<Dots> for Pt {
    fn from(value: Dots) -> Self {
        Mm::from(value).into()
    }
}
struct Dpi(f32);
impl Div<Mm> for Dots {
    type Output = Dpi;

    fn div(self, rhs: Mm) -> Self::Output {
        Dpi((self.0) / rhs.0 * MM_PER_INCH)
    }
}

const PAGE_WIDTH: Mm = Mm(210.0);
const PAGE_HEIGHT: Mm = Mm(297.0);
const X_MARGIN: Mm = Mm(10.0);
const Y_MARGIN: Mm = Mm(25.0);
const BOTTOM_RULE: Mm = Mm(PAGE_HEIGHT.0 - Y_MARGIN.0);

pub struct Fonts {
    pub font_system: FontSystem,
    pub fonts: HashSet<fontdb::ID>,
}

impl Fonts {
    fn get_font_by_id(&mut self, id: fontdb::ID) -> IndirectFontRef {
        self.fonts.insert(id);
        IndirectFontRef {
            name: self
                .font_system
                .db()
                .face(id)
                .unwrap()
                .post_script_name
                .to_owned(),
        }
    }
}

pub struct Document {
    pub fonts: Fonts,
    pub pdf: PdfDocument,
    pub pages: Vec<Page>,
    pub paragraph: Paragraph,
    pub syntax: SyntaxSet,
    pub theme: ThemeSet,
    pub images: usize,
}

pub struct Page {
    pub page: PdfPageIndex,
    pub text: PdfLayerIndex,
    pub y_offset: Mm,
}

pub struct Paragraph {
    pub text: String,
    pub attrs: AttrsList,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            text: String::new(),
            attrs: AttrsList::new(Attrs::new().family(Family::Serif)),
        }
    }
}

impl Page {
    fn new(pdf: &mut PdfDocument) -> Self {
        let (page, text) = pdf.add_page(PAGE_WIDTH, PAGE_HEIGHT, "text");
        Page {
            page,
            text,
            y_offset: Y_MARGIN,
        }
    }
}

impl Paragraph {
    pub fn write_body(&mut self, text: &str, attrs: Attrs) {
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();

        self.attrs.add_span(start..end, attrs);
    }
    pub fn write_line_break(&mut self) {
        self.text.push('\n');
    }
}

struct ShapedLines {
    lines: Vec<LayoutLine>,
    attrs: AttrsList,
    font_size: Pt,
    x_margin: Mm,
}

impl Document {
    pub fn write_line_break(&mut self) {
        self.paragraph.write_line_break();
    }

    pub fn write_body(&mut self, text: &str, attrs: Attrs) {
        self.paragraph.write_body(text, attrs);
    }

    pub fn end_last_paragraph(&mut self) {
        let font_size = Pt(12.0);
        let line_height = Pt(14.0);

        let paragraph = std::mem::take(&mut self.paragraph);
        if !paragraph.text.is_empty() {
            let lines = self.shape_lines(&paragraph.text, paragraph.attrs, font_size, X_MARGIN);
            self.write_shaped_lines(lines, line_height, Mm(0.0), false);

            let page_layout = self.pages.last_mut().unwrap();
            page_layout.y_offset += Mm::from(line_height) * 0.5;
        }
    }

    pub fn add_y_offset(&mut self, offset: Mm) {
        match self.pages.last_mut() {
            Some(p) => p,
            None => {
                self.pages.push(Page::new(&mut self.pdf));
                self.pages.last_mut().unwrap()
            }
        }
        .y_offset += offset;
    }

    pub fn write_header(&mut self, paragraph: &str, heading: u8) {
        let font_size = SIZES[heading as usize - 1];
        let line_height = font_size * 1.4;
        let attrs = Attrs::new().family(Family::SansSerif).weight(Weight::BOLD);

        // if heading == 1 {
        //     self.add_y_offset(line_height.into())
        // }

        let attrs = AttrsList::new(attrs);
        let lines = self.shape_lines(paragraph, attrs, font_size, X_MARGIN);
        self.overflow(Mm::from(line_height) * lines.lines.len() as f32);

        self.write_shaped_lines(lines, line_height, Mm::from(line_height) * 0.5, false);
    }

    fn shape_lines(
        &mut self,
        text: &str,
        attrs: AttrsList,
        font_size: Pt,
        x_margin: Mm,
    ) -> ShapedLines {
        let shape = ShapeLine::new(&mut self.fonts.font_system, text, &attrs);
        let lines = shape.layout(
            Dots::from(font_size).0,
            Dots::from(PAGE_WIDTH - x_margin * 2.0).0,
            cosmic_text::Wrap::Word,
            Some(cosmic_text::Align::Left),
        );
        ShapedLines {
            lines,
            attrs,
            font_size,
            x_margin,
        }
    }

    fn write_shaped_lines(
        &mut self,
        layout: ShapedLines,
        line_height: Pt,
        y_offset: Mm,
        center: bool,
    ) {
        for line in layout.lines {
            self.overflow(Mm(0.0));

            // where does the line start
            let x_offset = if center {
                (PAGE_WIDTH - Mm::from(Dots(line.w))) * 0.5
            } else {
                layout.x_margin
            };

            self.write_line(
                &line,
                &layout.attrs,
                x_offset,
                layout.font_size,
                line_height,
                y_offset,
            );
        }
    }

    fn overflow(&mut self, size: Mm) {
        assert!(size + Y_MARGIN < BOTTOM_RULE, "block is toooooo big");

        match self.pages.last_mut() {
            Some(p) => {
                // if this will overflow our line limit, then make a new page
                if p.y_offset + size > BOTTOM_RULE {
                    self.pages.push(Page::new(&mut self.pdf));
                }
            }
            None => {
                self.pages.push(Page::new(&mut self.pdf));
            }
        };
    }

    fn write_line(
        &mut self,
        line: &LayoutLine,
        attrs: &AttrsList,
        x_offset: Mm,
        font_size: Pt,
        line_height: Pt,
        y_offset: Mm,
    ) {
        let page_layout = self.pages.last_mut().unwrap();
        let layer = self
            .pdf
            .get_page(page_layout.page)
            .get_layer(page_layout.text);

        // start the line
        layer.begin_text_section();
        layer.set_text_cursor(x_offset, PAGE_HEIGHT - page_layout.y_offset - y_offset);

        let runs = GroupSliceBy {
            slice: line.glyphs.as_slice(),
            group: |glyph| (attrs.get_span(glyph.start), glyph.cache_key.font_id),
        };
        for ((attr, font_id), run) in runs {
            let pdf_font = self.fonts.get_font_by_id(font_id);
            layer.set_font(&pdf_font, font_size.0 * attr.scaling);
            layer.set_fill_color(map_cosmic_color(attr.color_opt));
            layer.write_codepoints(run.iter().map(|x| x.cache_key.glyph_id))
        }
        layer.end_text_section();
        page_layout.y_offset += line_height.into();
    }

    /// write page titles and page numbers
    pub fn write_extras(&mut self) {
        let font_size = Pt(12.0);
        let line_height = Pt(14.0);

        let attr = Attrs::new().family(Family::Serif).weight(Weight::BOLD);

        let title_shape = ShapeLine::new(
            &mut self.fonts.font_system,
            "Async Rust: Deep Dive",
            &AttrsList::new(attr),
        );
        let title_layout = title_shape.layout(
            Dots::from(font_size).0,
            Dots::from(PAGE_WIDTH).0,
            cosmic_text::Wrap::Word,
            Some(cosmic_text::Align::Center),
        );
        let [title_layout] = title_layout.as_slice() else { panic!("header overflowed line") };

        let font_id = {
            let mut font_ids = title_layout.glyphs.iter().map(|a| a.cache_key.font_id);
            match font_ids.next() {
                Some(first) => font_ids.try_fold(first, |a, b| (a == b).then_some(a)),
                None => None,
            }
        }
        .expect("extras should need just a single font");

        let pdf_font = self.fonts.get_font_by_id(font_id);

        for (i, page_layout) in self.pages.iter_mut().enumerate() {
            let number = format!("{}", i + 1);
            let number_shape =
                ShapeLine::new(&mut self.fonts.font_system, &number, &AttrsList::new(attr));
            let number_layout = number_shape
                .layout(
                    Dots::from(font_size).0,
                    Dots::from(PAGE_WIDTH).0,
                    cosmic_text::Wrap::Word,
                    Some(cosmic_text::Align::Center),
                )
                .remove(0);

            let current_layer = self
                .pdf
                .get_page(page_layout.page)
                .get_layer(page_layout.text);

            current_layer.set_font(&pdf_font, font_size.0);
            current_layer.set_line_height(line_height.0);

            current_layer.begin_text_section();
            current_layer.set_fill_color(map_cosmic_color(attr.color_opt));
            let x = Mm::from(Dots(title_layout.glyphs.first().unwrap().x));
            current_layer.set_text_cursor(x, PAGE_HEIGHT - Mm(5.0) - Mm::from(line_height)); // 5mm from the top
            current_layer
                .write_codepoints(title_layout.glyphs.iter().map(|x| x.cache_key.glyph_id));
            current_layer.end_text_section();

            current_layer.begin_text_section();
            let x = Mm::from(Dots(number_layout.glyphs.first().unwrap().x));
            current_layer.set_fill_color(map_cosmic_color(attr.color_opt));
            current_layer.set_text_cursor(x, Mm(12.0) - Mm::from(line_height));
            current_layer
                .write_codepoints(number_layout.glyphs.iter().map(|x| x.cache_key.glyph_id));
            current_layer.end_text_section();
        }
    }

    /// prepare new page, if necessary
    pub fn new_page(&mut self) {
        // todo: check for pre-created pages. for now it's not possible
        let (page, text) = self.pdf.add_page(PAGE_WIDTH, PAGE_HEIGHT, "text");
        self.pages.push(Page {
            page,
            text,
            y_offset: Y_MARGIN,
        });
    }

    pub fn write_code(&mut self, lang: &str, text: &str, font_size: Pt, line_height: Pt) {
        let theme = self.theme.themes["base16-ocean.dark"].clone();
        let highlighter = Highlighter::new(&theme);
        let mut highlight_state = HighlightState::new(&highlighter, ScopeStack::new());
        let mut parse_state = self
            .syntax
            .find_syntax_by_extension(lang)
            .map(ParseState::new);

        let default_bg = crate::printpdf::Color::Rgb(Rgb::new(0.85, 0.85, 0.85, None));
        let default_fg = Color::rgb(38, 38, 38);

        let bg = theme.settings.background.map_or(default_bg, map_color);
        let fg = theme
            .settings
            .foreground
            .map_or(default_fg, |c| Color::rgba(c.r, c.g, c.b, c.a));

        let default_attrs = Attrs::new().family(Family::Monospace).color(fg);

        {
            self.overflow(Mm::from(line_height) * text.lines().count() as f32);
            let page_layout = self.pages.last_mut().unwrap();

            // page_layout.y_offset -= Mm::from(line_height) * 0.5;

            let height = Mm::from(line_height) * (1 + text.lines().count()) as f32;

            let current_layer = self
                .pdf
                .get_page(page_layout.page)
                .get_layer(page_layout.text);

            let top = page_layout.y_offset - Mm::from(line_height) * 0.5;
            let bottom = page_layout.y_offset + height;

            current_layer.set_fill_color(bg);
            current_layer.add_shape(Line {
                points: vec![
                    (Point::new(X_MARGIN * 1.5, PAGE_HEIGHT - bottom), false),
                    (Point::new(X_MARGIN * 1.5, PAGE_HEIGHT - top), false),
                    (
                        Point::new(PAGE_WIDTH - X_MARGIN * 1.5, PAGE_HEIGHT - top),
                        false,
                    ),
                    (
                        Point::new(PAGE_WIDTH - X_MARGIN * 1.5, PAGE_HEIGHT - bottom),
                        false,
                    ),
                ],
                is_closed: true,
                has_fill: true,
                has_stroke: false,
                is_clipping_path: false,
            });

            page_layout.y_offset += Mm::from(line_height);
        }

        for line in text.lines() {
            let mut attrs = AttrsList::new(default_attrs);

            if let Some(state) = parse_state.as_mut() {
                let ops = state.parse_line(line, &self.syntax).unwrap();
                for (style, _, range) in
                    RangedHighlightIterator::new(&mut highlight_state, &ops, line, &highlighter)
                {
                    let c = style.foreground;
                    attrs.add_span(range, default_attrs.color(Color::rgba(c.r, c.g, c.b, c.a)))
                }
            }

            let shape = ShapeLine::new(&mut self.fonts.font_system, line, &attrs);
            let line = shape.layout(
                Dots::from(font_size).0,
                Dots::from(PAGE_WIDTH).0,
                cosmic_text::Wrap::Word,
                Some(cosmic_text::Align::Center),
            );
            let [line] = line.as_slice() else { panic!("codeblock line overflowed") };

            self.write_line(
                line,
                &attrs,
                X_MARGIN * 2.0,
                font_size,
                line_height,
                Mm(0.0),
            );
        }

        self.add_y_offset(Mm::from(line_height) * 1.5);
    }

    pub fn add_image(&mut self, title: Paragraph, image: &DynamicImage) {
        self.images += 1;

        let width = image.width();
        let height = image.height();

        let max_width: Mm = PAGE_WIDTH * 0.75;
        let max_height: Mm = PAGE_HEIGHT * 0.75;

        let render_width;
        let render_height;
        if max_width * height as f32 / width as f32 > max_height {
            render_height = max_height;
            render_width = max_height * width as f32 / height as f32
        } else {
            render_height = max_width * height as f32 / width as f32;
            render_width = max_width;
        }

        let font_size = Pt(12.0);
        let line_height = Pt(14.0);

        let caption_lines =
            self.shape_lines(&title.text, title.attrs, font_size, PAGE_WIDTH * 0.125);

        self.overflow(render_height + Mm::from(line_height) * caption_lines.lines.len() as f32);

        let page_layout = self.pages.last_mut().unwrap();
        let current_page = self.pdf.get_page(page_layout.page);

        let image = crate::printpdf::Image::from_dynamic_image(image);
        image.add_to_layer(
            current_page,
            page_layout.text,
            ImageTransform {
                translate_x: Some((PAGE_WIDTH - render_width) * 0.5),
                translate_y: Some(PAGE_HEIGHT - page_layout.y_offset - render_height),
                rotate: None,
                scale_x: None,
                scale_y: None,
                dpi: Some((Dots(width as f32) / render_width).0),
            },
        );
        page_layout.y_offset += render_height + Mm::from(line_height);

        self.write_shaped_lines(caption_lines, line_height, Mm(0.0), true);
        self.add_y_offset(Mm::from(line_height));
    }
}

fn map_color(c: syntect::highlighting::Color) -> crate::printpdf::Color {
    crate::printpdf::Color::Rgb(Rgb::new(
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        None,
    ))
}

fn map_cosmic_color(c: Option<cosmic_text::Color>) -> crate::printpdf::Color {
    match c {
        Some(c) => crate::printpdf::Color::Rgb(Rgb::new(
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            None,
        )),
        None => crate::printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)),
    }
}

// generated using a log scale
const SIZES: [Pt; 6] = [
    Pt(48.0),
    Pt(36.377197),
    Pt(27.56876),
    Pt(20.893213),
    Pt(15.834095),
    Pt(12.0),
];

/// Itertools GroupBy but more efficient
struct GroupSliceBy<'a, T, F, G>
where
    F: FnMut(&T) -> G,
{
    slice: &'a [T],
    group: F,
}

impl<'a, T, F, G> Iterator for GroupSliceBy<'a, T, F, G>
where
    F: FnMut(&T) -> G,
    G: PartialEq,
{
    type Item = (G, &'a [T]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            return None;
        }

        let group = (self.group)(&self.slice[0]);
        for i in 1..self.slice.len() {
            let b = &self.slice[i];

            if (self.group)(b) != group {
                let (slice, rest) = self.slice.split_at(i);
                self.slice = rest;

                return Some((group, slice));
            }
        }

        Some((group, std::mem::take(&mut self.slice)))
    }
}
