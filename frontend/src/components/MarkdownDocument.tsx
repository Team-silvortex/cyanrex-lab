import Link from "next/link";
import { Fragment, ReactNode } from "react";

type Props = {
  markdown: string;
  currentSlug: string;
};

export default function MarkdownDocument({ markdown, currentSlug }: Props) {
  const lines = markdown.replace(/\r\n/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    if (!line.trim()) {
      index += 1;
      continue;
    }

    if (line.startsWith("```")) {
      const language = line.slice(3).trim();
      const content: string[] = [];
      index += 1;
      while (index < lines.length && !lines[index].startsWith("```")) {
        content.push(lines[index]);
        index += 1;
      }
      index += 1;
      blocks.push(<pre key={`code-${index}`}><code data-language={language}>{content.join("\n")}</code></pre>);
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      const level = heading[1].length;
      const content = inlineMarkdown(heading[2], currentSlug);
      const key = `heading-${index}`;
      blocks.push(level === 1 ? <h1 key={key}>{content}</h1>
        : level === 2 ? <h2 key={key}>{content}</h2>
          : level === 3 ? <h3 key={key}>{content}</h3>
            : <h4 key={key}>{content}</h4>);
      index += 1;
      continue;
    }

    if (isTableHeader(lines, index)) {
      const headers = tableCells(lines[index]);
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length && /^\s*\|.*\|\s*$/.test(lines[index])) {
        rows.push(tableCells(lines[index]));
        index += 1;
      }
      blocks.push(
        <div key={`table-${index}`} style={{ overflowX: "auto" }}>
          <table><thead><tr>{headers.map((cell, cellIndex) => <th key={cellIndex}>{inlineMarkdown(cell, currentSlug)}</th>)}</tr></thead>
            <tbody>{rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}>{inlineMarkdown(cell, currentSlug)}</td>)}</tr>)}</tbody>
          </table>
        </div>,
      );
      continue;
    }

    if (/^\s*[-*]\s+/.test(line) || /^\s*\d+\.\s+/.test(line)) {
      const ordered = /^\s*\d+\./.test(line);
      const items: string[] = [];
      const pattern = ordered ? /^\s*\d+\.\s+(.+)$/ : /^\s*[-*]\s+(.+)$/;
      while (index < lines.length) {
        const item = lines[index].match(pattern);
        if (!item) break;
        items.push(item[1]);
        index += 1;
      }
      const children = items.map((item, itemIndex) => <li key={itemIndex}>{inlineMarkdown(item, currentSlug)}</li>);
      blocks.push(ordered ? <ol key={`list-${index}`}>{children}</ol> : <ul key={`list-${index}`}>{children}</ul>);
      continue;
    }

    const paragraph = [line.trim()];
    index += 1;
    while (index < lines.length && lines[index].trim() && !startsBlock(lines, index)) {
      paragraph.push(lines[index].trim());
      index += 1;
    }
    blocks.push(<p key={`paragraph-${index}`}>{inlineMarkdown(paragraph.join(" "), currentSlug)}</p>);
  }

  return <article className="markdown-document">{blocks}</article>;
}

function inlineMarkdown(text: string, currentSlug: string): ReactNode[] {
  const pattern = /(\[[^\]]+\]\([^)]+\)|`[^`]+`|\*\*[^*]+\*\*)/g;
  return text.split(pattern).filter(Boolean).map((part, index) => {
    const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (link) {
      if (/^https?:\/\//.test(link[2])) {
        return <a key={index} href={link[2]} target="_blank" rel="noreferrer">{link[1]}</a>;
      }
      return <Link key={index} href={resolveDocLink(currentSlug, link[2])}>{link[1]}</Link>;
    }
    if (part.startsWith("`") && part.endsWith("`")) return <code key={index}>{part.slice(1, -1)}</code>;
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={index}>{part.slice(2, -2)}</strong>;
    return <Fragment key={index}>{part}</Fragment>;
  });
}

function resolveDocLink(currentSlug: string, target: string): string {
  const clean = target.split("#", 1)[0];
  const base = currentSlug.split("/").slice(0, -1);
  for (const segment of clean.split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") base.pop();
    else base.push(segment);
  }
  return `/learn/${base.join("/").replace(/\.md$/, "")}`;
}

function startsBlock(lines: string[], index: number): boolean {
  const line = lines[index];
  return /^(#{1,4})\s+/.test(line) || line.startsWith("```")
    || /^\s*[-*]\s+/.test(line) || /^\s*\d+\.\s+/.test(line) || isTableHeader(lines, index);
}

function isTableHeader(lines: string[], index: number): boolean {
  return /^\s*\|.*\|\s*$/.test(lines[index] ?? "")
    && /^\s*\|(?:\s*:?-+:?\s*\|)+\s*$/.test(lines[index + 1] ?? "");
}

function tableCells(line: string): string[] {
  return line.trim().replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim());
}
