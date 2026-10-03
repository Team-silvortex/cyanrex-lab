import { loader } from "@monaco-editor/react";

// Configure before init, including direct visits to non-eBPF editor pages. Never use a CDN.
loader.config({ paths: { vs: "/monaco/vs" } });
export { loader };
