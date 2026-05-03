use super::uri::path_from_file_uri;
use crate::app::Mode;
use crate::domain::Finding;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tower_lsp_server::ls_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    Uri,
};

#[derive(Clone, Debug)]
pub(super) struct AnalysisSnapshot {
    pub(super) root: PathBuf,
    pub(super) base: Option<String>,
    pub(super) mode: Mode,
    pub(super) findings: Vec<Finding>,
    pub(super) diagnostics_by_uri: BTreeMap<Uri, Vec<Diagnostic>>,
}

impl AnalysisSnapshot {
    pub(super) fn is_consistent(&self) -> bool {
        let diagnostic_count = self
            .diagnostics_by_uri
            .values()
            .map(Vec::len)
            .sum::<usize>();
        !self.root.as_os_str().is_empty()
            && self
                .base
                .as_ref()
                .is_none_or(|base| !base.trim().is_empty())
            && !self.mode.as_str().is_empty()
            && self.findings.len() == diagnostic_count
    }

    pub(super) fn diagnostics_for_uri(&self, uri: &Uri) -> Option<&[Diagnostic]> {
        self.diagnostics_by_uri.get(uri).map(Vec::as_slice)
    }

    pub(super) fn finding_by_id(&self, finding_id: &str) -> Option<&Finding> {
        self.findings
            .iter()
            .find(|finding| finding.id == finding_id)
    }

    pub(super) fn finding_for_diagnostic(&self, diagnostic: &Diagnostic) -> Option<&Finding> {
        let finding_id = diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("finding_id"))
            .and_then(|value| value.as_str())?;
        self.finding_by_id(finding_id)
    }
}

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
