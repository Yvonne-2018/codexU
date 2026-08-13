# codexU Windows 移植方案（macOS → Windows）

基于 macOS 版（`Sources/CodexUsageWidget`）与 Windows 版（`windows/`）功能对比，整理未移植功能及移植方案。

## 本轮移植范围（并行工作流）

| 工作流 | 功能 | 归属文件 |
|---|---|---|
| A | 推理时长（inference performance） | codexu-core：新增 `readers/model_inference.rs`、`codex_transcript.rs`、`models/usage.rs` |
| B | Codex 实时任务推送（live 列） | codexu-core：新增 `readers/live_task.rs`、`codex_app_server.rs` |
| C | Claude skill 用量聚合 + 模型级趋势 | codexu-core：`claude_transcript.rs`、`common.rs` |
| D | Web 前端面板（挂载死代码/推理/模型趋势/设置增强） | codexu-tauri `web/src/*` |
| E | Tauri 集成（快捷键/单实例/自启/托盘额度/更新检查/CLI导出/诊断） | codexu-tauri `src-tauri/*`、codexu-cli |

## 数据契约

### A. 推理时长（写入 `LocalUsage.inference_performance`）
```json
"inference_performance": {
  "refreshed_at": <epoch_ms>,
  "models": [
    {
      "model": "gpt-5.4",
      "effort": "high",
      "call_count": 12,
      "total_duration_ms": 36000,
      "average_duration_ms": 3000.0,
      "p50_duration_ms": 2800.0,
      "p90_duration_ms": 4800.0,
      "total_output_tokens": 9000,
      "average_tokens_per_second": 0.25,
      "reasoning_output_ratio": 0.35
    }
  ]
}
```
数据源：Codex transcript 的 `task_complete` 事件（`duration_ms`）+ `turn_context`（model/effort）+ token_count deltas（output/reasoning）。

### C. 模型级趋势（填充现有 `UsageTrend.model_trends`）
```json
"model_trends": [
  {
    "id": "gpt-5.4",
    "model": "gpt-5.4",
    "day_buckets": [ { "id":"2026-08-13", "date":<ms>, "usage": {...}, "source_quality":"detailed" } ],
    "summary": { "seven_day": {...}, "daily_average_tokens": 0, "peak_day": null, "change_percent": null, "is_new_activity": false },
    "active_day_count": 3
  }
]
```

### E 提供的 Tauri 命令（D 前端调用）
- `run_diagnostics() -> DiagnosticsReport`
- `check_for_updates() -> UpdateInfo`
- `set_autostart(enabled: bool) -> ()`、`get_autostart() -> bool`
- 托盘菜单动态额度展示：刷新后重建托盘菜单显示 Codex 5h/7d/mo 用量百分比

```json
// DiagnosticsReport
{ "codex_root": "...", "codex_root_exists": true, "state_db_exists": true,
  "claude_projects_exists": true, "claude_tasks_exists": true,
  "codex_executable": "C:/.../codex.exe", "codex_quota_read_succeeded": true,
  "messages": [] }
// UpdateInfo
{ "current_version": "0.1.0", "latest_version": "0.2.0" | null,
  "release_url": "..." | null, "checked_at": <ms> | null, "error": "..." | null }
```

## 延期项（记录原因）
- **Claude status-line 额度**：数据源 `statusline-snapshot.json` 由外部机制写入，Windows 上无对应来源（Windows 无官方 Claude 额度路径）。
- **codex:// 深链**：需确认 Windows 上 Codex CLI 是否注册该协议；待验证后单独立项。
- **统计时区偏好**：影响聚合口径，改动面大，本轮回合后单独做。
- **无边框玻璃窗/托盘富图标渲染**：macOS 视觉概念，Tauri 托盘图标渲染能力有限，做"托盘菜单额度摘要"作为等价替代。
