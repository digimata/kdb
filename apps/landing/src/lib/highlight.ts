import { codeToHtml } from "shiki";
import type { ThemeRegistration } from "shiki";

/*
 * Shiki doesn't ship an official "vercel" theme. These are minimal
 * TextMate themes that approximate Vercel's code blocks — the accent
 * teal (#067A6E light / #50E3C2 dark) is the defining touch.
 */

const vercelDark: ThemeRegistration = {
  name: "vercel-dark",
  type: "dark",
  colors: {
    "editor.foreground": "#ededed",
    "editor.background": "#08090a",
  },
  tokenColors: [
    {
      scope: ["comment", "punctuation.definition.comment"],
      settings: { foreground: "#666666", fontStyle: "italic" },
    },
    {
      scope: [
        "string",
        "meta.string",
        "string.quoted",
        "string.unquoted",
        "constant.character",
      ],
      settings: { foreground: "#50E3C2" },
    },
    {
      scope: ["constant.numeric", "constant.language", "support.constant"],
      settings: { foreground: "#79FFE1" },
    },
    {
      scope: ["keyword", "storage", "storage.type", "keyword.control"],
      settings: { foreground: "#FF4ECD" },
    },
    {
      scope: ["entity.name.function", "meta.function-call", "support.function"],
      settings: { foreground: "#F5A623" },
    },
    {
      scope: ["variable", "meta.variable", "support.variable"],
      settings: { foreground: "#ededed" },
    },
    {
      scope: ["variable.parameter", "entity.name.type", "support.class"],
      settings: { foreground: "#7CE38B" },
    },
    {
      scope: ["punctuation", "meta.brace", "meta.bracket", "meta.delimiter"],
      settings: { foreground: "#888888" },
    },
    {
      scope: ["markup.heading", "entity.name.section"],
      settings: { foreground: "#50E3C2", fontStyle: "bold" },
    },
    {
      scope: ["markup.bold"],
      settings: { foreground: "#ededed", fontStyle: "bold" },
    },
    {
      scope: ["markup.italic"],
      settings: { foreground: "#ededed", fontStyle: "italic" },
    },
    {
      scope: ["markup.inline.raw", "markup.fenced_code"],
      settings: { foreground: "#79FFE1" },
    },
    {
      scope: ["entity.name.tag"],
      settings: { foreground: "#FF4ECD" },
    },
    {
      scope: [
        "entity.other.attribute-name",
        "meta.attribute",
      ],
      settings: { foreground: "#F5A623" },
    },
  ],
};

const vercelLight: ThemeRegistration = {
  name: "vercel-light",
  type: "light",
  colors: {
    "editor.foreground": "#171717",
    "editor.background": "#fafafa",
  },
  tokenColors: [
    {
      scope: ["comment", "punctuation.definition.comment"],
      settings: { foreground: "#8f8f8f", fontStyle: "italic" },
    },
    {
      scope: [
        "string",
        "meta.string",
        "string.quoted",
        "string.unquoted",
        "constant.character",
      ],
      settings: { foreground: "#067A6E" },
    },
    {
      scope: ["constant.numeric", "constant.language", "support.constant"],
      settings: { foreground: "#067A6E" },
    },
    {
      scope: ["keyword", "storage", "storage.type", "keyword.control"],
      settings: { foreground: "#C02872" },
    },
    {
      scope: ["entity.name.function", "meta.function-call", "support.function"],
      settings: { foreground: "#A75900" },
    },
    {
      scope: ["variable", "meta.variable", "support.variable"],
      settings: { foreground: "#171717" },
    },
    {
      scope: ["variable.parameter", "entity.name.type", "support.class"],
      settings: { foreground: "#1A7F37" },
    },
    {
      scope: ["punctuation", "meta.brace", "meta.bracket", "meta.delimiter"],
      settings: { foreground: "#6e6e6e" },
    },
    {
      scope: ["markup.heading", "entity.name.section"],
      settings: { foreground: "#067A6E", fontStyle: "bold" },
    },
    {
      scope: ["markup.bold"],
      settings: { foreground: "#171717", fontStyle: "bold" },
    },
    {
      scope: ["markup.italic"],
      settings: { foreground: "#171717", fontStyle: "italic" },
    },
    {
      scope: ["markup.inline.raw", "markup.fenced_code"],
      settings: { foreground: "#067A6E" },
    },
    {
      scope: ["entity.name.tag"],
      settings: { foreground: "#C02872" },
    },
    {
      scope: [
        "entity.other.attribute-name",
        "meta.attribute",
      ],
      settings: { foreground: "#A75900" },
    },
  ],
};

const KDB_SPAN =
  '<span style="--shiki-light:#067A6E;--shiki-dark:#50E3C2;color:var(--shiki-dark)">kdb</span>';

/*
 * Shellsession tokenization lumps the whole command line into a single span,
 * so we recolor the word `kdb` by splitting any span that contains it into
 * three: prefix · kdb · suffix. Word-bounded so subcommands and args stay put.
 */
function accentKdb(html: string): string {
  const spanRe = /<span style="([^"]*)">([^<]*)<\/span>/g;
  const kdbRe = /\bkdb\b/;
  return html.replace(spanRe, (match, style, text) => {
    const m = kdbRe.exec(text);
    if (!m) return match;
    const before = text.slice(0, m.index);
    const after = text.slice(m.index + m[0].length);
    const prefix = before ? `<span style="${style}">${before}</span>` : "";
    const suffix = after ? `<span style="${style}">${after}</span>` : "";
    return `${prefix}${KDB_SPAN}${suffix}`;
  });
}

export async function highlight(
  code: string,
  lang: string = "shellsession",
): Promise<string> {
  const html = await codeToHtml(code, {
    lang,
    themes: {
      light: vercelLight,
      dark: vercelDark,
    },
    defaultColor: "dark",
  });
  return accentKdb(html);
}
