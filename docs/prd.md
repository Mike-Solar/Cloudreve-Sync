# Cloudreve 同步客户端 PRD  
**模式：双向同步 · 冲突双保留（云端保留原名） · 全量 SHA256 · 软删除**  
**协议：Cloudreve 官方 REST API（非 WebDAV）**

---

## 1. 产品概述

### 1.1 产品目标
开发一个 **开源、跨平台、可靠的 Cloudreve 同步客户端**，通过 Cloudreve 官方 API 实现：
- 双向文件同步
- 冲突自动“双保留”（不覆盖、不丢数据）
- 原始时间戳可恢复、可比较
- 稳定运行（不反复同步、不误删）

### 1.2 核心设计原则
- **以 Cloudreve API 为唯一后端**
- **不修改云端 `updated_at` 语义**
- **所有同步判定基于客户端写入的 metadata**
- **任何不可判定场景 → 双保留，绝不覆盖**

---

## 2. 同步语义定义（最重要）

### 2.1 文件身份
- 云端唯一标识：`file_id`
- 本地唯一标识：`<task_id> + local_relpath`
- 客户端维护 **映射表（本地 DB）**，避免仅靠路径判断

---

### 2.2 自定义同步 Metadata 规范（写入 Cloudreve）

| Key | 说明 |
|----|----|
| `sync:device_id` | 客户端设备唯一 ID |
| `sync:mtime_ms` | 原始本地修改时间（Unix ms） |
| `sync:sha256` | 文件内容 SHA256 |
| `sync:gen` | 同步代数（递增，可选） |
| `sync:deleted_at_ms` | 软删除时间（存在即视为删除） |
| `sync:conflict_of` | 冲突源文件 file_id |
| `sync:conflict_ts` | 冲突发生时间 |

> 所有值为字符串（Cloudreve metadata 约束）

---

## 3. 同步模式

### 3.1 支持模式
- ✅ 单向同步  
  - 本地 → 云端  
  - 云端 → 本地
- ✅ **双向同步（默认）**

---

## 4. 冲突处理策略（已锁定）

### 4.1 冲突定义
同一逻辑文件满足以下条件：
- 相对于 **上次同步基线**
  - 本地已变更
  - 云端已变更
- 且 `sha256` 不一致

即判定为 **真实冲突**

---

### 4.2 冲突处理规则（双保留）

**规则固定，不可配置：**

- **云端版本保留原文件名**
- 本地版本生成冲突副本并上传

#### 冲突副本命名规则
<name> (conflict-<device>-<YYYYMMDD-HHMMSS>).<ext>

示例：
report.docx
report (conflict-LAPTOP-20260117-154230).docx


#### 冲突副本 metadata
- `sync:conflict_of = <原 file_id>`
- `sync:device_id`
- `sync:mtime_ms`
- `sync:sha256`

---

## 5. 删除策略（软删除 · 已锁定）

### 5.1 删除定义
- 删除 ≠ 立即物理删除
- 删除 = **打删除标记 + 隐藏**

### 5.2 删除实现

#### 本地删除 → 云端
1. 客户端写入：
   - `sync:deleted_at_ms = <now>`
2. 文件在云端 UI 中：
   - 可移动至回收站或逻辑隐藏（由 Cloudreve 行为决定）
3. 客户端 DB 写 tombstone

#### 云端删除 → 本地
- 若云端 metadata 存在 `sync:deleted_at_ms`
- 本地执行删除（或移入本地回收区）

---

### 5.3 防止误删机制
- 无 tombstone 的“缺失文件” **不得立即判定为删除**
- 必须满足：
  - 该文件在上次同步中存在
  - 且明确观察到删除标记或删除事件

---

## 6. 变更检测算法

### 6.1 优先级顺序
1. 对比 `size`
2. 对比 `sync:mtime_ms`
3. **对比 `sync:sha256`（全量）**

### 6.2 Hash 策略
- 算法：SHA256
- 本地计算后：
  - 上传成功 → 写入云端 metadata
- 下载文件：
  - 校验 hash
  - 写入本地 DB

---

## 7. 同步流程（状态机）

```
扫描本地
↓
拉取云端列表（含 metadata）
↓
映射匹配（file_id / path / metadata）
↓
生成差异集
↓
┌───────────────┐
│ upload │
│ download │
│ conflict │
│ delete(mark) │
└───────────────┘
↓
执行队列（可并发）
↓
写回 metadata
↓
更新本地基线
```

---

## 8. 本地数据库设计（SQLite）

### 8.1 `tasks`
| 字段 | 说明 |
|----|----|
| task_id | 主键 |
| base_url | Cloudreve 站点 |
| local_root | 本地根目录 |
| remote_root_uri | 云端根 URI |
| device_id | 本机 ID |
| mode | bidirectional |
| settings_json | 高级配置 |

---

### 8.2 `entries`
| 字段 | 说明 |
|----|----|
| task_id | 外键 |
| local_relpath | 本地相对路径 |
| cloud_file_id | 云端 ID |
| cloud_uri | 云端路径 |
| last_local_mtime_ms | 上次同步 |
| last_local_sha256 | 上次同步 |
| last_remote_mtime_ms | 来自 metadata |
| last_remote_sha256 | 来自 metadata |
| last_sync_ts_ms | 上次成功同步 |
| state | ok / conflict / deleted |

---

### 8.3 `tombstones`
| 字段 | 说明 |
|----|----|
| task_id | 外键 |
| cloud_file_id | 文件 ID |
| local_relpath | 路径 |
| deleted_at_ms | 删除时间 |
| origin | local / remote |

---

### 8.4 `conflicts`
| 字段 | 说明 |
|----|----|
| task_id | 外键 |
| original_relpath | 原路径 |
| conflict_relpath | 冲突路径 |
| created_at_ms | 时间 |
| reason | both_modified |

---

## 9. 上传与下载

### 9.1 上传
- 小文件：直接上传
- 大文件：
  - Create upload session
  - 分片上传（并发）
  - Finish
- 成功后：
  - 写 metadata（hash / mtime / device）

### 9.2 下载
- 使用 Cloudreve 临时下载 URL
- 下载后校验 SHA256
- 写入本地 DB

---

## 10. 用户可配置项（有限）

| 配置项 | 默认 |
|----|----|
| 最大并发上传 | 4 |
| 最大并发下载 | 4 |
| 带宽限速 | 不限 |
| 冲突目录 | 同目录 |
| 日志级别 | info |

> 冲突策略 / hash 策略 / 删除策略 **不可配置**

---

## 11. 非功能性需求

### 11.1 稳定性
- 网络异常自动重试（指数退避）
- 客户端崩溃后可恢复上传

### 11.2 安全
- Token 加密存储
- HTTPS 强制校验
- 不记录明文凭据

---

## 12. MVP 范围（首个开源版本）

- ✅ 单站点
- ✅ 双向同步
- ✅ 冲突双保留
- ✅ 全量 SHA256
- ✅ 软删除
- ❌ 权限同步
- ❌ 在线编辑协作

---

## 13. 明确声明（必须在 README / UI 中展示）

> 本客户端不覆盖文件，不自动解决冲突  
> 冲突发生时，始终保留双方文件副本  
> 所有原始时间戳通过 metadata 保存与恢复

---