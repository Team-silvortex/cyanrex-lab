import type { ChangeEvent } from "react";
import { useConfirmedAction } from "../../components/useConfirmedAction";
import { getEngineUrl } from "../../config/runtime";
import type { useEbpfPageController } from "./useEbpfPageController";
import { MAX_UPLOAD_BYTES, type EbpfCompilerTarget, type UserScript } from "./models";

type Controller = ReturnType<typeof useEbpfPageController>;
type Translate = (key: string, vars?: Record<string, string | number>) => string;

export function useEbpfSafetyActions(controller: Controller, t: Translate, labId: string) {
  const safety = useConfirmedAction();
  const replace = (target: string, next: string, apply: () => void) => {
    if (safety.isBusy() || controller.running) return;
    if (next === controller.code) { apply(); return; }
    safety.request({ action: t("ebpf.load"), description: t("safety.replaceSource"), targets: [target], preview: next }, apply);
  };
  const selectTemplate = (id: string) => {
    const template = controller.templates.find(item => item.id === id);
    if (!template) { if (!safety.isBusy()) controller.setSelectedTemplate(id); return; }
    replace(`${template.name} (${id})`, template.code, () => {
      controller.setSelectedTemplate(id);
      controller.setCode(template.code);
    });
  };
  const lab = controller.activeLabProgress?.lab;
  const labTemplate = labId && lab?.id === labId
    ? controller.templates.find(item => item.id === lab.template_id) : undefined;
  const loadLabTemplate = () => {
    if (!labTemplate) return;
    replace(`${lab?.title} · ${labTemplate.name} (${labTemplate.id})`, labTemplate.code, () => {
      controller.setSelectedTemplate(labTemplate.id);
      controller.setCode(labTemplate.code);
      controller.setScriptTitle(`lab-${labId}`);
    });
  };
  const loadScript = (script: UserScript) => replace(`${script.title} (${script.id})`, script.script, () => {
    controller.setScriptTitle(script.title);
    controller.setCode(script.script);
  });
  const upload = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = ""; // Re-selecting the same file must work after cancelling.
    if (!file || safety.isBusy() || controller.running) return;
    if (file.size > MAX_UPLOAD_BYTES) { void controller.onUpload(file); return; }
    safety.request({ action: t("ebpf.importFile"), description: t("safety.replaceSource"), targets: [file.name] },
      signal => controller.onUpload(file, signal));
  };
  const run = () => {
    if (controller.running) return;
    safety.request({ action: t("ebpf.compileRun"), description: t("safety.run"), targets: [controller.scriptTitle],
      preview: controller.code, dangerous: false, details: [
        { label: "Engine", value: new URL(getEngineUrl()).origin },
        { label: t("ebpf.runtimeBackend"), value: controller.runtimeBackend },
        { label: t("ebpf.templateLabel"), value: controller.selectedTemplate || "—" },
        { label: t("learn.activeLab"), value: labId || "—" },
        { label: t("ebpf.debugBreakpoints"), value: controller.debugBreakpoints.join(", ") || "—" },
        { label: t("ebpf.kernelStream"), value: `${controller.enableKernelStream} · ${controller.samplingPerSec}/s · ${controller.streamSeconds}s` },
      ] }, controller.runEbpf);
  };
  const selectCompilerTarget = (next: EbpfCompilerTarget) => {
    if (safety.isBusy() || next === controller.compileBackends.target) return;
    if (next === "local") { controller.compileBackends.setTarget(next); return; }
    safety.request({ action: t("ebpf.compilerBackend"), description: t("safety.compilerTarget"),
      targets: [next.slice(6)] }, () => controller.compileBackends.setTarget(next));
  };
  const detachOne = (path: string) => {
    if (!path?.trim() || controller.running) return;
    safety.request({ action: t("ebpf.detach"), description: t("safety.detach"), targets: [path] }, () => controller.detach(path));
  };
  const detachAll = () => {
    if (controller.running || !controller.attachments.length) return;
    safety.request({ action: t("ebpf.detachAll"), description: t("safety.detachAll"),
      targets: [...controller.attachments], phrase: "DETACH" }, () => controller.detach(null));
  };
  const deleteScript = (script: UserScript) => safety.request({ action: t("ebpf.delete"), description: t("safety.deleteScript"),
    targets: [`${script.title} (${script.id})`] }, () => controller.deleteScript(script.id));
  return { ...safety, selectTemplate, loadLabTemplate, labTemplateAvailable: Boolean(labTemplate),
    selectCompilerTarget, loadScript, upload, run, detachOne, detachAll, deleteScript };
}
