//! A `PDFDocument` represents the whole content of the file

use crate::printpdf::utils::random_character_string_32;
use std::collections::HashMap;

use crate::printpdf::OffsetDateTime;
use lopdf;

use crate::printpdf::indices::*;
use crate::printpdf::{
    Error, ExternalFont, IccProfileList, Mm, PdfConformance, PdfMetadata, PdfPage,
};

/// PDF document
#[derive(Debug, Clone)]
pub struct PdfDocument {
    /// Pages of the document
    pub(super) pages: Vec<PdfPage>,
    // /// Fonts used in this document
    // pub fonts: FontList,
    /// ICC profiles used in the document
    pub(super) _icc_profiles: IccProfileList,
    /// Inner PDF document
    pub(super) inner_doc: lopdf::Document,
    /// Document ID. Must be changed if the document is loaded / parsed from a file
    pub document_id: String,
    /// Metadata for this document
    pub metadata: PdfMetadata,
    /// The bookmarks in the document. A HashMap<Page Number, Bookmark Name>
    pub bookmarks: HashMap<usize, String>,
}

// /// Marker struct for a document. Used to make the API a bit nicer.
// /// It simply calls `PdfDocument` functions.
// pub struct PdfDocumentReference {
//     /// A wrapper for a document, so actions from outside this library
//     /// are restricted to functions inside this crate (only functions in `lopdf`
//     /// can directly manipulate the document)
//     pub(crate) document: Rc<RefCell<PdfDocument>>,
// }

impl PdfDocument {
    /// Creates a new PDF document
    #[inline]
    pub fn new<S1, S2>(
        document_title: S1,
        initial_page_width: Mm,
        initial_page_height: Mm,
        initial_layer_name: S2,
    ) -> (Self, PdfPageIndex, PdfLayerIndex)
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let mut doc = Self {
            pages: Vec::new(),
            document_id: random_character_string_32(),
            // fonts: FontList::new(),
            _icc_profiles: IccProfileList::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            metadata: PdfMetadata::new(document_title, 1, false, PdfConformance::default()),
            bookmarks: HashMap::new(),
        };

        let (initial_page, layer_index) = PdfPage::new(
            initial_page_width,
            initial_page_height,
            initial_layer_name,
            0,
        );

        doc.pages.push(initial_page);

        (doc, PdfPageIndex(0), layer_index)
    }

    pub fn empty<S: Into<String>>(document_title: S) -> Self {
        Self {
            pages: Vec::new(),
            document_id: random_character_string_32(),
            // fonts: FontList::new(),
            _icc_profiles: IccProfileList::new(),
            inner_doc: lopdf::Document::with_version("1.3"),
            metadata: PdfMetadata::new(document_title, 1, false, PdfConformance::X3_2002_PDF_1_3),
            bookmarks: HashMap::new(),
        }
    }
}

impl PdfDocument {
    // ----- BUILDER FUNCTIONS

    /// Changes the title on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_title<S>(mut self, new_title: S) -> Self
    where
        S: Into<String>,
    {
        self.metadata.document_title = new_title.into();
        self
    }

    /// Changes the author metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_author<S>(mut self, author: S) -> Self
    where
        S: Into<String>,
    {
        self.metadata.author = author.into();
        self
    }

    /// Changes the creator metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_creator<S>(mut self, creator: S) -> Self
    where
        S: Into<String>,
    {
        self.metadata.creator = creator.into();
        self
    }

    /// Changes the producer/publisher metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_producer<S>(mut self, producer: S) -> Self
    where
        S: Into<String>,
    {
        self.metadata.producer = producer.into();
        self
    }

    /// Changes the keywords metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_keywords<S>(mut self, keywords: Vec<S>) -> Self
    where
        S: Into<String>,
    {
        self.metadata.keywords = keywords.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Changes the subject metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_subject<S>(mut self, subject: S) -> Self
    where
        S: Into<String>,
    {
        self.metadata.subject = subject.into();
        self
    }

    /// Changes the subject metadata property on both the document info dictionary as well as the metadata
    #[inline]
    pub fn with_identifier<S>(mut self, identifier: S) -> Self
    where
        S: Into<String>,
    {
        self.metadata.identifier = identifier.into();
        self
    }

    /// Set the trapping of the document
    #[inline]
    pub fn with_trapping(mut self, trapping: bool) -> Self {
        self.metadata.trapping = trapping;
        self
    }

    /// Sets the document ID (for comparing two PDF documents for equality)
    #[inline]
    pub fn with_document_id(mut self, id: String) -> Self {
        self.metadata.xmp_metadata.document_id = id;
        self
    }

    /// Set the version of the document
    #[inline]
    pub fn with_document_version(mut self, version: u32) -> Self {
        self.metadata.document_version = version;
        self
    }

    /// Changes the conformance of this document. It is recommended to call
    /// `check_for_errors()` after changing it.
    #[inline]
    pub fn with_conformance(mut self, conformance: PdfConformance) -> Self {
        self.metadata.conformance = conformance;
        self
    }

    /// Sets the creation date on the document.
    ///
    /// Per default, the creation date is set to the current time.
    #[inline]
    pub fn with_creation_date(mut self, creation_date: OffsetDateTime) -> Self {
        self.metadata.creation_date = creation_date;
        self
    }

    /// Sets the metadata date on the document.
    ///
    /// By default, the metadata date is set to the current time.
    #[inline]
    pub fn with_metadata_date(mut self, metadata_date: OffsetDateTime) -> Self {
        self.metadata.metadata_date = metadata_date;
        self
    }

    /// Sets the modification date on the document. Intended to be used when
    /// reading documents that already have a modification date.
    #[inline]
    pub fn with_mod_date(mut self, mod_date: OffsetDateTime) -> Self {
        self.metadata.modification_date = mod_date;
        self
    }

    // ----- ADD FUNCTIONS

    /// Create a new pdf page and returns the index of the page
    #[inline]
    pub fn add_page<S>(
        &mut self,
        x_mm: Mm,
        y_mm: Mm,
        inital_layer_name: S,
    ) -> (PdfPageIndex, PdfLayerIndex)
    where
        S: Into<String>,
    {
        let (pdf_page, pdf_layer_index) =
            PdfPage::new(x_mm, y_mm, inital_layer_name, self.pages.len());
        self.pages.push(pdf_page);
        let page_index = PdfPageIndex(self.pages.len() - 1);
        (page_index, pdf_layer_index)
    }
    /// Create a new pdf page and returns the index of the page.
    /// If the page already has a bookmark, overwrites it.
    #[inline]
    pub fn add_bookmark<S>(&mut self, name: S, page: PdfPageIndex)
    where
        S: Into<String>,
    {
        self.bookmarks.insert(page.0, name.into());
    }

    /// Returns the page (for inserting content)
    #[inline]
    pub fn get_page(&mut self, page: PdfPageIndex) -> &mut PdfPage {
        &mut self.pages[page.0]
    }

    // /// Returns a direct reference (object ID) to the font from an
    // /// indirect reference (postscript name)
    // #[inline]
    // pub fn get_font(&self, font: &IndirectFontRef) -> Option<DirectFontRef> {
    //     self.fonts.get_font(font)
    // }

    /// Drops the PDFDocument, returning the inner `lopdf::Document`.
    /// # Safety
    /// Document may be only half-written, use only in extreme cases
    #[inline]
    pub unsafe fn get_inner(self) -> lopdf::Document {
        // let doc = Rc::try_unwrap(self.document).unwrap().into_inner();
        self.inner_doc
    }

    /// Save PDF document to bytes
    pub fn save_to_bytes(
        self,
        fonts: impl IntoIterator<Item = cosmic_text::fontdb::ID>,
        db: &mut cosmic_text::FontSystem,
    ) -> Result<Vec<u8>, Error> {
        use lopdf::Object::*;
        use lopdf::StringFormat::Literal;
        use lopdf::{Dictionary as LoDictionary, Object as LoObject};

        // todo: remove unwrap, handle error
        let mut doc = self;
        let pages_id = doc.inner_doc.new_object_id();
        let bookmarks_id = doc.inner_doc.new_object_id();
        let mut bookmarks_list = LoDictionary::from_iter(vec![
            ("Type", "Outlines".into()),
            ("Count", Integer(doc.bookmarks.len() as i64)),
            /* First and Last will be filled in once they are created from the pages */
        ]);

        // extra pdf infos
        let (xmp_metadata, document_info, icc_profile) = doc.metadata.clone().into_obj();

        let xmp_metadata_id = match xmp_metadata {
            Some(metadata) => Some(doc.inner_doc.add_object(metadata)),
            None => None,
        };

        let document_info_id = doc.inner_doc.add_object(document_info);

        // add catalog
        let icc_profile_descr = "Commercial and special offset print acccording to ISO \
                                 12647-2:2004 / Amd 1, paper type 1 or 2 (matte or gloss-coated \
                                 offset paper, 115 g/m2), screen ruling 60/cm";
        let icc_profile_str = "Coated FOGRA39 (ISO 12647-2:2004)";
        let icc_profile_short = "FOGRA39";

        let mut output_intents = LoDictionary::from_iter(vec![
            ("S", Name("GTS_PDFX".into())),
            ("OutputCondition", String(icc_profile_descr.into(), Literal)),
            ("Type", Name("OutputIntent".into())),
            (
                "OutputConditionIdentifier",
                String(icc_profile_short.into(), Literal),
            ),
            (
                "RegistryName",
                String("http://www.color.org".into(), Literal),
            ),
            ("Info", String(icc_profile_str.into(), Literal)),
        ]);

        let mut catalog = LoDictionary::from_iter(vec![
            ("Type", "Catalog".into()),
            ("PageLayout", "OneColumn".into()),
            (
                "PageMode",
                if !doc.bookmarks.is_empty() {
                    "UseOutlines"
                } else {
                    "UseNone"
                }
                .into(),
            ),
            ("Outlines", Reference(bookmarks_id)),
            ("Pages", Reference(pages_id)),
        ]);

        if let Some(profile) = icc_profile {
            let icc_profile: lopdf::Stream = profile.into();
            let icc_profile_id = doc.inner_doc.add_object(Stream(icc_profile));
            output_intents.set("DestinationOutputProfile", Reference(icc_profile_id));
            catalog.set("OutputIntents", Array(vec![Dictionary(output_intents)]));
        }

        if let Some(metadata_id) = xmp_metadata_id {
            catalog.set("Metadata", Reference(metadata_id));
        }

        let mut pages = LoDictionary::from_iter(vec![
            ("Type", "Pages".into()),
            ("Count", Integer(doc.pages.len() as i64)),
            /* Kids and Resources missing */
        ]);

        // add all pages with contents
        let mut page_ids = Vec::<LoObject>::new();

        // ----- OCG CONTENT

        // page index + page names to add the OCG to the /Catalog
        let page_layer_names: Vec<(usize, Vec<::std::string::String>)> = doc
            .pages
            .iter()
            .map(|page| {
                (
                    page.index,
                    page.layers.iter().map(|layer| layer.name.clone()).collect(),
                )
            })
            .collect();

        // add optional content groups (layers) to the /Catalog
        let usage_ocg_dict = LoDictionary::from_iter(vec![
            ("Type", Name("OCG".into())),
            (
                "CreatorInfo",
                Dictionary(LoDictionary::from_iter(vec![
                    ("Creator", String("Adobe Illustrator 14.0".into(), Literal)),
                    ("Subtype", Name("Artwork".into())),
                ])),
            ),
        ]);

        let usage_ocg_dict_ref = doc.inner_doc.add_object(Dictionary(usage_ocg_dict));

        let intent_arr = Array(vec![Name("View".into()), Name("Design".into())]);

        let intent_arr_ref = doc.inner_doc.add_object(intent_arr);

        // page index, layer index, reference to OCG dictionary
        let ocg_list: Vec<(usize, Vec<(usize, lopdf::Object)>)> = page_layer_names
            .into_iter()
            .map(|(page_idx, layer_names)| {
                (
                    page_idx,
                    layer_names
                        .into_iter()
                        .enumerate()
                        .map(|(layer_idx, layer_name)| {
                            (
                                layer_idx,
                                Reference(doc.inner_doc.add_object(Dictionary(
                                    LoDictionary::from_iter(vec![
                                        ("Type", Name("OCG".into())),
                                        ("Name", String(layer_name.into(), Literal)),
                                        ("Intent", Reference(intent_arr_ref)),
                                        ("Usage", Reference(usage_ocg_dict_ref)),
                                    ]),
                                ))),
                            )
                        })
                        .collect(),
                )
            })
            .collect();

        let flattened_ocg_list: Vec<lopdf::Object> = ocg_list
            .iter()
            .flat_map(|(_, layers)| layers.iter().map(|(_, obj)| obj.clone()))
            .collect();

        catalog.set(
            "OCProperties",
            Dictionary(LoDictionary::from_iter(vec![
                ("OCGs", Array(flattened_ocg_list.clone())),
                // optional content configuration dictionary, page 376
                (
                    "D",
                    Dictionary(LoDictionary::from_iter(vec![
                        ("Order", Array(flattened_ocg_list.clone())),
                        // "radio button groups"
                        ("RBGroups", Array(vec![])),
                        // initially visible OCG
                        ("ON", Array(flattened_ocg_list)),
                    ])),
                ),
            ])),
        );

        // ----- END OCG CONTENT (on document level)

        // ----- PAGE CONTENT

        // add fonts (shared resources)
        let mut font_dict_id = None;

        let mut font_dict = lopdf::Dictionary::new();

        for id in fonts {
            let font = &*db.get_font(id).unwrap();
            let face_info = db.db().face(id).unwrap().clone();
            let name = face_info.post_script_name.clone();
            let font = ExternalFont { font, face_info };

            if let Some(font_dict_collected) = font.into_with_document(&mut doc.inner_doc) {
                let inner_obj = doc.inner_doc.new_object_id();
                doc.inner_doc
                    .objects
                    .insert(inner_obj, Dictionary(font_dict_collected));
                font_dict.set(name, Reference(inner_obj));
            }
        }

        if !font_dict.is_empty() {
            font_dict_id = Some(doc.inner_doc.add_object(Dictionary(font_dict)));
        }

        let mut page_id_to_obj: HashMap<usize, (u32, u16)> = HashMap::new();

        for (idx, page) in doc.pages.into_iter().enumerate() {
            let mut p = LoDictionary::from_iter(vec![
                ("Type", "Page".into()),
                ("Rotate", Integer(0)),
                (
                    "MediaBox",
                    vec![0.into(), 0.into(), page.width.into(), page.height.into()].into(),
                ),
                (
                    "TrimBox",
                    vec![0.into(), 0.into(), page.width.into(), page.height.into()].into(),
                ),
                (
                    "CropBox",
                    vec![0.into(), 0.into(), page.width.into(), page.height.into()].into(),
                ),
                ("Parent", Reference(pages_id)),
            ]);

            // this will collect the resources needed for rendering this page
            let layers_temp = ocg_list.iter().find(|e| e.0 == idx).unwrap();
            let (mut resources_page, layer_streams) =
                page.collect_resources_and_streams(&mut doc.inner_doc, &layers_temp.1);

            if let Some(f) = font_dict_id {
                resources_page.set("Font", Reference(f));
            }

            if !resources_page.is_empty() {
                let resources_page_id = doc.inner_doc.add_object(Dictionary(resources_page));
                p.set("Resources", Reference(resources_page_id));
            }

            // merge all streams of the individual layers into one big stream
            let mut layer_streams_merged_vec = Vec::<u8>::new();
            for mut stream in layer_streams {
                layer_streams_merged_vec.append(&mut stream.content);
            }

            let merged_layer_stream =
                lopdf::Stream::new(lopdf::Dictionary::new(), layer_streams_merged_vec);
            let page_content_id = doc.inner_doc.add_object(merged_layer_stream);

            p.set("Contents", Reference(page_content_id));
            let page_obj = doc.inner_doc.add_object(p);
            if doc.bookmarks.contains_key(&idx) {
                page_id_to_obj.insert(idx, page_obj);
            }
            page_ids.push(Reference(page_obj))
        }

        if !doc.bookmarks.is_empty() {
            let len = doc.bookmarks.len();
            if len == 1 {
                let page_index = doc.bookmarks.iter().next().unwrap().0.to_owned();
                let title = doc.bookmarks.iter().next().unwrap().1.to_owned();
                let obj_ref = doc
                    .inner_doc
                    .add_object(Dictionary(LoDictionary::from_iter(vec![
                        ("Parent", Reference(bookmarks_id)),
                        ("Title", String(title.into(), Literal)),
                        (
                            "Dest",
                            Array(vec![
                                Reference(page_id_to_obj.get(&page_index).unwrap().to_owned()),
                                "XYZ".into(),
                                Null,
                                Null,
                                Null,
                            ]),
                        ),
                    ])));
                bookmarks_list.set("First", Reference(obj_ref));
                bookmarks_list.set("Last", Reference(obj_ref));
            } else {
                let mut sorted_bmarks: Vec<(&usize, &std::string::String)> =
                    doc.bookmarks.iter().collect();
                sorted_bmarks.sort();
                for (i, (page_index, b_name)) in sorted_bmarks.iter().enumerate() {
                    let dest = (
                        "Dest",
                        Array(vec![
                            Reference(page_id_to_obj.get(page_index).unwrap().to_owned()),
                            "XYZ".into(),
                            Null,
                            Null,
                            Null,
                        ]),
                    );
                    doc.inner_doc
                        .add_object(Dictionary(LoDictionary::from_iter(if i == 0 {
                            bookmarks_list.set("First", Reference((doc.inner_doc.max_id + 1, 0)));
                            vec![
                                ("Parent", Reference(bookmarks_id)),
                                (
                                    "Title",
                                    String(b_name.to_owned().to_owned().into(), Literal),
                                ),
                                ("Next", Reference((doc.inner_doc.max_id + 2, 0))),
                                dest,
                            ]
                        } else if i == len - 1 {
                            bookmarks_list.set("Last", Reference((doc.inner_doc.max_id + 1, 0)));
                            vec![
                                ("Parent", Reference(bookmarks_id)),
                                (
                                    "Title",
                                    String(b_name.to_owned().to_owned().into(), Literal),
                                ),
                                ("Prev", Reference((doc.inner_doc.max_id, 0))),
                                dest,
                            ]
                        } else {
                            vec![
                                ("Parent", Reference(bookmarks_id)),
                                (
                                    "Title",
                                    String(b_name.to_owned().to_owned().into(), Literal),
                                ),
                                ("Prev", Reference((doc.inner_doc.max_id, 0))),
                                ("Next", Reference((doc.inner_doc.max_id + 2, 0))),
                                dest,
                            ]
                        })));
                }
            }
        }

        pages.set::<_, LoObject>("Kids".to_string(), page_ids.into());

        // ----- END PAGE CONTENT

        doc.inner_doc.objects.insert(pages_id, Dictionary(pages));
        doc.inner_doc
            .objects
            .insert(bookmarks_id, Dictionary(bookmarks_list));

        // save inner document
        let catalog_id = doc.inner_doc.add_object(catalog);
        let instance_id = random_character_string_32();

        doc.inner_doc.trailer.set("Root", Reference(catalog_id));
        doc.inner_doc
            .trailer
            .set("Info", Reference(document_info_id));
        doc.inner_doc.trailer.set(
            "ID",
            Array(vec![
                String(doc.document_id.as_bytes().to_vec(), Literal),
                String(instance_id.as_bytes().to_vec(), Literal),
            ]),
        );

        Self::optimize(&mut doc.inner_doc);

        let mut bytes = Vec::new();
        doc.inner_doc.save_to(&mut bytes)?;

        Ok(bytes)
    }

    #[inline]
    fn optimize(doc: &mut lopdf::Document) {
        doc.prune_objects();
        doc.delete_zero_length_streams();
        doc.compress();
    }
}
