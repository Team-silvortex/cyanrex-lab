import { useEffect, useRef, useState, type RefObject } from "react";
import type * as Monaco from "monaco-editor";
import { loader } from "../../utils/monacoLoader";
import { getEditorLanguage } from "./languages";
import { configureBuiltinLanguageServices, registerBasicLanguageIntelligence } from "./languageServices";

export type EditorSession = {
  editor: Monaco.editor.IStandaloneCodeEditor;
  monaco: typeof Monaco;
  synchronize: (language: string, source: string) => void;
};

/** One owned, disposable document. No Engine, filesystem, persistence or task dependency. */
export function useEditorSession(
  container: RefObject<HTMLDivElement | null>,
  initial: { language: string; text: string },
  onChange: (source: string) => void,
  onProblems: (markers: Monaco.editor.IMarker[]) => void,
) {
  const callbacks = useRef({ onChange, onProblems });
  const initialDocument = useRef(initial);
  callbacks.current = { onChange, onProblems };
  const [session, setSession] = useState<EditorSession | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    let cleanup: (() => void) | undefined;
    void loader.init().then((monaco: typeof Monaco) => {
      if (!active || !container.current) return;
      configureBuiltinLanguageServices(monaco);
      // getRandomValues also works on a self-hosted HTTP LAN; randomUUID requires a secure context.
      const workspace = Array.from(crypto.getRandomValues(new Uint32Array(4)),
        part => part.toString(16).padStart(8, "0")).join("");
      let sequence = 0;
      const create = (source: string, language: string) => {
        const profile = getEditorLanguage(language);
        if (!profile) throw new Error("Unknown editor language");
        return monaco.editor.createModel(source, profile.id,
          monaco.Uri.parse(`inmemory://cyanrex-editor/${workspace}/${sequence++}/document${profile.extension}`));
      };
      let model = create(initialDocument.current.text, initialDocument.current.language);
      let synchronizing = false;
      const editor = monaco.editor.create(container.current, {
        model, theme: "vs-dark", automaticLayout: true, minimap: { enabled: false },
        fontSize: 14, tabSize: 2, scrollBeyondLastLine: false, wordWrap: "on",
        wordBasedSuggestions: "currentDocument", formatOnPaste: false, formatOnType: false,
        // Monaco 0.55.1's occurrence highlighter leaks a rejected delayed promise on model disposal.
        // Keep explicit reference/definition actions; avoid its global cursor-highlighting state.
        occurrencesHighlight: "off",
        ariaLabel: "Code editor", renderValidationDecorations: "on",
      });
      const publishMarkers = () => {
        if (!active || editor.getModel() !== model || model.isDisposed()) return;
        callbacks.current.onProblems(monaco.editor.getModelMarkers({ resource: model.uri }).slice(0, 100));
      };
      const changed = editor.onDidChangeModelContent(() => {
        if (!active || synchronizing || editor.getModel() !== model) return;
        callbacks.current.onProblems([]);
        callbacks.current.onChange(model.getValue());
      });
      const markers = monaco.editor.onDidChangeMarkers(resources => {
        if (resources.some(uri => uri.toString() === model.uri.toString())) publishMarkers();
      });
      const intelligence = registerBasicLanguageIntelligence(monaco, editor);
      cleanup = () => {
        intelligence.dispose(); markers.dispose(); changed.dispose();
        editor.dispose(); model.dispose();
      };
      setSession({ editor, monaco, synchronize(language, source) {
        if (!active || editor.getModel() !== model) return;
        if (model.getLanguageId() === language && model.getValue() === source) return;
        // The parent owns content. Reconciliation is not another user edit or save acknowledgement.
        synchronizing = true;
        try {
          if (model.getLanguageId() === language) model.setValue(source);
          else {
            const previous = model;
            model = create(source, language);
            editor.setModel(model);
            previous.dispose();
          }
          callbacks.current.onProblems([]);
        } finally { synchronizing = false; }
      } });
    }).catch(() => { if (active) { cleanup?.(); setFailed(true); } });
    return () => { active = false; cleanup?.(); };
  }, [container]);
  return { session, failed };
}
