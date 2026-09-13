const ts = require("typescript");

module.exports = function transpile(source) {
  return ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
    fileName: this.resourcePath,
  }).outputText;
};
