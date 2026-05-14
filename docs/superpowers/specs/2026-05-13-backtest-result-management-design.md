# Backtest Result Management Design

**Date:** 2026-05-13
**Owner:** 达拉崩吧
**Scope:** 马丁回测结果管理，不改变实盘发布逻辑。

## Problem

回测任务会持续累积，当前页面缺少清理和隐藏机制。用户点击回测后会看到越来越多历史任务，无法区分当前关注、已归档、失败或可删除的任务。候选结果也缺少明确的生命周期管理入口。

## Goals

- 用户可以在回测任务列表中归档任务，让默认页面保持干净。
- 用户可以删除不需要的回测任务，删除时同步清理候选、artifact、事件等派生数据。
- 删除运行中任务前必须先取消，避免 worker 正在写入时被硬删。
- 页面可以筛选全部、活跃、已完成、失败/取消、已归档任务。
- 删除/归档后自动刷新任务和候选区域，避免显示悬空结果。

## Non-Goals

- 不删除已发布的实盘 Portfolio；如果任务已被发布引用，删除任务应被后端拒绝，提示用户先保留或只归档。
- 不做批量删除旧任务的定时清理。
- 不删除本地 artifact 文件系统文件；本轮只删除数据库索引记录，避免误删共享卷中的非目标文件。

## Backend Design

- `BacktestRepository::archive_task(task_id)`：把 `backtest_tasks.summary.archived` 标记为 `true`，记录 `archived_at`，追加 `archived` 事件。
- `BacktestRepository::delete_task(owner, task_id)`：事务内验证 owner、状态非 `queued/running/paused`、没有 `martingale_portfolios.source_task_id` 引用；然后删除 artifacts、candidate summaries、task events、task 本体。
- `BacktestService::archive_task(owner, task_id)`：校验归属后调用 repo，返回更新后的任务。
- `BacktestService::delete_task(owner, task_id)`：校验归属后调用 repo，返回 `{ task_id, deleted: true }`。
- Routes:
  - `POST /backtest/tasks/{id}/archive`
  - `DELETE /backtest/tasks/{id}`

## Frontend Design

- Next proxy:
  - `POST /api/user/backtest/tasks/[id]/archive`
  - `DELETE /api/user/backtest/tasks/[id]`
- `BacktestTaskList` 新增筛选按钮：活跃、已完成、失败/取消、已归档、全部。
- 每个任务新增操作：归档、删除。
- 删除操作使用 `window.confirm` 二次确认；运行中/排队/暂停任务显示“先取消再删除”。
- 删除当前选中任务后清空候选和选中状态，刷新任务列表。

## Safety

- 删除必须 owner scoped。
- 运行中任务不能删除。
- 已发布 Portfolio 引用的任务不能删除，只能归档。
- 前端删除失败时显示后端错误信息。

## Testing

- shared-db 测试：归档写入 summary，删除级联候选/artifact/event，运行中拒绝删除，被 portfolio 引用拒绝删除。
- api-server 测试：owner scoped archive/delete。
- frontend contract 测试：代理 route 存在，任务列表包含筛选与归档/删除操作。
- TypeScript 编译通过。
