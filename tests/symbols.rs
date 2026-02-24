use kdb::symbols::{CodeLanguage, Symbol, SymbolKind, extract_symbols};

// ---------------------------------------------------------
// tests/symbols.rs
//
// fn extract_symbols_rust_tracks_visibility()           L14
// fn extract_symbols_typescript_tracks_visibility()     L37
// fn extract_symbols_python_tracks_visibility()         L63
// fn extract_symbols_go_tracks_visibility()             L94
// fn find()                                            L115
// ---------------------------------------------------------

#[test]
fn extract_symbols_rust_tracks_visibility() {
    let source = concat!(
        "pub struct Api {}\n",
        "struct Local {}\n",
        "pub fn serve() {}\n",
        "fn hide() {}\n",
        "impl Api {\n",
        "    pub fn open(&self) {}\n",
        "    fn closed(&self) {}\n",
        "}\n",
    );

    let symbols = extract_symbols(CodeLanguage::Rust, source).expect("extract rust symbols");

    assert!(find(&symbols, SymbolKind::Struct, "Api", None).is_public);
    assert!(!find(&symbols, SymbolKind::Struct, "Local", None).is_public);
    assert!(find(&symbols, SymbolKind::Function, "serve", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "hide", None).is_public);
    assert!(find(&symbols, SymbolKind::Method, "open", Some("Api")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "closed", Some("Api")).is_public);
}

#[test]
fn extract_symbols_typescript_tracks_visibility() {
    let source = concat!(
        "export class Service {\n",
        "  run() {}\n",
        "  private hidden() {}\n",
        "}\n",
        "class Local {\n",
        "  ping() {}\n",
        "}\n",
        "export function helper() {}\n",
        "const localFn = () => 1;\n",
    );

    let symbols =
        extract_symbols(CodeLanguage::TypeScript, source).expect("extract typescript symbols");

    assert!(find(&symbols, SymbolKind::Class, "Service", None).is_public);
    assert!(!find(&symbols, SymbolKind::Class, "Local", None).is_public);
    assert!(find(&symbols, SymbolKind::Method, "run", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "hidden", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "ping", Some("Local")).is_public);
    assert!(find(&symbols, SymbolKind::Function, "helper", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "localFn", None).is_public);
}

#[test]
fn extract_symbols_python_tracks_visibility() {
    let source = concat!(
        "class Service:\n",
        "    def run(self):\n",
        "        return 1\n",
        "    def _hidden(self):\n",
        "        return 2\n",
        "\n",
        "class _Local:\n",
        "    def run(self):\n",
        "        return 3\n",
        "\n",
        "def helper():\n",
        "    return 4\n",
        "\n",
        "def _private():\n",
        "    return 5\n",
    );

    let symbols = extract_symbols(CodeLanguage::Python, source).expect("extract python symbols");

    assert!(find(&symbols, SymbolKind::Class, "Service", None).is_public);
    assert!(!find(&symbols, SymbolKind::Class, "_Local", None).is_public);
    assert!(find(&symbols, SymbolKind::Method, "run", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "_hidden", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "run", Some("_Local")).is_public);
    assert!(find(&symbols, SymbolKind::Function, "helper", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "_private", None).is_public);
}

#[test]
fn extract_symbols_go_tracks_visibility() {
    let source = concat!(
        "package main\n\n",
        "type Server struct{}\n",
        "type worker struct{}\n",
        "func Start() {}\n",
        "func stop() {}\n",
        "func (s *Server) Run() {}\n",
        "func (s *Server) hidden() {}\n",
    );

    let symbols = extract_symbols(CodeLanguage::Go, source).expect("extract go symbols");

    assert!(find(&symbols, SymbolKind::Struct, "Server", None).is_public);
    assert!(!find(&symbols, SymbolKind::Struct, "worker", None).is_public);
    assert!(find(&symbols, SymbolKind::Function, "Start", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "stop", None).is_public);
    assert!(find(&symbols, SymbolKind::Method, "Run", Some("Server")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "hidden", Some("Server")).is_public);
}

fn find<'a>(
    symbols: &'a [Symbol],
    kind: SymbolKind,
    name: &str,
    parent: Option<&str>,
) -> &'a Symbol {
    symbols
        .iter()
        .find(|symbol| {
            symbol.kind == kind && symbol.name == name && symbol.parent.as_deref() == parent
        })
        .unwrap_or_else(|| panic!("missing symbol {kind:?} {name} {parent:?}"))
}
