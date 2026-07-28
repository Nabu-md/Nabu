use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use objc2::{extern_class, msg_send_id, rc::Id, runtime::AnyObject, ClassType, Message};
use objc2_foundation::{NSURL, NSString, NSDictionary, NSArray};

// Extension for NSDictionary
impl NSDictionary<NSString, AnyObject> {
    pub fn objectForKey(&self, key: &NSString) -> Option<Id<AnyObject>> {
        unsafe {
            let val: *mut AnyObject = msg_send_id![self, objectForKey: key];
            if val.is_null() { None } else { Some(Id::retain(val).unwrap()) }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Annotation {
    pub page: u32,
    pub content: String,
}

pub struct PdfAnnotator {
    annotations_dir: std::path::PathBuf,
}

impl PdfAnnotator {
    pub fn new(vault_root: &std::path::Path) -> Self {
        let dir = vault_root.join(".nabu/annotations");
        Self {
            annotations_dir: dir,
        }
    }

    pub fn annotate(&self, _pdf_path: &str, page: u32, content: &str) -> Result<()> {
        if page == 0 {
            anyhow::bail!("Page {} out of range", page);
        }
        let ann = Annotation {
            page,
            content: content.into(),
        };
        let ann_id = uuid::Uuid::new_v4();
        let ann_path = self
            .annotations_dir
            .join(format!("{}_{}.json", ann_id, page));

        let file = std::fs::File::create(&ann_path).context("Failed to create annotation file")?;
        serde_json::to_writer_pretty(file, &ann).context("Failed to serialize annotation")?;
        Ok(())
    }
}

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct PDFDocument;

    unsafe impl ClassType for PDFDocument {
        type Super = AnyObject;
        type Mutability = objc2::mutability::InteriorMutable;
        const NAME: &'static str = "PDFDocument";
    }
);

unsafe impl Message for PDFDocument {}

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct PDFPage;

    unsafe impl ClassType for PDFPage {
        type Super = AnyObject;
        type Mutability = objc2::mutability::InteriorMutable;
        const NAME: &'static str = "PDFPage";
    }
);

unsafe impl Message for PDFPage {}

impl PDFDocument {
    pub fn initWithURL(url: &NSURL) -> Option<Id<Self>> {
        unsafe {
            let cls = <Self as ClassType>::class();
            let obj: *mut Self = msg_send_id![cls, initWithURL: url];
            Id::retain(obj)
        }
    }

    pub fn init() -> Option<Id<Self>> {
        unsafe {
            let cls = <Self as ClassType>::class();
            let obj: *mut Self = msg_send_id![cls, init];
            Id::retain(obj)
        }
    }

    pub fn writeToURL(&self, url: &NSURL) -> bool {
        unsafe { msg_send_id![self, writeToURL: url] }
    }

    pub fn documentAttributes(&self) -> Option<Id<NSDictionary<NSString, AnyObject>>> {
        unsafe {
            let attrs: *mut NSDictionary<NSString, AnyObject> = msg_send_id![self, documentAttributes];
            if attrs.is_null() { None } else { Some(Id::retain(attrs).unwrap()) }
        }
    }

    pub fn pageAtIndex(&self, index: usize) -> Option<Id<PDFPage>> {
        unsafe {
            let page: *mut PDFPage = msg_send_id![self, pageAtIndex: index];
            Id::retain(page)
        }
    }

    pub fn pageCount(&self) -> usize {
        unsafe { msg_send_id![self, pageCount] }
    }

    pub fn insertPage_atIndex(&self, page: &PDFPage, index: usize) {
        unsafe { msg_send_id![self, insertPage: page, atIndex: index] }
    }

    pub fn removePageAtIndex(&self, index: usize) {
        unsafe { msg_send_id![self, removePageAtIndex: index] }
    }
}

impl PDFPage {
    pub fn setRotation(&self, rotation: i32) {
        unsafe { msg_send_id![self, setRotation: rotation] }
    }

    pub fn string(&self) -> Option<Id<NSString>> {
        unsafe {
            let s: *mut NSString = msg_send_id![self, string];
            if s.is_null() { None } else { Some(Id::retain(s).unwrap()) }
        }
    }
    
    pub fn annotations(&self) -> Option<Id<NSArray<AnyObject>>> {
        unsafe {
            let annos: *mut NSArray<AnyObject> = msg_send_id![self, annotations];
            if annos.is_null() { None } else { Some(Id::retain(annos).unwrap()) }
        }
    }
}

pub struct PdfEngine;

impl PdfEngine {
    pub fn merge(paths: &[std::path::PathBuf], output: &std::path::Path) -> Result<()> {
        let mut merged_doc = PDFDocument::init().context("Failed to create PDFDocument")?;
        let mut current_page_index = 0;

        for path in paths {
            let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
            let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;
            
            for i in 0..doc.pageCount() {
                let page = doc.pageAtIndex(i).context("Failed to get page")?;
                merged_doc.insertPage_atIndex(&page, current_page_index);
                current_page_index += 1;
            }
        }
        
        let output_url = NSURL::fileURLWithPath(&NSString::from_str(output.to_str().unwrap()));
        if !merged_doc.writeToURL(&output_url) {
            return Err(anyhow::anyhow!("Failed to save merged PDF"));
        }
        
        Ok(())
    }

    pub fn split(path: &std::path::Path, output_dir: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;
        
        for i in 0..doc.pageCount() {
            let new_doc = PDFDocument::init().context("Failed to create PDFDocument")?;
            let page = doc.pageAtIndex(i).context("Failed to get page")?;
            new_doc.insertPage_atIndex(&page, 0);
            
            let output_path = output_dir.join(format!("page_{}.pdf", i + 1));
            let output_url = NSURL::fileURLWithPath(&NSString::from_str(output_path.to_str().unwrap()));
            new_doc.writeToURL(&output_url);
        }
        Ok(())
    }

    pub fn extract_pages(path: &std::path::Path, pages: &[u32], output: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;
        let new_doc = PDFDocument::init().context("Failed to create PDFDocument")?;
        
        for (i, &page_idx) in pages.iter().enumerate() {
            let page = doc.pageAtIndex(page_idx as usize - 1).context("Failed to get page")?;
            new_doc.insertPage_atIndex(&page, i);
        }
        
        let output_url = NSURL::fileURLWithPath(&NSString::from_str(output.to_str().unwrap()));
        new_doc.writeToURL(&output_url);
        Ok(())
    }

    pub fn rotate_pages(path: &std::path::Path, pages: &[u32], rotation: i32, output: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;
        
        for &page_idx in pages {
            let page = doc.pageAtIndex(page_idx as usize - 1).context("Failed to get page")?;
            page.setRotation(rotation);
        }
        
        let output_url = NSURL::fileURLWithPath(&NSString::from_str(output.to_str().unwrap()));
        doc.writeToURL(&output_url);
        Ok(())
    }

    pub fn compress(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url).context("Failed to load PDF")?;
        let output_url = NSURL::fileURLWithPath(&NSString::from_str(output.to_str().unwrap()));
        doc.writeToURL(&output_url);
        Ok(())
    }

    pub fn fill_form(path: &std::path::Path, data: std::collections::HashMap<String, String>, output: &std::path::Path) -> Result<()> {
        Ok(())
    }

    pub fn flatten_form(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
        Ok(())
    }

    pub fn init_document() -> Option<Id<PDFDocument>> {
        PDFDocument::init()
    }
}
