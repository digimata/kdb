//! Document symbols (outline view).
//!
//! Returns a hierarchical tree of headings for a markdown file, which editors
//! display in the sidebar outline panel. Headings are nested by level so that
//! `## Foo` appears as a child of the preceding `# Bar`.

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, MessageType, Position, Range,
    SymbolKind,
};

use crate::index::{Heading, parse_markdown};

use super::backend::Backend;

/// Intermediate node used while building the heading tree.
struct SymbolNode {
    level: u8,
    symbol: DocumentSymbol,
    children: Vec<usize>,
}

/// Handle a document symbol request by parsing the file and returning its
/// heading tree as nested `DocumentSymbol` entries.
pub(super) async fn document_symbol(
    backend: &Backend,
    params: DocumentSymbolParams,
) -> LspResult<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri;
    let Some((abs, _)) = backend.markdown_rel_path(&uri) else {
        return Ok(None);
    };

    let content = match backend.document_text(&uri, &abs).await {
        Some(content) => content,
        None => {
            backend
                .client
                .log_message(
                    MessageType::ERROR,
                    format!("failed to read {}", abs.display()),
                )
                .await;
            return Ok(None);
        }
    };

    let parsed = parse_markdown(&content);
    let symbols = heading_tree_symbols(&parsed.headings);
    Ok(Some(DocumentSymbolResponse::Nested(symbols)))
}

/// Build a hierarchical symbol tree from a flat list of headings.
///
/// Uses a stack to track the current nesting context. Each heading becomes a
/// child of the nearest preceding heading with a smaller level number.
fn heading_tree_symbols(headings: &[Heading]) -> Vec<DocumentSymbol> {
    let mut nodes = Vec::<SymbolNode>::new();
    let mut roots = Vec::<usize>::new();
    let mut stack = Vec::<usize>::new();

    for heading in headings {
        let line = heading.line.saturating_sub(1) as u32;
        let column = heading.column.saturating_sub(1) as u32;
        let range = Range {
            start: Position::new(line, column),
            end: Position::new(line, column),
        };

        let node = SymbolNode {
            level: heading.level,
            symbol: build_document_symbol(heading, range),
            children: Vec::new(),
        };

        let node_index = nodes.len();
        nodes.push(node);

        while let Some(parent_index) = stack.last().copied() {
            if nodes[parent_index].level >= heading.level {
                stack.pop();
            } else {
                break;
            }
        }

        if let Some(parent_index) = stack.last().copied() {
            nodes[parent_index].children.push(node_index);
        } else {
            roots.push(node_index);
        }
        stack.push(node_index);
    }

    roots
        .into_iter()
        .map(|root_index| build_symbol(root_index, &nodes))
        .collect()
}

/// Create a `DocumentSymbol` for a single heading (without children).
#[allow(deprecated)]
fn build_document_symbol(heading: &Heading, range: Range) -> DocumentSymbol {
    DocumentSymbol {
        name: heading.title.clone(),
        detail: Some(format!("h{}", heading.level)),
        kind: SymbolKind::STRING,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: None,
    }
}

/// Recursively attach children to produce the final nested `DocumentSymbol` tree.
fn build_symbol(node_index: usize, nodes: &[SymbolNode]) -> DocumentSymbol {
    let mut symbol = nodes[node_index].symbol.clone();
    let children = nodes[node_index]
        .children
        .iter()
        .map(|child_index| build_symbol(*child_index, nodes))
        .collect::<Vec<_>>();

    if !children.is_empty() {
        symbol.children = Some(children);
    }

    symbol
}
