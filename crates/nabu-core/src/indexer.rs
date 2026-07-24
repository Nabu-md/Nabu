use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, doc};
use tantivy::collector::TopDocs;
use std::path::PathBuf;

pub struct Indexer {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    schema: Schema,
}

impl Indexer {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", STORED | STRING);
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.add_text_field("tags", TEXT | STORED);
        let schema = schema_builder.build();

        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(&path)?, schema.clone())?;
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()?;
        let writer = index.writer(50_000_000)?;

        Ok(Self { index, reader, writer, schema })
    }

    pub fn index_document(&mut self, path: &str, content: &str) -> anyhow::Result<()> {
        let path_field = self.schema.get_field("path").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let tag_field = self.schema.get_field("tags").unwrap();
        
        let document = doc!(
            path_field => path,
            content_field => content,
            tag_field => crate::parser::extract_tags(content).join(" "),
        );
        
        self.writer.add_document(document)?;
        
        self.writer.commit()?;
        Ok(())
    }
    pub fn search(&self, _query_str: &str) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }

}
