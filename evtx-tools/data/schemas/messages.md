# messages.db 文件架构设计（v2）

## 总览

```
dump 输出目录结构:
  ./msg_dbs/
  ├── hotech-msg-win11-24h2-26100.db           ← 系统版本汇总（已有，不变）
  ├── hotech-msg-all.db             ← 多版本合并（merge 产）
  └── dlldb/                        ← 按 DLL 拆分的原子文件（新增）
        ├── adtschema.en-US.10.0.26220.1000.db
        ├── adtschema.zh-CN.10.0.26220.1000.db
        ├── authz.en-US.10.0.26220.1000.db
        ├── sysmon.en-US.15.2.0.0.db
        └── ...
```

两层文件服务于不同目的。

---

## Layer 1: 系统汇总文件（已有）

`hotech-msg-{product}-{displayver}-{build}.db`

| 用途 | 说明 |
|------|------|
| parse 自动匹配 | `--msg-dir` 按 build_number 查找 |
| 取证导出 | 一个文件包含完整消息，方便拷贝 |
| 快速上手 | 下载一个 .db 就能用 |

Schema 不变（单版本简洁版，无 build_number）。

---

## Layer 2: 按 DLL 拆分文件（新增）

`dlldb/{dll_name}.{lang}.{pe_version}.db`

### 文件命名规则

```
{dll_name}.{lang}.{pe_version}.db

dll_name    = DLL 基名（去路径、去扩展名，全小写）
            例: adtschema, authz, sysmon, vmswitch.sys

lang        = 语言代码（ISO 格式）
            例: en-US, zh-CN, zh-TW, de-DE

pe_version  = DLL 文件的 PE 版本号（4 段）
            例: 10.0.26220.1000, 15.2.0.0
```

**例：**

| 源文件 | 输出文件名 |
|--------|-----------|
| `C:\Windows\System32\adtschema.dll` | `adtschema.en-US.10.0.26220.1000.db` |
| `C:\Windows\System32\zh-CN\adtschema.dll.mui` | `adtschema.zh-CN.10.0.26220.1000.db` |
| `C:\Windows\System32\Authz.dll` | `authz.en-US.10.0.26220.1000.db` |
| `C:\ProgramData\Sysmon\sysmon.exe` | `sysmon.en-US.15.2.0.0.db` |

### 文件身份唯一性

全文件名是唯一标识：(dll_name × lang × pe_version) 三元组。

```
adtschema.zh-CN.10.0.26220.1000.db
  ↑dll名    ↑语言   ↑版本号

同一 DLL 不同语言 → 不同文件
同一 DLL 不同版本 → 不同文件
同一语言不同 DLL → 不同文件
```

### Schema

```sql
-- 文件名确定身份，内部不存版本信息
CREATE TABLE messages (
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    level         INTEGER,
    task          TEXT,
    opcode        TEXT,
    keywords      TEXT,
    parameters    TEXT,
    PRIMARY KEY (event_id)
);
```

极简——不需要 provider 表（一个文件 = 一个 DLL = 一个 provider，通常如此）。
如果 DLL 包含多个 provider 的消息，加 `provider_name` 列：

```sql
CREATE TABLE messages (
    provider_name TEXT NOT NULL,        -- 当 DLL 包含多个 provider 时
    event_id      INTEGER NOT NULL,
    message       TEXT NOT NULL,
    ...
    PRIMARY KEY (provider_name, event_id)
);
```

### 文件大小

单个 DLL .db 通常在 10KB–200KB 之间（adtschema.dll 544 条消息约 180KB）。
适合 git 直接跟踪，不需要 LFS。

---

## 两种文件的生成关系

```
dump --out-dir ./msg_dbs/ --lang 2052,1033 --split-dlls

执行逻辑:
  1. 注册表扫描所有 provider → DLL 路径
  2. 对每个 DLL:
     a. pelite 解析 PE → 版本号 + 消息表
     b. 写入 dlldb/{dll}.{lang}.{ver}.db     ← 独立文件
     c. 同时写入内存中的 system 汇总
  3. 汇总完成后:
     d. 写入 hotech-msg-{product}-{displayver}-{build}.db          ← 系统汇总
  4. 如有 --with-welm:
     e. WELM 数据写入 hotech-msg-{product}-{displayver}-{build}.db  ← 补充英文消息

关系:
  hotech-msg-{product}-{displayver}-{build}.db = dlldb/*.db 中本机语言的 + WELM
  但 hotech-msg-{product}-{displayver}-{build}.db 是独立生成的，不依赖 dlldb 文件
```

---

## 全自动提交更新流程

### GitHub Actions 工作流

```yaml
# .github/workflows/build-msgdb.yml
name: Build Message Database

on:
  schedule:
    - cron: '0 2 * * 0'         # 每周日凌晨 2 点
  workflow_dispatch:              # 手动触发
  push:
    branches: [main]
    paths: ['data/dlldb/**']      # 社区提交 DLL 数据时触发

jobs:
  build:
    runs-on: windows-latest       # 需要 Windows runner
    steps:
      - uses: actions/checkout@v4
        with:
          lfs: false              # 文件小，不用 LFS

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Build hotech-evtx
        run: cargo build --release -p hotech-evtx

      - name: Dump messages
        run: |
          ./target/release/hotech-evtx dump ^
            --out-dir ./data/ ^
            --lang 1033,2052,1028 ^
            --split-dlls ^
            --with-welm

      - name: Check for new DLL files
        id: check
        run: |
          git add data/dlldb/
          git add data/hotech-msg-*.db
          $changed = git diff --staged --name-only
          if ($changed) {
            echo "changed=true" >> $env:GITHUB_OUTPUT
            echo "files=$changed" >> $env:GITHUB_OUTPUT
          } else {
            echo "changed=false" >> $env:GITHUB_OUTPUT
          }

      - name: Commit new DLL data
        if: steps.check.outputs.changed == 'true'
        run: |
          git config user.name "hotech-bot"
          git config user.email "bot@hotech.dev"
          git commit -m "chore(msgdb): update message DLL data"
          git push

      - name: Create Release
        if: steps.check.outputs.changed == 'true'
        uses: softprops/action-gh-release@v2
        with:
          tag_name: msgdb-${{ github.run_number }}
          files: |
            data/hotech-msg-*.db
            data/dlldb/*.db
          body: |
            ## Message Database Update
            
            Windows Build: ${{ env.OS_BUILD }}
            New/Updated DLLs: ${{ env.CHANGED_DLLS }}
```

### 社区贡献流程

```bash
# 贡献者（只有 Sysmon 新增了版本）
hotech-evtx dump --out-dir ./my_dlls/ --split-dlls --providers "Sysmon"

# 只提交新增/修改的 DLL 文件
git add data/dlldb/sysmon.en-US.15.3.0.0.db
git rm data/dlldb/sysmon.en-US.15.2.0.0.db   # 旧版本
git commit -m "feat(msgdb): add Sysmon 15.3.0.0 messages"
git push

# → CI 自动:
#   1. 检测到 dlldb/ 变更
#   2. 重新构建 hotech-msg-all.db
#   3. 发布新 Release
```

### 版本更新触发场景

| 触发 | 方式 | 自动？ |
|------|------|--------|
| Windows 更新 | 每周定时 CI | ✅ 自动 |
| 社区提交新 DLL 数据 | PR 合入 main | ✅ 自动 |
| 手动更新 | `workflow_dispatch` | ✅ 手动 |
| 新工具版本发布 | release CI | ✅ 自动 |

---

## 对 parse 命令的影响

parse 不受影响——继续使用 `--msg-dir` 匹配 `hotech-msg-{product}-{displayver}-{build}.db`。
DLL 级别文件仅为数据交换/社区贡献设计，不是 parse 的输入格式。

```
parse 输入:
  --msg-dir → hotech-msg-{product}-{displayver}-{build}.db（精确匹配）
           → hotech-msg-all.db（合并库 fallback）

dlldb/*.db 不直接用于 parse → 用于:
  - 社区提交
  - git 版本管理
  - 增量差异比较
  - 重建 hotech-msg-{product}-{displayver}-{build}.db
```

---

## 目录结构（开源仓库）

```
data/
├── dlldb/                        # 社区维护的原子消息文件（git 跟踪）
│   ├── adtschema.en-US.10.0.26220.1000.db
│   ├── adtschema.zh-CN.10.0.26220.1000.db
│   ├── authz.en-US.10.0.26220.1000.db
│   ├── kernelbase.en-US.10.0.26220.1000.db
│   └── ...
│
├── releases/                     # 预构建的系统汇总文件（Release 产物）
│   ├── hotech-msg-win11-24h2-26100.db
│   ├── hotech-msg-win10-22h2-19045.db
│   ├── hotech-msg-win10-1809-17763.db
│   └── hotech-msg-all.db
│
└── welm/                         # NSA WELM 种子数据（git 跟踪）
    └── welm_combined.db
```
