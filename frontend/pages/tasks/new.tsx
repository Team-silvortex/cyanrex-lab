import dynamic from "next/dynamic";
import SidebarLayout from "../../src/components/SidebarLayout";
import { useI18n } from "../../src/i18n/context";

const TaskDraftWorkspace = dynamic(() => import("../../src/features/tasks/TaskDraftWorkspace"), { ssr: false });

export default function NewTaskPage() {
  const { t } = useI18n();
  return <SidebarLayout title={t("taskDraft.title")}><TaskDraftWorkspace /></SidebarLayout>;
}
