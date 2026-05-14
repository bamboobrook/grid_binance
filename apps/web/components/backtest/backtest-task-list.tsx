"use client";

import { useMemo, useState } from "react";
import { requestBacktestApi } from "@/components/backtest/request-client";
import { DataTable, type DataTableRow } from "@/components/ui/table";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type TaskFilter = "active" | "completed" | "failed" | "archived" | "all";

type BacktestTask = {
  id: string;
  name: string;
  progress: string;
  rawStatus?: string;
  stage: string;
  status: string;
  updatedAt: string;
  archived?: boolean;
};

type BacktestTaskListProps = {
  lang: UiLanguage;
  loading?: boolean;
  onRefresh?: () => void | Promise<void>;
  onSelectTask?: (taskId: string) => void;
  selectedTaskId?: string;
  tasks: BacktestTask[];
};

export function BacktestTaskList({ lang, loading = false, onRefresh, onSelectTask, selectedTaskId, tasks }: BacktestTaskListProps) {
  const [feedback, setFeedback] = useState<string>("");
  const [pendingKey, setPendingKey] = useState<string>("");
  const [filter, setFilter] = useState<TaskFilter>("active");

  async function handlePostAction(id: string, action: string) {
    setPendingKey(`${id}:${action}`);
    setFeedback(pickText(lang, "正在提交任务操作…", "Submitting task action..."));
    const result = await requestBacktestApi(`/api/user/backtest/tasks/${id}/${action}`, { method: "POST" });
    setPendingKey("");
    if (result.ok) {
      setFeedback(pickText(lang, `任务 ${id} 已执行 ${action}。`, `Task ${id} completed ${action}.`));
      await onRefresh?.();
      return;
    }
    setFeedback(result.message);
  }

  async function handleDelete(id: string, rawStatus?: string) {
    if (isActiveStatus(rawStatus)) {
      setFeedback(pickText(lang, "请先取消运行中/排队/暂停的任务，再删除。", "Cancel queued/running/paused tasks before deleting."));
      return;
    }
    const confirmed = window.confirm(pickText(lang, "确认删除这个回测任务？候选结果、图表索引和事件记录都会被清理。", "Delete this backtest task? Candidates, chart indexes, and task events will be removed."));
    if (!confirmed) return;
    setPendingKey(`${id}:delete`);
    const result = await requestBacktestApi(`/api/user/backtest/tasks/${id}`, { method: "DELETE" });
    setPendingKey("");
    if (result.ok) {
      setFeedback(pickText(lang, "回测任务已删除。", "Backtest task deleted."));
      await onRefresh?.();
      return;
    }
    setFeedback(result.message);
  }

  const visibleTasks = useMemo(() => tasks.filter((task) => taskMatchesFilter(task, filter)), [filter, tasks]);
  const rows: DataTableRow[] = visibleTasks.map((task) => ({
    id: task.id,
    name: (
      <button className="text-left" onClick={() => onSelectTask?.(task.id)} type="button">
        <p className="font-medium">{task.name}</p>
        <p className="text-xs text-muted-foreground">{task.stage}</p>
        {task.archived ? <p className="text-xs text-amber-600">{pickText(lang, "已归档", "Archived")}</p> : null}
        {selectedTaskId === task.id ? <p className="text-xs text-primary">{pickText(lang, "正在查看", "Viewing")}</p> : null}
      </button>
    ),
    status: <span className="rounded-full bg-secondary/50 px-2 py-1 text-xs font-medium">{task.status}</span>,
    progress: task.progress,
    updatedAt: task.updatedAt,
    actions: (
      <div className="flex flex-wrap gap-2">
        <MiniAction action="pause" busy={pendingKey === `${task.id}:pause`} id={task.id} label={pickText(lang, "暂停", "Pause")} onAction={handlePostAction} />
        <MiniAction action="resume" busy={pendingKey === `${task.id}:resume`} id={task.id} label={pickText(lang, "继续", "Resume")} onAction={handlePostAction} />
        <MiniAction action="cancel" busy={pendingKey === `${task.id}:cancel`} id={task.id} label={pickText(lang, "取消", "Cancel")} onAction={handlePostAction} />
        <MiniAction action="archive" busy={pendingKey === `${task.id}:archive`} id={task.id} label={pickText(lang, "归档", "Archive")} onAction={handlePostAction} />
        <button className="rounded-full border border-red-500/40 px-3 py-1 text-xs font-medium text-red-600 disabled:opacity-60" disabled={pendingKey === `${task.id}:delete`} onClick={() => void handleDelete(task.id, task.rawStatus)} type="button">
          {pendingKey === `${task.id}:delete` ? "..." : pickText(lang, "删除", "Delete")}
        </button>
      </div>
    ),
  }));

  return (
    <section className="space-y-3 rounded-2xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{pickText(lang, "任务列表", "Task list")}</h2>
          <p className="text-sm text-muted-foreground">{loading ? pickText(lang, "正在加载真实任务…", "Loading real tasks...") : pickText(lang, "管理活跃、完成、失败和已归档回测任务。", "Manage active, completed, failed, and archived backtest tasks.")}</p>
        </div>
        <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => void onRefresh?.()} type="button">{pickText(lang, "刷新", "Refresh")}</button>
      </div>
      <div className="flex flex-wrap gap-2">
        {filterOptions(lang).map((option) => (
          <button className={`rounded-full border px-3 py-1 text-xs font-medium ${filter === option.value ? "border-primary bg-primary/10 text-primary" : "border-border"}`} key={option.value} onClick={() => setFilter(option.value)} type="button">{option.label}</button>
        ))}
      </div>
      <DataTable
        caption={pickText(lang, "Backtest Tasks", "Backtest Tasks")}
        columns={[{ key: "name", label: pickText(lang, "任务", "Task") }, { key: "status", label: pickText(lang, "状态", "Status") }, { key: "progress", label: pickText(lang, "进度", "Progress"), align: "right" }, { key: "updatedAt", label: pickText(lang, "更新时间", "Updated"), align: "right" }, { key: "actions", label: pickText(lang, "操作", "Actions") }]}
        emptyMessage={pickText(lang, "当前筛选下没有回测任务。", "No backtest tasks match the current filter.")}
        rows={rows}
      />
      <p aria-live="polite" className="text-sm text-muted-foreground">{feedback}</p>
    </section>
  );
}

function filterOptions(lang: UiLanguage): { label: string; value: TaskFilter }[] {
  return [
    { value: "active", label: pickText(lang, "活跃", "Active") },
    { value: "completed", label: pickText(lang, "已完成", "Completed") },
    { value: "failed", label: pickText(lang, "失败/取消", "Failed/Cancelled") },
    { value: "archived", label: pickText(lang, "已归档", "Archived") },
    { value: "all", label: pickText(lang, "全部", "All") },
  ];
}

function taskMatchesFilter(task: BacktestTask, filter: TaskFilter) {
  if (filter === "all") return true;
  if (filter === "archived") return Boolean(task.archived);
  if (task.archived) return false;
  if (filter === "active") return isActiveStatus(task.rawStatus);
  if (filter === "completed") return ["succeeded", "completed"].includes(task.rawStatus ?? "");
  if (filter === "failed") return ["failed", "cancelled"].includes(task.rawStatus ?? "");
  return true;
}

function isActiveStatus(status?: string) {
  return ["queued", "running", "paused"].includes(status ?? "");
}

function MiniAction({ action, busy, id, label, onAction }: { action: string; busy: boolean; id: string; label: string; onAction: (id: string, action: string) => void }) {
  return <button className="rounded-full border border-border px-3 py-1 text-xs font-medium disabled:opacity-60" disabled={busy} onClick={() => onAction(id, action)} type="button">{busy ? "..." : label}</button>;
}
