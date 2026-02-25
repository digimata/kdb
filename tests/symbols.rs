use kdb::lang::CodeLanguage;
use kdb::symbols::{extract_symbols, Symbol, SymbolKind};

// --------------------------------------------------------------------------
// tests/symbols.rs
//
// fn extract_symbols_rust_tracks_visibility_and_display_kind()           L14
// fn extract_symbols_typescript_tracks_visibility_and_display_kind()     L76
// fn extract_symbols_python_tracks_visibility_and_display_kind()        L154
// fn extract_symbols_go_tracks_visibility_and_display_kind()            L225
// fn find()                                                             L274
// --------------------------------------------------------------------------

#[test]
fn extract_symbols_rust_tracks_visibility_and_display_kind() {
    let source = concat!(
        "pub struct Api {}\n",
        "struct Local {}\n",
        "pub fn serve() {}\n",
        "fn hide() {}\n",
        "pub const VERSION: &str = \"1\";\n",
        "static mut COUNT: i32 = 0;\n",
        "pub mod inner {}\n",
        "macro_rules! make_api { () => {} }\n",
        "impl Api {\n",
        "    pub fn open(&self) {}\n",
        "    fn closed(&self) {}\n",
        "}\n",
    );

    let symbols = extract_symbols(CodeLanguage::Rust, source).expect("extract rust symbols");

    assert!(find(&symbols, SymbolKind::Struct, "Api", None).is_public);
    assert!(!find(&symbols, SymbolKind::Struct, "Local", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Struct, "Api", None).display_kind,
        "pub struct"
    );

    assert!(find(&symbols, SymbolKind::Function, "serve", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "hide", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Function, "serve", None).display_kind,
        "pub fn"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Function, "hide", None).display_kind,
        "fn"
    );

    assert!(find(&symbols, SymbolKind::Method, "open", Some("Api")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "closed", Some("Api")).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Method, "open", Some("Api")).display_kind,
        "pub fn"
    );

    assert_eq!(
        find(&symbols, SymbolKind::Const, "VERSION", None).display_kind,
        "pub const"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Static, "COUNT", None).display_kind,
        "static mut"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Module, "inner", None).display_kind,
        "pub mod"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Macro, "make_api", None).display_kind,
        "macro_rules!"
    );
}

#[test]
fn extract_symbols_typescript_tracks_visibility_and_display_kind() {
    let source = concat!(
        "export class Service {\n",
        "  readonly kind = 'service';\n",
        "  constructor() {}\n",
        "  get threads() { return 1; }\n",
        "  async run() {}\n",
        "  private hidden() {}\n",
        "}\n",
        "class Local {\n",
        "  ping() {}\n",
        "}\n",
        "export async function helper() {}\n",
        "const localFn = () => 1;\n",
        "export let version = 1;\n",
    );

    let symbols =
        extract_symbols(CodeLanguage::TypeScript, source).expect("extract typescript symbols");

    assert!(find(&symbols, SymbolKind::Class, "Service", None).is_public);
    assert!(!find(&symbols, SymbolKind::Class, "Local", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Class, "Service", None).display_kind,
        "export class"
    );

    assert_eq!(
        find(&symbols, SymbolKind::Property, "kind", Some("Service")).display_kind,
        "readonly"
    );
    assert_eq!(
        find(
            &symbols,
            SymbolKind::Constructor,
            "constructor",
            Some("Service")
        )
        .display_kind,
        "constructor"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Getter, "threads", Some("Service")).display_kind,
        "get"
    );

    assert!(find(&symbols, SymbolKind::Method, "run", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "hidden", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "ping", Some("Local")).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Method, "run", Some("Service")).display_kind,
        "async"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Method, "hidden", Some("Service")).display_kind,
        "private"
    );

    assert!(find(&symbols, SymbolKind::Function, "helper", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Function, "helper", None).display_kind,
        "export async function"
    );

    assert!(!find(&symbols, SymbolKind::Const, "localFn", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Const, "localFn", None).display_kind,
        "const"
    );

    assert!(find(&symbols, SymbolKind::Variable, "version", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Variable, "version", None).display_kind,
        "export let"
    );
}

#[test]
fn extract_symbols_python_tracks_visibility_and_display_kind() {
    let source = concat!(
        "class Service:\n",
        "    def __init__(self):\n",
        "        self._status = 0\n",
        "    async def run(self):\n",
        "        return 1\n",
        "    def _hidden(self):\n",
        "        return 2\n",
        "    @property\n",
        "    def status(self):\n",
        "        return self._status\n",
        "    @status.setter\n",
        "    def status(self, value):\n",
        "        self._status = value\n",
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
    assert_eq!(
        find(&symbols, SymbolKind::Class, "Service", None).display_kind,
        "class"
    );

    assert_eq!(
        find(
            &symbols,
            SymbolKind::Constructor,
            "__init__",
            Some("Service")
        )
        .display_kind,
        "def"
    );
    assert!(find(&symbols, SymbolKind::Method, "run", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "_hidden", Some("Service")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "run", Some("_Local")).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Method, "run", Some("Service")).display_kind,
        "async def"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Getter, "status", Some("Service")).display_kind,
        "@property def"
    );
    assert_eq!(
        find(&symbols, SymbolKind::Setter, "status", Some("Service")).display_kind,
        "@setter def"
    );

    assert!(find(&symbols, SymbolKind::Function, "helper", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "_private", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Function, "helper", None).display_kind,
        "def"
    );
}

#[test]
fn extract_symbols_go_tracks_visibility_and_display_kind() {
    let source = concat!(
        "package main\n\n",
        "type Server struct{}\n",
        "type worker struct{}\n",
        "func Start() {}\n",
        "func stop() {}\n",
        "func (s *Server) Run() {}\n",
        "func (s *Server) hidden() {}\n",
        "const AppName = \"kdb\"\n",
        "var local = 1\n",
    );

    let symbols = extract_symbols(CodeLanguage::Go, source).expect("extract go symbols");

    assert!(find(&symbols, SymbolKind::Struct, "Server", None).is_public);
    assert!(!find(&symbols, SymbolKind::Struct, "worker", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Struct, "Server", None).display_kind,
        "type struct"
    );

    assert!(find(&symbols, SymbolKind::Function, "Start", None).is_public);
    assert!(!find(&symbols, SymbolKind::Function, "stop", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Function, "Start", None).display_kind,
        "func"
    );

    assert!(find(&symbols, SymbolKind::Method, "Run", Some("Server")).is_public);
    assert!(!find(&symbols, SymbolKind::Method, "hidden", Some("Server")).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Method, "Run", Some("Server")).display_kind,
        "func"
    );

    assert!(find(&symbols, SymbolKind::Const, "AppName", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Const, "AppName", None).display_kind,
        "const"
    );

    assert!(!find(&symbols, SymbolKind::Variable, "local", None).is_public);
    assert_eq!(
        find(&symbols, SymbolKind::Variable, "local", None).display_kind,
        "var"
    );
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
