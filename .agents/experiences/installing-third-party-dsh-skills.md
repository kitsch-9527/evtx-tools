# 安装第三方 DSH Skills 经验总结

## 背景

安装来自 GitHub 仓库（如 [mattpocock/skills](https://github.com/mattpocock/skills)）的所有技能到 DeepSeek Harness (DSH) 环境。

## 核心发现：DSH 技能发现只支持一级深度

### 问题描述

DSH 的 `skill-filesystem` 插件**只支持一级深度的目录发现**，这是最关键的一点。

```
✅ 正确格式（一级深度）：
<root>/<skill-name>/SKILL.md

❌ 错误格式（嵌套深度，不会被发现）：
<root>/engineering/grill-with-docs/SKILL.md
<root>/productivity/tdd/SKILL.md
```

如果你从仓库克隆的技能保持了分类嵌套结构（如 `skills/engineering/grill-with-docs/SKILL.md`），DSH **不会**发现它们。

### 原因分析

`skill-filesystem` 插件的发现逻辑（参考源码 `dsh-skill-filesystem/lib/index.js`）：

```javascript
async roots(cwd) {
  const roots = [];
  // 项目根目录下的技能
  if (this.includeDefaultRoots && cwd !== void 0) {
    const projectRoot = await findProjectRoot(resolve(cwd), ...);
    roots.push({
      path: join(projectRoot, ".dsh/skills"),   // 只扫一级
      source: "project-dsh",
      rank: 100
    }, {
      path: join(projectRoot, ".agents/skills"),
      source: "project-agents",
      rank: 200
    });
  }
  // 自定义目录
  roots.push(...this.customSkillDirs.map(...));
  // 用户级目录
  if (this.includeDefaultRoots) roots.push({
    path: join(this.dshHome, "skills"),
    source: "user-dsh",
    rank: 400
  }, {
    path: join(this.agentsHome, "skills"),
    source: "user-agents",
    rank: 500
  });
  return roots;
}
```

## 正确的安装流程

### 步骤 1：克隆仓库

```bash
git clone https://github.com/mattpocock/skills.git /tmp/mattpocock-skills
```

### 步骤 2：识别技能结构

检查仓库中的技能目录结构：

```bash
# 列出所有技能目录（通常在 skills/ 或 src/skills/ 下）
find /path/to/repo -name "SKILL.md" -type f
```

注意技能的分类方式（如 `engineering/`、`productivity/` 等子目录）。

### 步骤 3：选择安装位置

DSH 会在以下位置扫描技能（按优先级排序）：

| 优先级 | 位置 | 说明 |
|--------|------|------|
| 100 | `<projectRoot>/.dsh/skills` | 项目级 DSH 技能 |
| 200 | `<projectRoot>/.agents/skills` | 项目级 .agents 技能 |
| 300 | `customSkillDirs` | 自定义目录（需配置） |
| 400 | `~/.dsh/skills` | 用户级 DSH 技能 |
| 500 | `~/.agents/skills` | 用户级 .agents 技能 |

**项目根目录（projectRoot）解析规则：**
- 如果当前目录或其上级有 `.git` 目录，则该目录为项目根
- 如果没有 `.git`，则使用当前工作目录（cwd）作为项目根

**推荐位置：**
- **全局安装**（所有项目可用）：`~/.dsh/skills` 或 `~/.agents/skills`
- **项目级安装**（仅当前项目可用）：`<projectRoot>/.dsh/skills` 或 `<projectRoot>/.agents/skills`

### 步骤 4：扁平化安装

**关键：必须将技能扁平化到一级深度**

假设要安装到 `~/.dsh/skills`：

```bash
# 假设仓库中技能位于 skills/engineering/<name>/SKILL.md 等
REPO_PATH="/tmp/mattpocock-skills/skills"
TARGET="$HOME/.dsh/skills"

# 找出所有包含 SKILL.md 的目录（不管其分类父目录）
find "$REPO_PATH" -name "SKILL.md" -type f | while read -r skill_md; do
  # 获取技能目录（SKILL.md 所在的目录）
  skill_dir=$(dirname "$skill_md")
  # 获取技能名称（目录名）
  skill_name=$(basename "$skill_dir")
  # 复制整个技能目录到目标根目录（一级深度）
  cp -r "$skill_dir" "$TARGET/$skill_name"
done

# 验证安装结果
find "$TARGET" -name "SKILL.md" -type f | wc -l
```

### 步骤 5：重启 DSH 会话

技能安装后，需要**重启 DSH 会话**（刷新页面或重新打开）才能让新技能被发现。

```bash
# 如果使用 web GUI，刷新浏览器页面
# 如果使用 CLI，退出并重新进入
```

## 技能类型说明

### 模型自动调用型（Model-invoked）

- 可以被模型根据任务内容自动调用
- 会出现在可用技能目录中
- SKILL.md 中**没有** `disable-model-invocation: true`

### 用户调用型（User-invoked）

- 只能通过用户输入斜杠命令调用（如 `/grill-me`、`/implement`）
- 不会出现在模型的技能目录中
- SKILL.md 中**有** `disable-model-invocation: true`

## 安装验证

### 验证技能是否被发现

```bash
# 检查安装位置的技能目录
ls -la ~/.dsh/skills/  # 应该看到 <skill-name>/ 目录（一级深度）

# 检查技能文件
find ~/.dsh/skills -name "SKILL.md" -type f | head -5
```

### 验证 DSH 是否加载了技能

```
# 在 DSH 会话中运行以下命令：
/skill
# 或
/find-skills
# 或查看系统提示中的 <available_skills> 列表
```

## 常见问题排查

### Q1: 安装后技能不显示

**原因 1：目录深度不是一级**
- 检查技能目录结构：`ls ~/.dsh/skills/`
- 确保没有分类子目录（如 `engineering/`、`productivity/`）

**原因 2：没有重启会话**
- DSH 在会话启动时扫描技能目录，需要重启才能发现新技能

**原因 3：安装位置错误**
- 确认 DSH 配置的 `skill-filesystem` 插件的 `includeDefaultRoots: true`
- 检查 `~/.dsh/skills` 和 `<projectRoot>/.dsh/skills` 是否都已扫描

**原因 4：SKILL.md 格式问题**
- 检查 SKILL.md 是否有正确的 YAML frontmatter
- 必须包含 `name:` 和 `description:` 字段
- name 应该是 kebab-case（如 `grill-me`、`tdd`）

### Q2: 模型看不到用户调用型技能

这是**设计如此**，不是错误。用户调用型技能通过斜杠命令调用，不会出现在模型目录中。

### Q3: 同名技能冲突

如果在多个位置安装了同名技能（如 `.dsh/skills` 和 `.agents/skills`），DSH 会按优先级（rank 数字）选择一个。

## 完整示例：安装 mattpocock/skills

```bash
# 1. 克隆仓库
git clone https://github.com/mattpocock/skills.git /tmp/mattpocock-skills

# 2. 创建目标目录
mkdir -p ~/.dsh/skills

# 3. 扁平化复制所有技能
REPO="/tmp/mattpocock-skills/skills"
for category_dir in "$REPO"/*/; do
  for skill_dir in "$category_dir"*/; do
    if [ -f "$skill_dir/SKILL.md" ]; then
      skill_name=$(basename "$skill_dir")
      cp -r "$skill_dir" ~/.dsh/skills/
    fi
  done
done

# 4. 验证
ls ~/.dsh/skills/  # 应看到 37 个技能目录
find ~/.dsh/skills -name "SKILL.md" | wc -l  # 应显示 37

# 5. 重启 DSH 会话
# 刷新浏览器或重启 CLI
```

## 附录：技能 frontmatter 格式示例

```yaml
---
name: grill-me
description: A relentless interview to sharpen a plan or design
disable-model-invocation: true  # 可选，标记为用户调用型
user-invocable: true  # 可选，明确允许用户调用
whenToUse: Use when you want to stress-test your thinking
metadata:
  version: "1.0.0"
  author: "Matt Pocock"
---
```

## 关键 takeaway

1. **DSH 只支持一级深度的技能目录发现** — 这是最重要的一点
2. **安装后必须重启会话** — 技能扫描在会话启动时进行
3. **用户调用型技能不出现在模型目录** — 设计如此，通过斜杠命令调用
4. **选择合适的安装位置** — 根据需要选择全局（~/.dsh/skills）或项目级

---

*文档版本：2026-08-29*
*适用 DSH 版本：1.x+*
*基于 mattpocock/skills 仓库的安装经验*
