/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const ACTION_VERBS = [
  "copy",
  "write",
  "create",
  "mark",
  "check",
  "print",
  "list",
  "update",
  "sort",
  "tag",
  "assign",
  "prune",
  "classify",
  "pick",
  "take",
  "allocate",
  "link",
  "group",
  "inventory",
  "note",
  "review",
];

module.exports = grammar({
  name: "prosaic",

  extras: ($) => [/[ \t]/],

  rules: {
    source_file: ($) => repeat(choice($._line, $._newline)),

    _newline: (_) => /\n/,

    _line: ($) =>
      choice($.comment, $.block_label_line, $.statement),

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

    // a statement is one or more tokens on a line
    statement: ($) =>
      seq($._token, repeat($._token), $._newline),

    _token: ($) =>
      choice(
        $.control_keyword,
        $.action_verb,
        $.template_var,
        $.file_path,
        $.operator,
        $.identifier,
        $.word,
      ),

    control_keyword: (_) =>
      choice("for each", "for", "if", "in", "break"),

    action_verb: (_) =>
      choice(...ACTION_VERBS),

    template_var: (_) => /\{[a-zA-Z_][a-zA-Z0-9_.]*\}/,

    file_path: (_) => /\.[a-zA-Z0-9_/{}.-]+/,

    operator: (_) => choice("→", "=", "×"),

    // word ending with colon (not at line start, so won't be block_label)
    identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_]*:/,

    word: (_) => token(prec(-1, /[^\s{}\n]+/)),
  },
});
