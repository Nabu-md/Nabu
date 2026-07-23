use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tantivy::{
    doc,
    schema::{Field, Schema, FAST, STORED, TEXT},
    Index, IndexReader, IndexWriter,
};

const VAULT_ID_FIELD: &str = "vault_id";
const PATH_FIELD: &str = "path";
const TITLE_FIELD: &str = "title";
const CONTENT_FIELD: &str = "content";
const TAGS_FIELD: &str = "tags";
const MTIME_FIELD: &str = "mtime";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchSchemaDef {
    pub vault_id: Field,
    pub path: Field,
    pub title: Field,
    pub content: Field,
    pub tags: Field,
    pub mtime: Field,
}

#[derive(Debug, Clone, Default)]
pub struct NativeSearchSchema {
    pub vault_id: Field,
    pub path: Field,
    pub title: Field,
    pub content: Field,
    pub tags: Field,
    pub mtime: Field,
    schema: Schema,
}

impl NativeSearchSchema {
    fn build(schema: Schema) -> Self {
        let vault_id = schema.get_field(VAULT_ID_FIELD).unwrap();
        let path = schema.get_field(PATH_FIELD).unwrap();
        let title = schema.get_field(TITLE_FIELD).unwrap();
        let content = schema.get_field(CONTENT_FIELD).unwrap();
        let tags = schema.get_field(TAGS_FIELD).unwrap();
        let mtime = schema.get_field(MTIME_FIELD).unwrap();
        Self {
            vault_id,
            path,
            title,
            content,
            tags,
            mtime,
            schema,
        }
    }

    pub fn from(schema: tantivy::schema::Schema) -> Self {
        Self::build(schema)
    }

    pub fn define() -> SearchSchemaDef {
        let mut builder = Schema::builder();
        builder.add_text_field(VAULT_ID_FIELD, TEXT | STORED);
        builder.add_text_field(PATH_FIELD, TEXT | STORED);
        builder.add_text_field(TITLE_FIELD, TEXT | STORED);
        builder.add_text_field(CONTENT_FIELD, TEXT);
        builder.add_text_field(TAGS_FIELD, TEXT | STORED);
        builder.add_f64_field(MTIME_FIELD, FAST | STORED);

        let schema = builder.build();
        SearchSchemaDef::from(schema)
    }
}

impl From<tantivy::schema::Schema> for SearchSchemaDef {
    fn from(schema: tantivy::schema::Schema) -> Self {
        SearchSchemaDef::from_schema(schema)
    }
}

impl SearchSchemaDef {
    fn from_schema(schema: tantivy::schema::Schema) -> Self {
        let native = NativeSearchSchema::from(schema);
        SearchSchemaDef {
            vault_id: native.vault_id,
            path: native.path,
            title: native.title,
            content: native.content,
            tags: native.tags,
            mtime: native.mtime,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDocument {
    pub vault_id: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub mtime: f64,
}

#[derive(Debug, Default)]
pub struct SearchIndex {
    schema: NativeSearchSchema,
    writer: Option<IndexWriter>,
    reader: IndexReader,
    index: Index,
}

impl SearchIndex {
    pub fn open(directory: &Path) -> Result<Self> {
        let index = Index::open_in_dir(directory).context("failed to open tantivy index")?;
        let schema = index.schema();
        let schema = NativeSearchSchema::from(schema);
        let reader = index
            .reader_builder()
            .try_into()
            .context("failed to build index reader")?;
        Ok(Self {
            schema,
            writer: None,
            reader: reader,
            index,
        })
    }

    pub fn create(directory: &Path, schema: &SearchSchemaDef) -> Result<Self> {
        std::fs::create_dir_all(directory).context("failed to create search index directory")?;
        let mut builder = tantivy::schema::Schema::builder();
        builder.add_text_field(VAULT_ID_FIELD, tantivy::schema::TEXT | tantivy::schema::STORED);
        builder.add_text_field(PATH_FIELD, tantivy::schema::TEXT | tantivy::schema::STORED);
        builder.add_text_field(TITLE_FIELD, tantivy::schema::TEXT | tantivy::schema::STORED);
        builder.add_text_field(CONTENT_FIELD, tantivy::schema::TEXT);
        builder.add_text_field(TAGS_FIELD, tantivy::schema::TEXT | tantivy::schema::STORED);
        builder.add_f64_field(MTIME_FIELD, tantivy::schema::FAST | tantivy::schema::STORED);

        let schema = builder.build();
        let index = Index::create_in_dir(directory, schema).context("failed to create tantivy index")?;
        let schema = NativeSearchSchema::from(index.schema());
        let reader = index
            .reader_builder()
            .try_into()
            .context("failed to build index reader")?;

        Ok(Self {
            schema,
            writer: None,
            reader,
            index,
        })
    }

    pub fn writer(&mut self) -> Result<&mut IndexWriter> {
        if self.writer.is_none() {
            let writer = self
                .index
                .writer(50_000_000)
                .context("failed to create index writer")?;
            self.writer = Some(writer);
        }
        Ok(self.writer.as_mut().unwrap())
    }

    pub fn add_documents(&mut self, documents: impl IntoIterator<Item = NoteDocument>) -> Result<()> {
        let writer = self.writer()?;
        for note in documents {
            writer.add_document(doc!(
                self.schema.vault_id => note.vault_id,
                self.schema.path => note.path,
                self.schema.title => note.title,
                self.schema.content => note.content,
                self.schema.tags => note.tags.join(" "),
                self.schema.mtime => note.mtime,
            ))?;
        }
        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        if let Some(writer) = &mut self.writer {
            writer.commit().context("failed to commit search index")?;
        }
        Ok(())
    }

    pub fn query(&self, query: &str) -> Result<Vec<NoteDocument>> {
        let reader = self.reader.searcher();
        let query_parser = tantivy::query::QueryParser::for_index(
            &self.index,
            vec![self.schema.content, self.schema.title, self.schema.path],
        );
        let query = query_parser.parse_query(query).context("failed to parse query")?;
        let mut docs = Vec::new();
        for hit in reader.search(&query, &tantivy::TopDocs::with_limit(100))? {
            let doc = hit.1;
            docs.push(NoteDocument {
                vault_id: doc.get_first(self.schema.vault_id).unwrap().as_text().unwrap_or("").to_string(),
                path: doc.get_first(self.schema.path).unwrap().as_text().unwrap_or("").to_string(),
                title: doc.get_first(self.schema.title).unwrap().as_text().unwrap_or("").to_string(),
                content: doc.get_first(self.schema.content).unwrap().as_text().unwrap_or("").to_string(),
                tags: doc.get_first(self.schema.tags).unwrap().as_text().unwrap_or("").split_whitespace().map(|value| value.to_string()).collect(),
                mtime: doc.get_first(self.schema.mtime).unwrap().as_f64().unwrap_or(0.0),
            });
        }
        Ok(docs)
    }

    pub fn schema(&self) -> SearchSchemaDef {
        SearchSchemaDef::from_schema(self.index.schema())
    }
}
