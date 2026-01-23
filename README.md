# Cloudreve Sync (Unofficial)

Cloudreve 非官方双向同步客户端（Tauri + Vue 3 + Element Plus）。

## 基本功能

- 双向数据同步
- 一键分享
- 支持多文件夹同步
- 支持多账号

## 开发

1. 安装依赖
   - `npm install`
2. 启动前端（仅前端预览）
   - `npm run dev`
3. 启动桌面应用
   - `npm run tauri dev`

## 构建

- 构建前端资源：`npm run build`
- 打包桌面应用：`npm run tauri build`

## 配置与数据位置

- Linux: `~/.config/cn.mikesolar.cloudreve-sync`
- Windows: `%APPDATA%\\cn.mikesolar.cloudreve-sync`

## 许可证

GPLv3
