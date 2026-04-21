/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// projects/kdb/grammars/tree-sitter-prosaic/grammar.js
//

module.exports = grammar({
  name: "prosaic",

  extras: ($) => [/[ \t]/],

  rules: {
    source_file: ($) => repeat(choice($._line, $._newline)),

    _newline: (_) => /\n/,

    _line: ($) =>
      choice(
        $.comment,
        $.block_label_line,
        $.for_statement,
        $.if_statement,
        $.statement,
        $.plain_line,
      ),

    // /* ... */ comments (can span multiple lines)
    comment: ($) =>
      seq(
        "/*",
        repeat(
          choice(
            $.annotation,
            /[^*()/]/,
            /\*[^/]/,
            /\//,
          ),
        ),
        "*/",
      ),

    // (parenthesized text) inside comments
    annotation: (_) => /\([^)]+\)/,

    // label: at start of line, optionally followed by tokens
    block_label_line: ($) =>
      seq($.block_label, repeat($._token), $._newline),

    block_label: (_) => /[a-z_]+:/,

    // for each ... :
    for_statement: ($) =>
      seq("for", repeat1($._token), $._newline),

    // if ... :
    if_statement: ($) =>
      seq("if", repeat1($._token), $._newline),

    // first word is the action verb, rest are tokens
    statement: ($) =>
      seq($.action_verb, repeat($._token), $._newline),

    // fallback for lines starting with non-word chars (e.g. ## Schedule)
    plain_line: ($) =>
      seq(repeat1($._token), $._newline),

    _token: ($) =>
      choice(
        $.control_keyword,
        $.template_var,
        $.file_path,
        $.operator,
        $.identifier,
        $.word,
      ),

    // first word of a statement / prompt
    action_verb: (_) => /[a-zA-Z_][a-zA-Z0-9_]*/,

    control_keyword: (_) =>
      choice("each", "break"),

    template_var: (_) => /\{[a-zA-Z_][a-zA-Z0-9_.]*\}/,

    // paths: dot-prefixed (.tasks/TODO.md) or contain a slash (src/main.rs)
    file_path: (_) =>
      token(choice(
        /\.[a-zA-Z0-9_/{}.-]+/,
        /[a-zA-Z0-9_{}.-]+\/[a-zA-Z0-9_/{}.-]*/,
      )),

    operator: (_) => choice("→", "=", "×"),

    // word ending with colon (not at line start, so won't be block_label)
    identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_]*:/,

    word: (_) => token(prec(-1, /[^\s{}\n]+/)),
  },
});
