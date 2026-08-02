import type { ReactNode } from "react";
import styles from "./ShellCode.module.css";

type TokenKind = "comment" | "string" | "variable" | "keyword" | "text";

type Token = { kind: TokenKind; value: string };

const KEYWORDS = new Set([
  "set", "curl", "cat", "openssl", "sh", "trap", "mktemp", "rm", "cd",
  "env", "echo", "export", "if", "then", "else", "elif", "fi", "for",
  "while", "do", "done", "case", "esac", "local", "true", "false",
  "printf", "read", "exit", "unset", "command", "type",
]);

function tokenize(code: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < code.length) {
    const rest = code.slice(i);

    const comment = rest.match(/^#.*/);
    if (comment) {
      tokens.push({ kind: "comment", value: comment[0] });
      i += comment[0].length;
      continue;
    }

    const single = rest.match(/^'(?:[^'\\]|\\.)*'?/);
    if (single) {
      tokens.push({ kind: "string", value: single[0] });
      i += single[0].length;
      continue;
    }

    if (rest[0] === '"') {
      let j = 1;
      let buf = '"';
      while (j < code.length) {
        const ch = code[i + j];
        if (ch === "\\" && j + 1 < code.length) {
          buf += ch + code[i + j + 1];
          j += 2;
          continue;
        }
        if (ch === '"') {
          buf += '"';
          j += 1;
          break;
        }
        if (ch === "$") {
          const variable = code
            .slice(i + j)
            .match(/^\$(?:\{[^}]*\}|[A-Za-z_][A-Za-z0-9_]*)/);
          if (variable) {
            if (buf) {
              tokens.push({ kind: "string", value: buf });
              buf = "";
            }
            tokens.push({ kind: "variable", value: variable[0] });
            j += variable[0].length;
            continue;
          }
        }
        buf += ch;
        j += 1;
      }
      tokens.push({ kind: "string", value: buf });
      i += j;
      continue;
    }

    const variable = rest.match(/^\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*/);
    if (variable) {
      tokens.push({ kind: "variable", value: variable[0] });
      i += variable[0].length;
      continue;
    }

    const word = rest.match(/^[A-Za-z_][A-Za-z0-9_-]*/);
    if (word && KEYWORDS.has(word[0])) {
      tokens.push({ kind: "keyword", value: word[0] });
      i += word[0].length;
      continue;
    }

    tokens.push({ kind: "text", value: rest[0] });
    i += 1;
  }
  return tokens;
}

const STYLE_BY_KIND: Record<TokenKind, string> = {
  comment: styles.comment,
  string: styles.string,
  variable: styles.variable,
  keyword: styles.keyword,
  text: styles.text,
};

export function ShellCode({ code }: { code: string }) {
  const nodes: ReactNode[] = tokenize(code).map((token, index) => (
    <span className={STYLE_BY_KIND[token.kind]} key={index}>
      {token.value ?? ""}
    </span>
  ));
  return <>{nodes}</>;
}
