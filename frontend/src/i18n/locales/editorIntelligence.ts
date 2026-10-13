/** Display text only. Language IDs, completion labels and inserted source stay unchanged. */
export const editorIntelligenceMessages = {
  en: {
    plainText: "Plain text", ariaLabel: "Code editor",
    snippets: {
      rust: { fn: "Rust function snippet", let: "Rust binding snippet", match: "Rust match snippet" },
      python: { def: "Python function snippet", class: "Python class snippet", for: "Python loop snippet" },
      c: { main: "C main snippet", for: "C loop snippet", include: "C include snippet" },
      cpp: { main: "C++ main snippet", class: "C++ class snippet", for: "C++ loop snippet" },
      markdown: { heading: "Markdown heading snippet", link: "Markdown link snippet", code: "Markdown code block snippet" },
      yaml: { mapping: "YAML mapping snippet", list: "YAML list snippet" },
      sql: { select: "SQL select snippet", create_table: "SQL table snippet" },
      shell: { if: "Shell condition snippet", for: "Shell loop snippet", function: "Shell function snippet" },
    },
  },
  "zh-CN": {
    plainText: "纯文本", ariaLabel: "代码编辑器",
    snippets: {
      rust: { fn: "Rust 函数片段", let: "Rust 绑定片段", match: "Rust 模式匹配片段" },
      python: { def: "Python 函数片段", class: "Python 类片段", for: "Python 循环片段" },
      c: { main: "C 主函数片段", for: "C 循环片段", include: "C 头文件包含片段" },
      cpp: { main: "C++ 主函数片段", class: "C++ 类片段", for: "C++ 循环片段" },
      markdown: { heading: "Markdown 标题片段", link: "Markdown 链接片段", code: "Markdown 代码块片段" },
      yaml: { mapping: "YAML 映射片段", list: "YAML 列表片段" },
      sql: { select: "SQL 查询片段", create_table: "SQL 建表片段" },
      shell: { if: "Shell 条件片段", for: "Shell 循环片段", function: "Shell 函数片段" },
    },
  },
  es: {
    plainText: "Texto sin formato", ariaLabel: "Editor de código",
    snippets: {
      rust: { fn: "Fragmento de función de Rust", let: "Fragmento de enlace de Rust", match: "Fragmento de coincidencia de Rust" },
      python: { def: "Fragmento de función de Python", class: "Fragmento de clase de Python", for: "Fragmento de bucle de Python" },
      c: { main: "Fragmento de función principal de C", for: "Fragmento de bucle de C", include: "Fragmento de inclusión de C" },
      cpp: { main: "Fragmento de función principal de C++", class: "Fragmento de clase de C++", for: "Fragmento de bucle de C++" },
      markdown: { heading: "Fragmento de encabezado de Markdown", link: "Fragmento de enlace de Markdown", code: "Fragmento de bloque de código de Markdown" },
      yaml: { mapping: "Fragmento de mapa de YAML", list: "Fragmento de lista de YAML" },
      sql: { select: "Fragmento de consulta SQL", create_table: "Fragmento de tabla SQL" },
      shell: { if: "Fragmento de condición de Shell", for: "Fragmento de bucle de Shell", function: "Fragmento de función de Shell" },
    },
  },
  ja: {
    plainText: "プレーンテキスト", ariaLabel: "コードエディター",
    snippets: {
      rust: { fn: "Rust 関数スニペット", let: "Rust 変数束縛スニペット", match: "Rust パターンマッチスニペット" },
      python: { def: "Python 関数スニペット", class: "Python クラススニペット", for: "Python ループスニペット" },
      c: { main: "C メイン関数スニペット", for: "C ループスニペット", include: "C ヘッダー読み込みスニペット" },
      cpp: { main: "C++ メイン関数スニペット", class: "C++ クラススニペット", for: "C++ ループスニペット" },
      markdown: { heading: "Markdown 見出しスニペット", link: "Markdown リンクスニペット", code: "Markdown コードブロックスニペット" },
      yaml: { mapping: "YAML マッピングスニペット", list: "YAML リストスニペット" },
      sql: { select: "SQL 検索スニペット", create_table: "SQL テーブル作成スニペット" },
      shell: { if: "Shell 条件スニペット", for: "Shell ループスニペット", function: "Shell 関数スニペット" },
    },
  },
};
