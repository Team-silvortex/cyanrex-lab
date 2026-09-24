export const runnerSafetyMessages = {
  en: {
    runnerReadFailed: "Runner inventory could not be verified. Refresh before sending a probe or cancelling a job.",
    runnerStale: "Showing the last confirmed inventory for reference only. Actions require a verified refresh.",
    runnerActionUnconfirmed: "The Runner operation could not be confirmed and may already have taken effect. Refresh and check the job list before explicitly retrying.",
    runnerReviewChanged: "The reviewed target or inventory is no longer current. Refresh, inspect the target and open a new confirmation.",
    runnerProbeReview: "Queue one health probe for this Agent. This checks the control connection; it does not load an eBPF program. Review the Agent before submitting.",
  },
  zhCN: {
    runnerReadFailed: "无法核实 Runner 清单。请刷新后再发送探针或取消任务。",
    runnerStale: "当前显示上次确认的清单，仅供参考；操作前需成功刷新核对。",
    runnerActionUnconfirmed: "无法确认 Runner 操作结果，操作可能已生效。请先刷新并核对任务清单，再决定是否重新操作。",
    runnerReviewChanged: "已核对的目标或清单不再是最新状态。请刷新、检查目标并重新确认。",
    runnerProbeReview: "为此 Agent 排队一个健康探针，用于检查控制连接，不加载 eBPF 程序。请核对 Agent 后提交。",
  },
  es: {
    runnerReadFailed: "No se pudo verificar el inventario de Runner. Actualízalo antes de enviar una sonda o cancelar una tarea.",
    runnerStale: "Se muestra el último inventario confirmado solo como referencia. Las acciones requieren una actualización verificada.",
    runnerActionUnconfirmed: "No se pudo confirmar la operación de Runner y puede haberse aplicado. Actualiza y revisa las tareas antes de volver a intentarlo explícitamente.",
    runnerReviewChanged: "El objetivo o inventario revisado ya no está actualizado. Actualiza, revisa el objetivo y abre una nueva confirmación.",
    runnerProbeReview: "Poner en cola una sonda de estado para este Agent. Comprueba la conexión de control sin cargar un programa eBPF. Revisa el Agent antes de enviar.",
  },
  ja: {
    runnerReadFailed: "Runner の一覧を確認できませんでした。プローブ送信やジョブ取り消しの前に更新してください。",
    runnerStale: "最後に確認できた一覧を参考として表示しています。操作するには更新して確認する必要があります。",
    runnerActionUnconfirmed: "Runner 操作の結果を確認できません。すでに反映されている可能性があります。更新してジョブ一覧を確認してから、再実行するか判断してください。",
    runnerReviewChanged: "確認した対象または一覧が最新ではありません。更新して対象を調べ、改めて確認してください。",
    runnerProbeReview: "この Agent にヘルスプローブを一つ予約します。制御接続の確認のみで、eBPF プログラムはロードしません。送信前に Agent を確認してください。",
  },
};
