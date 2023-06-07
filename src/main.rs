use std::{collections::HashSet, ffi::OsStr, io::Read, path::Path};

mod pdf;
#[allow(dead_code)]
mod printpdf;
// mod toc;

use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, ComrakOptions,
};
use cosmic_text::{fontdb::Database, Attrs, Family, FontSystem, Style, Weight};
use indexmap::IndexMap;
use pdf::{Document, Fonts, Paragraph};
use printpdf::{PdfDocument, Pt};
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};
// use toc::TocNode;

fn main() {
    pretty_env_logger::init();

    let doc = PdfDocument::empty("Async Rust: Deep Dive");

    let mut font_db = Database::new();
    font_db.load_fonts_dir("assets/fonts");

    font_db.set_monospace_family("Fira Code");
    font_db.set_sans_serif_family("PT Sans");
    font_db.set_serif_family("PT Serif");
    font_db.load_system_fonts();

    let font_system = FontSystem::new_with_locale_and_db("en-US".to_owned(), font_db);

    let mut doc = Document {
        fonts: Fonts {
            font_system,
            fonts: HashSet::new(),
        },
        pdf: doc,
        pages: vec![],
        paragraph: Default::default(),
        syntax: SyntaxSet::load_defaults_nonewlines(),
        theme: ThemeSet::load_defaults(),
        images: 0,
    };

    let mut custom = ThemeSet::load_from_folder("assets/themes").unwrap();
    doc.theme.themes.append(&mut custom.themes);

    // let toc_arena = Arena::new();
    let ast_arena = Arena::new();

    let chapters = parse_documents(&ast_arena);
    // let _toc = dbg!(TocNode::build(&toc_arena, &chapters));

    for (_, &node) in chapters.iter() {
        doc.end_last_paragraph();
        doc.new_page();
        let mut node = node;
        loop {
            doc.render_ast_node(
                node,
                State {
                    weight: Weight::NORMAL,
                    style: Style::Normal,
                    heading: 0,
                },
            );
            let Some(n) = node.next_sibling() else { break };
            node = n;
        }
    }
    doc.end_last_paragraph();

    doc.write_extras();

    let Document { mut fonts, pdf, .. } = doc;

    let data = pdf
        .save_to_bytes(fonts.fonts, &mut fonts.font_system)
        .unwrap();
    std::fs::write("test_pages.pdf", data).unwrap();
    // dbg!(chapters);
}

fn parse_documents<'a>(arena: &'a Arena<AstNode<'a>>) -> IndexMap<String, &'a AstNode<'a>> {
    let mut chapters = IndexMap::<String, &AstNode>::new();
    let options = ComrakOptions::default();

    let mut buffer = String::new();
    let mut chapter = String::new();
    for entry in walkdir::WalkDir::new("chapters").sort_by_file_name() {
        let entry = entry.unwrap();
        if entry.file_type().is_dir() {
            chapter.clear();
            chapter.push_str(entry.path().to_str().unwrap());
        } else if entry.path().extension() == Some(OsStr::new("md")) {
            let mut file = std::fs::File::open(entry.path()).unwrap();
            buffer.clear();
            file.read_to_string(&mut buffer).unwrap();
            let node = parse_document(arena, &buffer, &options);

            chapters
                .entry(chapter.clone())
                .and_modify(|n| n.insert_after(node))
                .or_insert(node);
        }
    }

    chapters
}

#[derive(Clone, Copy)]
struct State {
    weight: Weight,
    style: Style,
    heading: u8,
}

// const NBSP: char = '\u{A0}';
static NBSP_STR: &str = "\u{A0}";

impl Document {
    fn render_ast_node<'a>(&mut self, node: &'a AstNode<'a>, mut state: State) {
        match &node.data.borrow().value {
            NodeValue::Document => {
                for child in node.children() {
                    self.render_ast_node(child, state)
                }
            }
            NodeValue::FrontMatter(_) => todo!("FrontMatter(_)"),
            NodeValue::BlockQuote => todo!("BlockQuote"),
            NodeValue::List(_) => todo!("List(_)"),
            NodeValue::Item(_) => todo!("Item(_)"),
            NodeValue::DescriptionList => todo!("DescriptionList"),
            NodeValue::DescriptionItem(_) => todo!("DescriptionItem(_)"),
            NodeValue::DescriptionTerm => todo!("DescriptionTerm"),
            NodeValue::DescriptionDetails => todo!("DescriptionDetails"),
            NodeValue::CodeBlock(code) => {
                self.end_last_paragraph();
                self.write_code(&code.info, &code.literal, Pt(10.0), Pt(12.0));
            }
            NodeValue::HtmlBlock(_) => todo!("HtmlBlock(_)"),
            NodeValue::Paragraph => {
                self.end_last_paragraph();
                state = State {
                    weight: Weight::NORMAL,
                    style: Style::Normal,
                    heading: 0,
                };
                for child in node.children() {
                    self.render_ast_node(child, state)
                }
            }
            NodeValue::Heading(heading) => {
                self.end_last_paragraph();
                state.heading = heading.level;
                for child in node.children() {
                    self.render_ast_node(child, state)
                }
            }
            NodeValue::ThematicBreak => todo!("ThematicBreak"),
            NodeValue::FootnoteDefinition(_) => todo!("FootnoteDefinition(_)"),
            NodeValue::Table(_) => todo!("Table(_)"),
            NodeValue::TableRow(_) => todo!("TableRow(_)"),
            NodeValue::TableCell => todo!("TableCell"),
            NodeValue::Text(text) => {
                if state.heading == 0 {
                    self.write_body(
                        text,
                        Attrs::new()
                            .family(Family::Serif)
                            .style(state.style)
                            .weight(state.weight),
                    );
                } else {
                    self.write_header(text, state.heading);
                }
            }
            NodeValue::TaskItem { .. } => todo!("TaskItem"),
            NodeValue::SoftBreak | NodeValue::LineBreak => {
                self.write_line_break();
            }
            NodeValue::Code(code) => {
                self.write_body(
                    &code.literal.replace(' ', NBSP_STR),
                    Attrs::new()
                        .family(Family::Monospace)
                        .style(state.style)
                        .weight(state.weight)
                        .scaling(0.9),
                );
            }
            NodeValue::HtmlInline(_) => unimplemented!("inline html not supported"),
            NodeValue::Emph => {
                state.style = Style::Italic;
                for child in node.children() {
                    self.render_ast_node(child, state)
                }
            }
            NodeValue::Strong => {
                state.weight = Weight::BOLD;
                for child in node.children() {
                    self.render_ast_node(child, state)
                }
            }
            NodeValue::Strikethrough => todo!("Strikethrough"),
            NodeValue::Superscript => todo!("Superscript"),
            NodeValue::Link(_) => todo!("Link(_)"),
            NodeValue::Image(image) => {
                let img = image::io::Reader::open(Path::new("assets/images").join(&image.url))
                    .unwrap()
                    .decode()
                    .unwrap();

                let mut p = Paragraph::default();
                for child in node.children() {
                    p.render_ast_text(
                        child,
                        State {
                            weight: Weight::NORMAL,
                            style: Style::Normal,
                            heading: 0,
                        },
                    );
                }

                self.end_last_paragraph();
                self.add_image(p, &img);
            }
            NodeValue::FootnoteReference(_) => todo!("FootnoteReference(_)"),
        }

        // for child in node.children() {
        //     self.render_ast_node(child, state)
        // }
    }
}

impl Paragraph {
    fn render_ast_text<'a>(&mut self, node: &'a AstNode<'a>, mut state: State) {
        match &node.data.borrow().value {
            NodeValue::Document => {}
            NodeValue::FrontMatter(_) => todo!("FrontMatter(_)"),
            NodeValue::BlockQuote => todo!("BlockQuote"),
            NodeValue::List(_) => todo!("List(_)"),
            NodeValue::Item(_) => todo!("Item(_)"),
            NodeValue::DescriptionList => todo!("DescriptionList"),
            NodeValue::DescriptionItem(_) => todo!("DescriptionItem(_)"),
            NodeValue::DescriptionTerm => todo!("DescriptionTerm"),
            NodeValue::DescriptionDetails => todo!("DescriptionDetails"),
            NodeValue::CodeBlock(_) => todo!("CodeBlock(_)"),
            NodeValue::HtmlBlock(_) => todo!("HtmlBlock(_)"),
            NodeValue::Paragraph => todo!("Paragraph"),
            NodeValue::Heading(_) => todo!("Heading(_)"),
            NodeValue::ThematicBreak => todo!("ThematicBreak"),
            NodeValue::FootnoteDefinition(_) => todo!("FootnoteDefinition(_)"),
            NodeValue::Table(_) => todo!("Table(_)"),
            NodeValue::TableRow(_) => todo!("TableRow(_)"),
            NodeValue::TableCell => todo!("TableCell"),
            NodeValue::Text(text) => {
                self.write_body(
                    text,
                    Attrs::new()
                        .family(Family::Serif)
                        .style(state.style)
                        .weight(state.weight),
                );
            }
            NodeValue::TaskItem { .. } => todo!("TaskItem"),
            NodeValue::SoftBreak | NodeValue::LineBreak => {
                self.write_line_break();
            }
            NodeValue::Code(_) => todo!("Code(_)"),
            NodeValue::HtmlInline(_) => unimplemented!("inline html not supported"),
            NodeValue::Emph => {
                state.style = Style::Italic;
                for child in node.children() {
                    self.render_ast_text(child, state)
                }
            }
            NodeValue::Strong => {
                state.weight = Weight::BOLD;
                for child in node.children() {
                    self.render_ast_text(child, state)
                }
            }
            NodeValue::Strikethrough => todo!("Strikethrough"),
            NodeValue::Superscript => todo!("Superscript"),
            NodeValue::Link(_) => todo!("Link(_)"),
            NodeValue::Image(_) => todo!("Image(_)"),
            NodeValue::FootnoteReference(_) => todo!("FootnoteReference(_)"),
        }

        for child in node.children() {
            self.render_ast_text(child, state)
        }
    }
}
