use anyhow::Context;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, doc, TantivyDocument};
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
    pub fn search(&self, query_str: &str) -> anyhow::Result<Vec<String>> {
        let searcher = self.reader.searcher();
        let query_parser = tantivy::query::QueryParser::for_index(&self.index, vec![
            self.schema.get_field("content").unwrap(),
            self.schema.get_field("tags").unwrap()
        ]);
        let query = query_parser.parse_query(query_str)?;
        let collector = tantivy::collector::TopDocs::with_limit(10).order_by_score();
        let top_docs = searcher.search(&query, &collector)?;
        
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let path = retrieved_doc.get_first(self.schema.get_field("path").unwrap())
                .context("Missing path")?
                .as_str()
                .context("Not text")?
                .to_string();
            results.push(path);
        }
        Ok(results)
    }

}
