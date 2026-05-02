use super::uri::path_from_file_uri;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tower_lsp_server::ls_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, Uri,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DocumentState {
    pub(super) uri: Uri,
    pub(super) path: PathBuf,
    pub(super) version: Option<i32>,
    pub(super) text: String,
}

#[derive(Default)]
pub(super) struct DocumentStore {
    pub(super) documents: BTreeMap<Uri, DocumentState>,
}

impl DocumentStore {
    pub(super) fn open(&mut self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let state = DocumentState {
            path: document_path(&uri),
            uri: uri.clone(),
            version: Some(params.text_document.version),
            text: params.text_document.text,
        };
        self.documents.insert(uri, state);
    }

    pub(super) fn change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = Some(params.text_document.version);
        let text = params
            .content_changes
            .into_iter()
            .last()
            .map(|change| change.text);
        if let Some(state) = self.documents.get_mut(&uri) {
            state.version = version;
            if let Some(text) = text {
                state.text = text;
            }
            return;
        }
        let Some(text) = text else {
            return;
        };
        let state = DocumentState {
            path: document_path(&uri),
            uri: uri.clone(),
            version,
            text,
        };
        self.documents.insert(uri, state);
    }

    pub(super) fn close(&mut self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }
}

fn document_path(uri: &Uri) -> PathBuf {
    path_from_file_uri(uri).unwrap_or_else(|| PathBuf::from(uri.as_str()))
}
