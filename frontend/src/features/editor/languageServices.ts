import type * as Monaco from "monaco-editor";

const configured = new WeakSet<typeof Monaco>();

/** Configure bundled browser workers once. No Engine, external LSP, package acquisition or execution. */
export function configureBuiltinLanguageServices(monaco: typeof Monaco): void {
  if (configured.has(monaco)) return;
  const ts = monaco.typescript;
  for (const [defaults, javascript] of [
    [ts.typescriptDefaults, false], [ts.javascriptDefaults, true],
  ] as const) {
    defaults.setCompilerOptions({
      strict: true,
      noEmit: true,
      target: ts.ScriptTarget.ESNext,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.NodeJs,
      // The bundled TypeScript's ModuleDetectionKind.Force is 3; Monaco omits that enum export.
      // Each scratch document is a module, not a global namespace shared with other models.
      moduleDetection: 3,
      allowNonTsExtensions: true,
      allowJs: javascript,
      checkJs: javascript,
      types: [],
      typeRoots: [],
    });
    // This worker reads mirrored models/bundled lib files only; it is not a tsserver ATA client.
    defaults.setWorkerOptions({});
    defaults.setEagerModelSync(false);
    defaults.setDiagnosticsOptions({ noSemanticValidation: false, noSyntaxValidation: false, onlyVisible: true });
    defaults.setModeConfiguration({
      completionItems: true, hovers: true, signatureHelp: true, definitions: true,
      references: true, rename: true, documentSymbols: true, documentHighlights: true,
      diagnostics: true, documentRangeFormattingEdits: true, onTypeFormattingEdits: true,
      codeActions: true, inlayHints: true,
    });
  }
  monaco.json.jsonDefaults.setDiagnosticsOptions({
    validate: true, allowComments: false, enableSchemaRequest: false, schemas: [],
    schemaRequest: "ignore", schemaValidation: "warning",
  });
  monaco.json.jsonDefaults.setModeConfiguration({
    completionItems: true, hovers: true, documentSymbols: true, diagnostics: true,
    documentFormattingEdits: true, documentRangeFormattingEdits: true, tokens: true,
    foldingRanges: true, selectionRanges: true, colors: true,
  });
  monaco.html.htmlDefaults.setOptions({ data: { useDefaultDataProvider: true, dataProviders: {} } });
  monaco.html.htmlDefaults.setModeConfiguration({
    completionItems: true, hovers: true, documentSymbols: true, rename: true,
    documentHighlights: true, documentFormattingEdits: true, documentRangeFormattingEdits: true,
    foldingRanges: true, selectionRanges: true, links: false, diagnostics: false,
  });
  monaco.css.cssDefaults.setOptions({
    validate: true, data: { useDefaultDataProvider: true, dataProviders: {} },
  });
  monaco.css.cssDefaults.setModeConfiguration({
    completionItems: true, hovers: true, definitions: true, references: true, rename: true,
    documentSymbols: true, documentHighlights: true, diagnostics: true,
    documentFormattingEdits: true, documentRangeFormattingEdits: true,
    foldingRanges: true, selectionRanges: true, colors: true,
  });
  configured.add(monaco);
}

type Snippet = { label: string; detail: string; insertText: string };
// Small explicit templates, not a parser: no diagnostics, guessed symbols, project search or I/O.
const snippets: Readonly<Record<string, readonly Snippet[]>> = {
  rust: [
    { label: "fn", detail: "Rust function snippet", insertText: "fn ${1:name}(${2}) {\n\t${0}\n}" },
    { label: "let", detail: "Rust binding snippet", insertText: "let ${1:name} = ${2:value};" },
    { label: "match", detail: "Rust match snippet", insertText: "match ${1:value} {\n\t${2:pattern} => ${3:result},\n\t_ => ${0},\n}" },
  ],
  python: [
    { label: "def", detail: "Python function snippet", insertText: "def ${1:name}(${2}):\n\t${0:pass}" },
    { label: "class", detail: "Python class snippet", insertText: "class ${1:Name}:\n\tdef __init__(self${2}):\n\t\t${0:pass}" },
    { label: "for", detail: "Python loop snippet", insertText: "for ${1:item} in ${2:items}:\n\t${0:pass}" },
  ],
  c: [
    { label: "main", detail: "C main snippet", insertText: "int main(void) {\n\t${0}\n\treturn 0;\n}" },
    { label: "for", detail: "C loop snippet", insertText: "for (int ${1:i} = 0; ${1} < ${2:count}; ++${1}) {\n\t${0}\n}" },
    { label: "include", detail: "C include snippet", insertText: "#include <${1:stdio.h}>" },
  ],
  cpp: [
    { label: "main", detail: "C++ main snippet", insertText: "int main() {\n\t${0}\n\treturn 0;\n}" },
    { label: "class", detail: "C++ class snippet", insertText: "class ${1:Name} {\npublic:\n\t${1}() = default;\n\t${0}\n};" },
    { label: "for", detail: "C++ loop snippet", insertText: "for (const auto& ${1:item} : ${2:items}) {\n\t${0}\n}" },
  ],
  markdown: [
    { label: "heading", detail: "Markdown heading snippet", insertText: "## ${1:Heading}\n\n${0}" },
    { label: "link", detail: "Markdown link snippet", insertText: "[${1:text}](${2:path})" },
    { label: "code", detail: "Markdown code block snippet", insertText: "```${1:language}\n${0}\n```" },
  ],
  yaml: [
    { label: "mapping", detail: "YAML mapping snippet", insertText: "${1:key}: ${2:value}" },
    { label: "list", detail: "YAML list snippet", insertText: "${1:items}:\n  - ${2:first}\n  - ${0:second}" },
  ],
  sql: [
    { label: "select", detail: "SQL select snippet", insertText: "SELECT ${1:columns}\nFROM ${2:table_name}\nWHERE ${0:condition};" },
    { label: "create_table", detail: "SQL table snippet", insertText: "CREATE TABLE ${1:table_name} (\n\t${2:id} INTEGER PRIMARY KEY,\n\t${0}\n);" },
  ],
  shell: [
    { label: "if", detail: "Shell condition snippet", insertText: "if [ ${1:condition} ]; then\n\t${0}\nfi" },
    { label: "for", detail: "Shell loop snippet", insertText: "for ${1:item} in ${2:items}; do\n\t${0}\ndone" },
    { label: "function", detail: "Shell function snippet", insertText: "${1:name}() {\n\t${0}\n}" },
  ],
};

/** Global Monaco registrations are scoped by identity to this editor's current live model. */
export function registerBasicLanguageIntelligence(
  monaco: typeof Monaco, editor: Monaco.editor.IStandaloneCodeEditor,
): Monaco.IDisposable {
  let disposed = false;
  const registrations: Monaco.IDisposable[] = [];
  function dispose() {
    if (disposed) return;
    disposed = true;
    for (const registration of registrations.splice(0)) registration.dispose();
  }
  for (const [language, templates] of Object.entries(snippets)) {
    registrations.push(monaco.languages.registerCompletionItemProvider(language, {
      provideCompletionItems(model, position, _context, token) {
        const current = () => !disposed && !token.isCancellationRequested
          && editor.getModel() === model && !model.isDisposed() && model.getLanguageId() === language;
        if (!current()) return { suggestions: [] };
        const word = model.getWordUntilPosition(position);
        const range = {
          startLineNumber: position.lineNumber, endLineNumber: position.lineNumber,
          startColumn: word.startColumn, endColumn: word.endColumn,
        };
        const suggestions = templates.map(template => ({
          ...template, range, kind: monaco.languages.CompletionItemKind.Snippet,
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
        }));
        return { suggestions: current() ? suggestions : [] };
      },
    }));
  }
  registrations.push(editor.onDidDispose(dispose));
  return { dispose };
}
