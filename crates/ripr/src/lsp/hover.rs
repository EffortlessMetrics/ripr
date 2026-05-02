use super::HOVER_TEXT;
use tower_lsp_server::ls_types::{Hover, HoverContents, MarkupContent, MarkupKind};

pub(super) fn hover_response() -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: HOVER_TEXT.to_string(),
        }),
        range: None,
    }
}
