import { registerHooks } from "node:module";

const modules = {
  settings: new URL("../../src/features/settings/settingsRequest.ts", import.meta.url).href,
  events: new URL("../../src/features/events/eventRequest.ts", import.meta.url).href,
  runtime: new URL("../../src/features/ebpf/runtimeRequest.ts", import.meta.url).href,
};
const parents = new Set(Object.values(modules));
const transport = new URL("../../src/transport/privateRequest.ts", import.meta.url).href;

/** Match only these three bundler imports; leave every other Node resolution unchanged. */
export async function privateTransportModule(name) {
  if (!Object.hasOwn(modules, name)) throw new Error("Unknown private transport fixture module");
  const resolver = registerHooks({ resolve(specifier, context, nextResolve) {
    if (parents.has(context.parentURL) && specifier === "../../transport/privateRequest") {
      return nextResolve(transport, context);
    }
    return nextResolve(specifier, context);
  } });
  try { return await import(modules[name]); }
  finally { resolver.deregister(); }
}
