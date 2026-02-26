use std::collections::HashSet;
use tree_sitter::Node;

use crate::lang::CodeLanguage;
use crate::symbols::{parse_tree, raw_node_text, walk_depth_first};

// -------------------------------------------------
// src/index/scanner.rs
//
// pub(super) struct IdentifierUsage             L37
// pub(super) struct UsageScanner                L46
//   pub fn new()                                L54
//   pub fn collect()                            L63
//   fn qualified_usage_for_node()              L109
//   fn rust_qualified_usage()                  L133
//   fn member_expression_usage()               L159
//   fn go_qualified_usage()                    L190
//   fn is_part_of_qualified_usage()            L210
//   fn is_usage_identifier()                   L221
// pub(super) fn rust_qualified_nodes()         L237
// pub(super) fn rust_binding_name()            L247
// fn go_qualified_nodes()                      L262
// fn is_go_qualified_binding_node()            L276
// fn is_rust_qualified_binding_node()          L292
// fn is_member_object_node()                   L307
// pub(super) fn is_import_identifier()         L319
// fn is_import_node()                          L330
// pub(super) fn is_declaration_identifier()    L348
// fn node_is_field()                           L410
// fn same_node()                               L416
// pub(super) fn scan_qualified_symbols()       L426
// -------------------------------------------------

/// A single identifier usage discovered by tree-sitter scanning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct IdentifierUsage {
    pub name: String,
    pub binding_name: Option<String>,
    pub line: usize,
    pub column: usize,
}

/// Scans a source file's AST for identifier usages that match imported names.
#[derive(Debug)]
pub(super) struct UsageScanner {
    language: CodeLanguage,
    source: String,
    imported_names: HashSet<String>,
}

impl UsageScanner {
    /// Create a new scanner for the given language and imported names.
    pub fn new(language: CodeLanguage, source: &str, imported_names: HashSet<String>) -> Self {
        Self {
            language,
            source: source.to_string(),
            imported_names,
        }
    }

    /// Collect all identifier usages matching imported names.
    ///
    /// The `tree` must have been parsed from the same source passed to `new()`.
    pub fn collect(&self, tree: &tree_sitter::Tree) -> Vec<IdentifierUsage> {
        if self.imported_names.is_empty() {
            return Vec::new();
        }

        let source_bytes = self.source.as_bytes();
        let mut usages = Vec::new();

        walk_depth_first(tree.root_node(), |node| {
            if let Some(usage) = self.qualified_usage_for_node(node, source_bytes) {
                usages.push(usage);
                return;
            }

            if !self.is_usage_identifier(node.kind()) {
                return;
            }

            if self.is_part_of_qualified_usage(node) {
                return;
            }

            let Some(name) = raw_node_text(node, source_bytes) else {
                return;
            };
            if !self.imported_names.contains(name) {
                return;
            }
            if is_import_identifier(node) || is_declaration_identifier(node) {
                return;
            }

            usages.push(IdentifierUsage {
                name: name.to_string(),
                binding_name: None,
                line: node.start_position().row + 1,
                column: node.start_position().column + 1,
            });
        });

        usages.sort();
        usages.dedup();
        usages
    }

    fn qualified_usage_for_node(
        &self,
        node: Node<'_>,
        source_bytes: &[u8],
    ) -> Option<IdentifierUsage> {
        match self.language {
            CodeLanguage::Rust => self.rust_qualified_usage(node, source_bytes),
            CodeLanguage::Go => self.go_qualified_usage(node, source_bytes),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => self
                .member_expression_usage(
                    node,
                    source_bytes,
                    "member_expression",
                    "object",
                    "property",
                ),
            CodeLanguage::Python => {
                self.member_expression_usage(node, source_bytes, "attribute", "object", "attribute")
            }
        }
    }

    /// Handle Rust module-qualified access (`module::Symbol`) discovered from
    /// imported module bindings (e.g. `use crate::module; module::Symbol`).
    fn rust_qualified_usage(&self, node: Node<'_>, source_bytes: &[u8]) -> Option<IdentifierUsage> {
        let (binding_node, symbol_node) = rust_qualified_nodes(node)?;
        let binding_name = rust_binding_name(binding_node, source_bytes)?;
        if !self.imported_names.contains(binding_name.as_str()) {
            return None;
        }

        if is_import_identifier(binding_node)
            || is_import_identifier(symbol_node)
            || is_declaration_identifier(symbol_node)
        {
            return None;
        }

        let name = raw_node_text(symbol_node, source_bytes)?;
        Some(IdentifierUsage {
            name: name.to_string(),
            binding_name: Some(binding_name),
            line: symbol_node.start_position().row + 1,
            column: symbol_node.start_position().column + 1,
        })
    }

    /// Handle qualified access via member expressions (TS/JS `obj.prop`,
    /// Python `obj.attr`). When the object matches a namespace import binding,
    /// the property/attribute is treated as a usage from the target module.
    fn member_expression_usage(
        &self,
        node: Node<'_>,
        source_bytes: &[u8],
        parent_kind: &str,
        object_field: &str,
        property_field: &str,
    ) -> Option<IdentifierUsage> {
        if node.kind() != parent_kind {
            return None;
        }
        let object_node = node.child_by_field_name(object_field)?;
        let property_node = node.child_by_field_name(property_field)?;

        let binding_name = raw_node_text(object_node, source_bytes)?;
        if !self.imported_names.contains(binding_name) {
            return None;
        }
        if is_import_identifier(object_node) {
            return None;
        }

        let name = raw_node_text(property_node, source_bytes)?;
        Some(IdentifierUsage {
            name: name.to_string(),
            binding_name: Some(binding_name.to_string()),
            line: property_node.start_position().row + 1,
            column: property_node.start_position().column + 1,
        })
    }

    fn go_qualified_usage(&self, node: Node<'_>, source_bytes: &[u8]) -> Option<IdentifierUsage> {
        let (binding_node, symbol_node) = go_qualified_nodes(node)?;
        let binding_name = raw_node_text(binding_node, source_bytes)?;
        if !self.imported_names.contains(binding_name) {
            return None;
        }

        if is_import_identifier(symbol_node) || is_declaration_identifier(symbol_node) {
            return None;
        }

        let name = raw_node_text(symbol_node, source_bytes)?;
        Some(IdentifierUsage {
            name: name.to_string(),
            binding_name: Some(binding_name.to_string()),
            line: symbol_node.start_position().row + 1,
            column: symbol_node.start_position().column + 1,
        })
    }

    fn is_part_of_qualified_usage(&self, node: Node<'_>) -> bool {
        match self.language {
            CodeLanguage::Rust => is_rust_qualified_binding_node(node),
            CodeLanguage::Go => is_go_qualified_binding_node(node),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
                is_member_object_node(node, "member_expression", "object")
            }
            CodeLanguage::Python => is_member_object_node(node, "attribute", "object"),
        }
    }

    fn is_usage_identifier(&self, kind: &str) -> bool {
        match self.language {
            CodeLanguage::Rust => matches!(kind, "identifier" | "type_identifier"),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
                matches!(kind, "identifier" | "type_identifier" | "jsx_identifier")
            }
            CodeLanguage::Python => kind == "identifier",
            CodeLanguage::Go => matches!(kind, "identifier" | "type_identifier"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tree-sitter node helpers
// ---------------------------------------------------------------------------

pub(super) fn rust_qualified_nodes(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    match node.kind() {
        "scoped_identifier" | "scoped_type_identifier" => Some((
            node.child_by_field_name("path")?,
            node.child_by_field_name("name")?,
        )),
        _ => None,
    }
}

pub(super) fn rust_binding_name(path_node: Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let mut current = path_node;
    loop {
        match current.kind() {
            "scoped_identifier" | "scoped_type_identifier" => {
                current = current.child_by_field_name("path")?;
            }
            _ => {
                let text = raw_node_text(current, source_bytes)?;
                return Some(text.to_string());
            }
        }
    }
}

fn go_qualified_nodes(node: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    match node.kind() {
        "selector_expression" => Some((
            node.child_by_field_name("operand")?,
            node.child_by_field_name("field")?,
        )),
        "qualified_type" => Some((
            node.child_by_field_name("package")?,
            node.child_by_field_name("name")?,
        )),
        _ => None,
    }
}

fn is_go_qualified_binding_node(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    match parent.kind() {
        "selector_expression" => parent
            .child_by_field_name("operand")
            .is_some_and(|operand| same_node(operand, node)),
        "qualified_type" => parent
            .child_by_field_name("package")
            .is_some_and(|package| same_node(package, node)),
        _ => false,
    }
}

fn is_rust_qualified_binding_node(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    match parent.kind() {
        "scoped_identifier" | "scoped_type_identifier" => parent
            .child_by_field_name("path")
            .is_some_and(|path| same_node(path, node)),
        _ => false,
    }
}

/// Check if `node` is the object side of a member expression / attribute
/// access, which means it should be skipped as a bare usage.
fn is_member_object_node(node: Node<'_>, parent_kind: &str, object_field: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != parent_kind {
        return false;
    }
    parent
        .child_by_field_name(object_field)
        .is_some_and(|obj| same_node(obj, node))
}

pub(super) fn is_import_identifier(node: Node<'_>) -> bool {
    let mut cursor = Some(node);
    while let Some(current) = cursor {
        if is_import_node(current.kind()) {
            return true;
        }
        cursor = current.parent();
    }
    false
}

fn is_import_node(kind: &str) -> bool {
    matches!(
        kind,
        "import_statement"
            | "import_clause"
            | "import_specifier"
            | "namespace_import"
            | "named_imports"
            | "import_declaration"
            | "import_spec"
            | "import_from_statement"
            | "aliased_import"
            | "use_declaration"
            | "extern_crate_declaration"
            | "mod_item"
    )
}

pub(super) fn is_declaration_identifier(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if matches!(
        parent.kind(),
        "qualified_type" | "scoped_identifier" | "scoped_type_identifier"
    ) {
        return false;
    }

    // JSX element tags use the `name` field, but they're usages, not declarations.
    if matches!(
        parent.kind(),
        "jsx_opening_element" | "jsx_closing_element" | "jsx_self_closing_element"
    ) {
        return false;
    }

    if node_is_field(parent, "name", node)
        || node_is_field(parent, "alias", node)
        || node_is_field(parent, "parameter", node)
        || node_is_field(parent, "pattern", node)
    {
        return true;
    }

    matches!(
        parent.kind(),
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_definition"
            | "method_declaration"
            | "class_definition"
            | "class_declaration"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "type_spec"
            | "interface_declaration"
            | "type_alias_declaration"
            | "variable_declarator"
            | "lexical_declaration"
            | "variable_declaration"
            | "const_spec"
            | "var_spec"
            | "short_var_declaration"
            | "parameters"
            | "formal_parameters"
            | "parameter_list"
            | "required_parameter"
            | "optional_parameter"
            | "typed_parameter"
            | "receiver"
            | "pair_pattern"
            | "assignment_pattern"
    )
}

fn node_is_field(parent: Node<'_>, field: &str, node: Node<'_>) -> bool {
    parent
        .child_by_field_name(field)
        .is_some_and(|field_node| same_node(field_node, node))
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
        && left.kind() == right.kind()
}

/// Scan source for qualified access patterns where the qualifier matches
/// `binding_name`. Returns the set of accessed symbol names.
///
/// Handles `qualifier::Name` (Rust), `qualifier.name` (TS/Python/Go).
pub(super) fn scan_qualified_symbols(
    language: CodeLanguage,
    source: &str,
    binding_name: &str,
) -> HashSet<String> {
    let mut accessed = HashSet::new();
    let Ok(tree) = parse_tree(language, source) else {
        return accessed;
    };
    let source_bytes = source.as_bytes();

    walk_depth_first(tree.root_node(), |node| match language {
        CodeLanguage::Rust => {
            let Some((path_node, name_node)) = rust_qualified_nodes(node) else {
                return;
            };
            let Some(root_name) = rust_binding_name(path_node, source_bytes) else {
                return;
            };
            if root_name != binding_name {
                return;
            }
            if is_import_identifier(path_node) || is_import_identifier(name_node) {
                return;
            }
            if let Some(name) = raw_node_text(name_node, source_bytes) {
                accessed.insert(name.to_string());
            }
        }
        CodeLanguage::Go => {
            let Some((operand, field)) = go_qualified_nodes(node) else {
                return;
            };
            let Some(name) = raw_node_text(operand, source_bytes) else {
                return;
            };
            if name != binding_name {
                return;
            }
            if let Some(sym) = raw_node_text(field, source_bytes) {
                accessed.insert(sym.to_string());
            }
        }
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            if node.kind() != "member_expression" {
                return;
            }
            let Some(obj) = node.child_by_field_name("object") else {
                return;
            };
            let Some(prop) = node.child_by_field_name("property") else {
                return;
            };
            let Some(obj_name) = raw_node_text(obj, source_bytes) else {
                return;
            };
            if obj_name != binding_name {
                return;
            }
            if let Some(sym) = raw_node_text(prop, source_bytes) {
                accessed.insert(sym.to_string());
            }
        }
        CodeLanguage::Python => {
            if node.kind() != "attribute" {
                return;
            }
            let Some(obj) = node.child_by_field_name("object") else {
                return;
            };
            let Some(attr) = node.child_by_field_name("attribute") else {
                return;
            };
            let Some(obj_name) = raw_node_text(obj, source_bytes) else {
                return;
            };
            if obj_name != binding_name {
                return;
            }
            if let Some(sym) = raw_node_text(attr, source_bytes) {
                accessed.insert(sym.to_string());
            }
        }
    });

    accessed
}
