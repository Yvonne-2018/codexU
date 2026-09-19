# codexU v1.3.1

codexU v1.3.1 补齐独立 Windows x86_64 Tauri Dashboard，并收敛推理性能聚合、原生视觉采集与双平台发布验证边界。

## 主要更新

- 完成 Windows Dashboard V0 的 Overview、Tasks、AI Leadership、Usage、Inference、Projects、Skills 和 Settings 八个 Web surface。
- Windows app-server 子进程启动改用 `CREATE_NO_WINDOW`，首次打开或刷新读取额度时不弹出 CMD/命令行窗口，并保留 loopback 请求、读取错误处理和子进程清理。
- Windows reader 只读取本机 Codex transcript、SQLite 和 automation 元数据，保持本地优先；不上传 usage、线程、路径、日志或账户数据。
- 推理性能聚合支持本地统计时区和 DST 边界，使用有界流式读取、指纹缓存与共享刷新索引；缺失或非法事件不会被伪造成有效样本。
- 固化 Windows glass surface 与语义 token、Playwright 合同/fixture、native exact-HWND 采集边界和 shell lifecycle 检查。
- 发布流程补齐 Windows x86_64 MSI/NSIS 构建脚本与跨平台资产校验；公开安装包默认未代码签名，未执行 Apple notarization。

## 验证

- 全局内存风险门禁：PASS；完整风险清单已复核。
- PR #44 合并后的 Windows CI：macOS、Windows Rust format、Windows web 均通过。
- Windows MSI/NSIS 已在本机按 release 脚本构建并复核；macOS 双架构 DMG 与跨平台汇总由 release workflow 的 macOS/Ubuntu runner 生成并复核。

## 安装包

- 内部构建号：28。
- Apple Silicon：`codexU-1.3.1-mac-arm64.dmg`
- Intel：`codexU-1.3.1-mac-x86_64.dmg`
- Windows MSI：`codexU-1.3.1-windows-x86_64.msi`
- Windows NSIS：`codexU-1.3.1-windows-x86_64-setup.exe`

## SHA-256

```text
SHA256_PLACEHOLDER  codexU-1.3.1-mac-arm64.dmg
SHA256_PLACEHOLDER  codexU-1.3.1-mac-x86_64.dmg
d511898277bdc20d77d321ce34d3c381cf7336b0440077c08da68eea2a79ea7d  codexU-1.3.1-windows-x86_64.msi
9f5d2673faa091247233c60e7cde247ca5fe3b484bc8721960b53c24ae731415  codexU-1.3.1-windows-x86_64-setup.exe
```

本版本的 macOS 签名、notarization 和 Windows 代码签名状态必须以实际构建结果为准；在正式发布前不得把未执行的签名或 notarization 描述为已完成。
