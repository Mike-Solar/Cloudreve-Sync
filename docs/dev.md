# Cloudreve Sync 开发指引

日期: 2025-02-14
执行者: Codex

## 前端

```bash
npm install
npm run dev
```

## Tauri 后端

```bash
cd src-tauri
cargo run
```

## 说明
- Tauri 开发模式依赖 `npm run dev` 提供前端页面。
- 打包前端产物位于 `dist/`，对应 `src-tauri/tauri.conf.json` 的 `distDir`。
