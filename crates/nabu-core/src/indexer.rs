use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
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
        let schema = schema_builder.build();

        let index = Index::create_or_open(path, schema.clone())?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;
        let writer = index.writer(50_000_000)?;

        Ok(Self { index, reader, writer, schema })
    }

    pub fn index_document(&mut self, path: &str, content: &str) -> anyhow::Result<()> {
        let path_field = self.schema.get_field("path").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        
        let mut doc = tantivy::Document::default();
        doc.add_text(path_field, path);
        doc.add_text(content_field, content);
        
        self.writer.add_document(doc)?;
        self.writer.commit()?;
        Ok(())
    }
}
