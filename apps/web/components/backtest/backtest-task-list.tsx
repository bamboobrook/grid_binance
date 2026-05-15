"use client";

import { useState } from "react";
import { requestBacktestApi } from "@/components/backtest/request-client";
import { DataTable, type DataTableRow } from "@/components/ui/table";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type BacktestTask = {
  id: string;
  name: string;
  progress: string;
  stage: string;
  status: string;
  updatedAt: string;
};

type BacktestTaskListProps = {
  lang: UiLanguage;
  loading?: boolean;
  onRefresh?: () => void | Promise<void>;
  onSelectTask?: (taskId: string) => void;
  selectedTaskId?: string;
  tasks: BacktestTask[];
};

export function BacktestTaskList({
  lang,
  loading = false,
  onRefresh,
  onSelectTask,
  selectedTaskId,
  tasks,
}: BacktestTaskListProps) {
  const [feedback, setFeedback] = useState<string>("");
  const [pendingKey, setPendingKey] = useState<string>("");

  async function handleAction(id: string, action: string) {
    setPendingKey(`${id}:${action}`);
    setFeedback(pickText(lang, "正在提交任务操作…", "Submitting task action..."));

    const result = await requestBacktestApi(`/api/user/backtest/tasks/${id}/${action}`, {
      method: "POST",
    });

    setPendingKey("");
    if (result.ok) {
      setFeedback(pickText(lang, `任务 ${id} 已执行 ${action}。`, `Task ${id} completed ${action}.`));
      await onRefresh?.();
      return;
    }

    setFeedback(result.message);
  }

  const rows: DataTableRow[] = tasks.map((task) => ({
    id: task.id,
    name: (
      <button className="text-left" onClick={() => onSelectTask?.(task.id)} type="button">
        <p className="font-medium">{task.name}</p>
        <p className="text-xs text-muted-foreground">{task.stage}</p>
        {selectedTaskId === task.id ? (
          <p className="text-xs text-primary">{pickText(lang, "正在查看", "Viewing")}</p>
        ) : null}
      </button>
    ),
    status: <span className="rounded-full bg-secondary/50 px-2 py-1 text-xs font-medium">{task.status}</span>,
    progress: task.progress,
    updatedAt: task.updatedAt,
    actions: (
      <div className="flex flex-wrap gap-2">
        <MiniAction action="pause" busy={pendingKey === `${task.id}:pause`} id={task.id} label={pickText(lang, "暂停", "Pause")} onAction={handleAction} />
        <MiniAction action="resume" busy={pendingKey === `${task.id}:resume`} id={task.id} label={pickText(lang, "继续", "Resume")} onAction={handleAction} />
        <MiniAction action="cancel" busy={pendingKey === `${task.id}:cancel`} id={task.id} label={pickText(lang, "取消", "Cancel")} onAction={handleAction} />
      </div>
    ),
  }));

  return (
    <section className="space-y-3 rounded-2xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{pickText(lang, "任务列表", "Task list")}</h2>
          <p className="text-sm text-muted-foreground">
            {loading
              ? pickText(lang, "正在加载真实任务…", "Loading real tasks...")
              : pickText(lang, "查看排队、运行中和待复核任务。", "Track queued, running, and review-ready tasks.")}
          </p>
        </div>
        <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => void onRefresh?.()} type="button">
          {pickText(lang, "刷新", "Refresh")}
        </button>
      </div>

      <DataTable
        caption={pickText(lang, "Backtest Tasks", "Backtest Tasks")}
        columns={[
          { key: "name", label: pickText(lang, "任务", "Task") },
          { key: "status", label: pickText(lang, "状态", "Status") },
          { key: "progress", label: pickText(lang, "进度", "Progress"), align: "right" },
          { key: "updatedAt", label: pickText(lang, "更新时间", "Updated"), align: "right" },
          { key: "actions", label: pickText(lang, "操作", "Actions") },
        ]}
        emptyMessage={pickText(lang, "暂无回测任务，选择币种后开始自动搜索 Top 5", "No backtest tasks yet; select symbols to start automatic Top 5 search.")}
        rows={rows}
      />
      <p aria-live="polite" className="text-sm text-muted-foreground">{feedback}</p>
    </section>
  );
}

function MiniAction({
  action,
  busy,
  id,
  label,
  onAction,
}: {
  action: string;
  busy: boolean;
  id: string;
  label: string;
  onAction: (id: string, action: string) => void;
}) {
  return (
    <button
      className="rounded-full border border-border px-3 py-1 text-xs font-medium disabled:opacity-60"
      disabled={busy}
      onClick={() => onAction(id, action)}
      type="button"
    >
      {busy ? "..." : label}
    </button>
  );
}
