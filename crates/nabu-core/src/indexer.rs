use crate::models::knowledge_object::KnowledgeObject;
use crate::search_query::{SearchQuery, Filter};
use anyhow::Context;
use std::path::PathBuf;
use std::collections::HashSet;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, doc, Term};

/// Indexer with incremental indexing support.
///
/// Only changed KnowledgeObjects are reindexed. Deleted objects are removed
/// from the index. The indexer tracks indexed document IDs to avoid
/// redundant indexing operations.
pub struct Indexer {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    schema: Schema,
    /// Set of document IDs currently in the index, used for incremental updates.
    indexed_ids: HashSet<String>,
}

impl Indexer {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", STORED | STRING);
        schema_builder.add_text_field("content", TEXT | STORED);
        schema_builder.add_text_field("tags", TEXT | STORED);
        schema_builder.add_json_field("custom", STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("object_type", STRING | STORED);
        schema_builder.add_text_field("modified_at", STRING | STORED);
        let schema = schema_builder.build();

        let index = Index::open_or_create(
            tantivy::directory::MmapDirectory::open(&path)?,
            schema.clone(),
        )?;
        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()?;
        let writer = index.writer(50_000_000)?;

        // Load existing indexed IDs for incremental tracking
        let indexed_ids = Self::load_indexed_ids(&reader, &schema);

        Ok(Self {
            index,
            reader,
            writer,
            schema,
            indexed_ids,
        })
    }

    /// Load all currently indexed document IDs from the index.
    /// This enables incremental indexing by tracking which documents are already indexed.
    fn load_indexed_ids(reader: &IndexReader, schema: &Schema) -> HashSet<String> {
        let searcher = reader.searcher();
        let path_field = schema.get_field("path").unwrap();
        let mut ids = HashSet::new();
        for doc_addr in searcher.doc_addresses() {
            if let Ok(doc) = searcher.doc(doc_addr) {
                if let Some(path_val) = doc.get_first(path_field) {
                    if let Some(path_str) = path_val.as_str() {
                        ids.insert(path_str.to_string());
                    }
                }
            }
        }
        ids
    }

    /// Index a single KnowledgeObject incrementally.
    ///
    /// If the document is already indexed (same ID), it is deleted and re-added.
    /// If the document is new, it is added directly.
    /// This avoids full index rebuilds for individual document changes.
    pub fn index_document(&mut self, ko: &KnowledgeObject) -> anyhow::Result<()> {
        let doc_id = ko.id.to_string();

        // If already indexed, delete the old version first (incremental update)
        if self.indexed_ids.contains(&doc_id) {
            self.delete_document(&doc_id)?;
        }

        let content = match &ko.content {
            crate::models::knowledge_object::ObjectContent::Markdown(m) => m.as_str(),
            crate::models::knowledge_object::ObjectContent::PlainText(t) => t.as_str(),
            crate::models::knowledge_object::ObjectContent::Html(h) => h.as_str(),
            crate::models::knowledge_object::ObjectContent::Binary => "",
            crate::models::knowledge_object::ObjectContent::Structured(json) => {
                serde_json::to_string(json).unwrap_or_default().as_str()
            }
        };

        let path_field = self.schema.get_field("path").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let tag_field = self.schema.get_field("tags").unwrap();
        let custom_field = self.schema.get_field("custom").unwrap();
        let title_field = self.schema.get_field("title").unwrap();
        let object_type_field = self.schema.get_field("object_type").unwrap();
        let modified_at_field = self.schema.get_field("modified_at").unwrap();

        let mut doc = TantivyDocument::default();
        doc.add_text(path_field, doc_id.clone());
        doc.add_text(content_field, content);
        doc.add_text(tag_field, crate::markdown::extract_tags(content).join(" "));
        doc.add_json_object(custom_field, serde_json::to_value(&ko.metadata.custom)?);
        doc.add_text(title_field, ko.metadata.title.clone().unwrap_or_default());
        doc.add_text(object_type_field, ko.object_type.to_string());
        doc.add_text(modified_at_field, ko.modified_at.clone());

        self.writer.add_document(doc)?;
        self.indexed_ids.insert(doc_id);

        // Commit after each document for durability; in production, batch commits
        // would be used for throughput.
        self.writer.commit()?;
        Ok(())
    }

    /// Delete a document from the index by its ID.
    /// Used for incremental updates when a document is removed or updated.
    pub fn delete_document(&mut self, doc_id: &str) -> anyhow::Result<()> {
        let path_field = self.schema.get_field("path").unwrap();
        let term = Term::from_field_text(path_field, doc_id);
        self.writer.delete_term(term);
        self.writer.commit()?;
        self.indexed_ids.remove(doc_id);
        Ok(())
    }

    /// Batch index multiple KnowledgeObjects incrementally.
    /// More efficient than calling `index_document` in a loop because
    /// commits are batched.
    pub fn index_documents_batch(&mut self, objects: &[KnowledgeObject]) -> anyhow::Result<()> {
        let mut to_delete: Vec<String> = Vec::new();
        let mut to_add: Vec<TantivyDocument> = Vec::new();

        for ko in objects {
            let doc_id = ko.id.to_string();
            if self.indexed_ids.contains(&doc_id) {
                to_delete.push(doc_id);
            }
            to_add.push(self.build_document(ko)?);
        }

        // Delete existing documents first
        for doc_id in &to_delete {
            let path_field = self.schema.get_field("path").unwrap();
            let term = Term::from_field_text(path_field, doc_id);
            self.writer.delete_term(term);
            self.indexed_ids.remove(doc_id);
        }

        // Add new/updated documents
        for doc in to_add {
            self.writer.add_document(doc)?;
        }

        self.writer.commit()?;
        Ok(())
    }

    /// Build a TantivyDocument from a KnowledgeObject without adding it to the index.
    fn build_document(&self, ko: &KnowledgeObject) -> anyhow::Result<TantivyDocument> {
        let doc_id = ko.id.to_string();
        let content = match &ko.content {
            crate::models::knowledge_object::ObjectContent::Markdown(m) => m.as_str(),
            crate::models::knowledge_object::ObjectContent::PlainText(t) => t.as_str(),
            crate::models::knowledge_object::ObjectContent::Html(h) => h.as_str(),
            crate::models::knowledge_object::ObjectContent::Binary => "",
            crate::models::knowledge_object::ObjectContent::Structured(json) => {
                serde_json::to_string(json).unwrap_or_default().as_str()
            }
        };

        let path_field = self.schema.get_field("path").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let tag_field = self.schema.get_field("tags").unwrap();
        let custom_field = self.schema.get_field("custom").unwrap();
        let title_field = self.schema.get_field("title").unwrap();
        let object_type_field = self.schema.get_field("object_type").unwrap();
        let modified_at_field = self.schema.get_field("modified_at").unwrap();

        let mut doc = TantivyDocument::default();
        doc.add_text(path_field, doc_id);
        doc.add_text(content_field, content);
        doc.add_text(tag_field, crate::markdown::extract_tags(content).join(" "));
        doc.add_json_object(custom_field, serde_json::to_value(&ko.metadata.custom)?);
        doc.add_text(title_field, ko.metadata.title.clone().unwrap_or_default());
        doc.add_text(object_type_field, ko.object_type.to_string());
        doc.add_text(modified_at_field, ko.modified_at.clone());

        Ok(doc)
    }

    /// Force a reload of the index reader to see newly committed documents.
    pub fn reload(&self) -> anyhow::Result<()> {
        self.reader.reload()?;
        Ok(())
    }

    pub fn search(&self, search_query: &SearchQuery) -> anyhow::Result<Vec<String>> {
        let searcher = self.reader.searcher();
        let query_parser = tantivy::query::QueryParser::for_index(
            &self.index,
            vec![
                self.schema.get_field("content").unwrap(),
                self.schema.get_field("tags").unwrap(),
                self.schema.get_field("title").unwrap(),
            ],
        );
        let query = query_parser.parse_query(&search_query.query)?;

        let collector = tantivy::collector::TopDocs::with_limit(10).order_by_score();
        let top_docs = searcher.search(&query, &collector)?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let path = retrieved_doc
                .get_first(self.schema.get_field("path").unwrap())
                .context("Missing path")?
                .as_str()
                .context("Not text")?
                .to_string();
            results.push(path);
        }
        Ok(results)
    }
}
