# evtx-tools.exe 使用说明

> 二进制位置：`evtx-parity-test/tools/evtx-tools.exe`（Rust，离线解析，不依赖 Windows API）。
> 配套消息库：`evtx-parity-test/msgdb/hotech-msg-<系统>-<build>.db`（schema v9，由 msgdb-dump 产出）。
> 本文所有示例均在实机验证过（版本 0.1.0，2026-09-30 构建）。

---

## 1. 命令面总览

```
evtx-tools.exe <COMMAND>

Commands:
  parse   解析 .evtx 文件 → events.db / events.csv（核心命令）
  info    显示消息库元数据
  help    帮助
```

配套工具 `evtx-batch.exe`：目录批量形态的同源解析器，参数与 parse 基本一致。

---

## 2. parse —— 解析事件

```
evtx-tools.exe parse [OPTIONS] --output <OUTPUT> <INPUT>
```

| 参数 | 说明 |
|---|---|
| `<INPUT>` | .evtx 文件路径（必填） |
| `-o, --output` | 输出文件路径（必填；**所在目录必须已存在**，工具不自动建目录） |
| `--format` | `db`（默认，SQLite）/ `csv` / `both`。**注意**：`both` 时 db 与 csv 会抢写同一 `-o` 路径——需要两种格式时分两次跑（管线 05 即如此） |
| `--msg-db` | 显式指定消息库路径 |
| `--msg-dir` | 目录自动探测 `hotech-msg-*.db`（与 `--msg-db` 二选一；都没有则以原始形态输出消息字段） |
| `--lang` | 消息语言，默认 `zh-CN`。**请求语言缺失时自动回落 en-US**（镜像 wevtapi MUI 回退链；Setup 通道即此形态） |
| `--with-xml` | 附带 raw XML 列（默认关） |
| `--threads` | 工作线程数（0=全部核心，默认 0） |
| `--profile` | 内置分析 profile（security / persistence / system / remote / process / powershell / mssql / web，可多值） |
| `--rules-dir` | 自定义规则 YAML 目录 |
| `--no-stats` | 跳过统计表生成 |

### 最小示例（已验证）

```bash
MSGDB=E:\Code\evet\evtx-parity-test\msgdb\hotech-msg-Windows_10_Education-25H2-26220.9343.db
mkdir -p E:\Code\evet\output-demo   # 输出目录必须先建

# 中文渲染解析（管线 05 的形态）
evtx-tools.exe parse <evtx> -o <out>.csv --format csv --msg-db $MSGDB --lang zh-CN

# 英文渲染
evtx-tools.exe parse <evtx> -o <out-en>.csv --format csv --msg-db $MSGDB --lang en-US
```

成功输出：`[done] N events written to <路径>`。

### 输出列（CSV，v9 渲染）

`event_record_id, event_time, provider_name, provider_guid, event_id, level, severity, task_category, opcode, keywords, channel, computer, process_id, thread_id, user_name, user_sid, activity_id, related_activity_id, message, level_name, keywords_name, event_data, raw_xml, file_name, sequence_number`

`message` 已完成：模板代入（%1..%n）、`%%N` 参数引用展开（含 Win32 系统错误回退）、typed 参数渲染（hex/HRESULT/NTSTATUS 符号/FILETIME 纳秒）、值映射与位图展开、枚举本地化。

---

## 3. info —— 消息库元数据

```
evtx-tools.exe info <DB> [--provider <名称过滤>]
```

> R30（2026-09-30）起版本校验与 parse 对齐：只认 v9（旧库提示 `re-run msgdb-dump`）。
> 实例输出：Schema version 9 / Provider count 1170 / Variant count 1553249；
> `--provider Security-Auditing` 可按名称过滤并显示各 provider 变体数。

---

## 4. 典型工作流

### 4.1 双语解析（管线 05 形态，比对闭环用）

```bash
EVTX=baseline-evtx/System.evtx
for L in en-US zh-CN; do
  evtx-tools.exe parse $EVTX -o new-parsed/$(echo $L | cut -c1-2)/System.csv \
    --format csv --msg-db $MSGDB --lang $L
done
```

（管线实际用 `scripts/05-new-parse.ps1`：db 与 csv 分开跑、双通道全套、日志含语言/库路径。）

### 4.2 安全分析

```bash
evtx-tools.exe parse Security.evtx -o sec.db --format db \
  --msg-db $MSGDB --lang zh-CN --profile security --with-xml
```

`--profile` 按 YAML 规则给事件打场景标签并生成统计表；`--rules-dir` 可加自定义规则。

### 4.3 无消息库快速看事件

省略 `--msg-db/--msg-dir`：message 列为空/原始形态，但 EventID/Provider/时间/EventData 结构照常输出——适合只看事件骨架。

---

## 5. 已知行为与坑位

| 现象 | 说明 / 处置 |
|---|---|
| `info` 报 `schema_version=N, want 9` | 库与工具版本不配对（R30 起 info 与 parse 一致只认 v9） | 重跑 03 重建消息库 |
| CSV 内夹带重复表头行 | 每 batch 重写 BOM+表头的历史遗留；下游按 `event_id` 是否纯数字过滤（06 已内置） |
| `both` 格式抢写 | db/csv 共用 `-o` 路径互相覆盖；分两次跑 |
| 输出目录不存在报 os error 3 | 先 `mkdir` |
| 中文乱码（重定向/管道时） | 工具 stdout 走 UTF-8/系统码页混合；python 侧解码按实际来源选择 |
| 请求语言缺文本 | 自动回落 en-US（`--lang` 的既定语义，与 wevtutil 行为对齐；Setup 通道即整通道回落） |
| 解析报消息库 schema 错 | 库与渲染器版本不配对：`re-run msgdb-dump`（03）后重试 |

---

## 6. 与管线的关系

- 管线 `05-new-parse.ps1` 就是对 `parse` 的两次封装（en-US + zh-CN 各全套四通道）。
- 消息库由 `msgdb-dump-x64.exe`（03）从本机注册表/DLL 静态萃取；库内 `lang` 维度
  （provider_message_variants / parameter_refs / value_maps 均双语）支撑 `--lang` 的
  首选+回退语义。使用细节见 `docs/usage-guide.md`。
