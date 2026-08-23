# v0.9.0 Roadmap

## 版本定位

v0.9.0 扩展 Skills Hub 的 AI Agent 工具兼容范围，新增 DeepSeek Harness 内置适配，并增加可配置的 Skill 发现扫描来源。用户可以继续使用统一的安装、同步、启停、导入和项目范围管理流程，同时控制哪些工具目录参与可导入 Skill 扫描。

本版本保持数据库结构和同步引擎不变，通过内置工具注册表接入 DeepSeek Harness，并在现有 settings 与 onboarding 流程上增加扫描配置、有效 Skill 校验和双入口管理界面。

## 版本目标

- 将 DeepSeek Harness 增加为第 47 个内置工具适配器。
- 支持同步到全局目录 `~/.dsh/skills`。
- 支持同步到项目目录 `<project>/.dsh/skills`。
- 通过默认配置目录 `~/.dsh` 检测 DeepSeek Harness。
- 在工具管理、安装和同步流程中统一展示 DeepSeek 品牌图标。
- 明确记录 DeepSeek Harness 的共享 `.agents/skills` 兼容目录、自定义 `DSH_HOME` 和 Skill 格式边界。
- 支持按实际工具 Skills 目录启用或停用发现扫描。
- 在设置页提供永久扫描管理入口，在发现横幅提供上下文快捷入口。
- 只把包含 `SKILL.md` 的一级子目录识别为可导入 Skill。
- 保持扫描设置与工具同步启用状态相互独立。

## 模块总览

| 模块 | 状态 | 说明 | 文档 |
| --- | --- | --- | --- |
| DeepSeek Harness 工具适配 | 已完成 | 支持全局与项目级 Skill 同步、安装检测、工具图标和双语名称 | [deepseek-harness-support.md](deepseek-harness-support.md) |
| Skill 发现扫描设置 | 已完成 | 按实际目录控制扫描来源，保存后重新扫描，并过滤无效目录 | [discovery-scan-settings.md](discovery-scan-settings.md) |
| 工具支持文档 | 已完成 | 更新中英文 README、内置工具数量和路径矩阵 | [deepseek-harness-support.md](deepseek-harness-support.md) |
| 版本元数据 | 已完成 | 前端、Rust 与 Tauri 版本统一为 `0.9.0` | 本文 |

## 验收标准

- 工具目录存在 `~/.dsh` 时，DeepSeek Harness 显示为已检测工具。
- 全局同步目标为 `~/.dsh/skills/<skill-name>`。
- 项目同步目标为 `<project>/.dsh/skills/<skill-name>`。
- 同步继续使用自动、软链接、Windows 目录联接和复制回退策略。
- 工具管理、安装选择和 Skill 同步状态使用一致的 DeepSeek Harness 名称与图标。
- 中英文 README 和 CHANGELOG 包含 DeepSeek Harness 支持信息。
- 设置页始终可以管理扫描来源；有发现结果时首页提供快捷入口。
- 共享同一 Skills 目录的工具只显示一个扫描来源开关。
- 关闭来源并保存后，发现数量立即刷新，已托管和已同步 Skill 不受影响。
- 重新启用来源后，对应有效 Skill 可以再次被发现。
- 不包含 `SKILL.md` 的普通子目录不会出现在发现结果中。
- `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 和 `src-tauri/tauri.conf.json` 的版本号保持一致。
- `npm run version:check` 和 `npm run check` 全部通过。

## 功能说明

- [deepseek-harness-support.md](deepseek-harness-support.md)：目录选择、适配行为、兼容范围和验证说明。
- [discovery-scan-settings.md](discovery-scan-settings.md)：配置模型、交互入口、扫描规则、IPC 和测试说明。

## 验证

实现完成后运行：

```bash
npm run version:check
npm run check
```

验证覆盖版本一致性、ESLint、前端测试、TypeScript/Vite 构建、Rust 格式、Clippy 和 Rust 测试。

2026-08-23 发布前验证结果：

- `npm run version:check` 通过，前端、Rust 和 Tauri 版本均为 `0.9.0`。
- `npm run check` 通过，前端 33 项测试、Rust 138 项测试全部通过。
- DeepSeek Harness 专项测试覆盖内置目录映射、项目范围能力和目录型 Skill 发现。
- 发现扫描专项测试覆盖来源开关、共享目录去重、Claude 插件来源、配置持久化和无关目录过滤。
- Tauri debug 无打包构建成功，桌面二进制与新增 IPC 完整链接。
- macOS Apple Silicon `.app` 与 `Skills Hub_0.9.0_aarch64.dmg` 本地构建成功，release 可执行文件可以正常启动。
- 自动更新产物签名需在 Release Workflow 中配置 `TAURI_SIGNING_PRIVATE_KEY`；macOS 正式签名继续使用现有 Apple 证书相关 Secrets。
