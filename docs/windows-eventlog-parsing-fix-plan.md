# Windows 事件查看器解析流程修复计划

## 问题总结

通过对比 Windows 事件查看器和 evtx-tools 的解析结果，发现以下问题：

### 1. 值映射问题 (Value Mapping)
**影响事件**: EventID 566 (Microsoft-Windows-Kernel-Power)
**问题描述**: 数值参数未映射为可读字符串
- Event Viewer: "原因 InputHid" / "原因 SessionUnlock" / "原因 InputAccelerometer"
- evtx-tools: "原因 32" / "原因 12" / "原因 31"

**根本原因**: value_maps 表为空，CRIM VMAP/BMAP 数据未加载

### 2. 消息模板错误 (Wrong Template)
**影响事件**: 
- EventID 16 (Microsoft-Windows-Kernel-General)
- EventID 6013 (EventLog)

**问题描述**: 使用了错误的消息模板
- EventID 16: Event Viewer 显示 "已经清除配置单元"，evtx-tools 显示 "程序包运行时信息"
- EventID 6013: Event Viewer 显示 "系统启动时间为 354558 秒"，evtx-tools 显示 "系统上一次启动是在 。"

**根本原因**: 消息数据库中缺少正确的模板，或模板匹配逻辑有问题

### 3. 参数截断问题 (Parameter Truncation)
**影响事件**: EventID 8233, 8230 (Microsoft-Windows-Security-SPP)
**问题描述**: 消息参数未完整替换
- Event Viewer: 完整消息包含多个参数
- evtx-tools: 只显示模板文本，参数缺失

**根本原因**: 参数提取或替换逻辑有问题

---

## 修复计划

### 阶段 1: 修复值映射 (Value Mapping)

**目标**: 实现 CRIM VMAP/BMAP 值映射加载和应用

**步骤**:
1. 检查 msgdb-dump 工具是否正确导出 value_maps 数据
2. 如果 value_maps 表为空，需要更新 msgdb-dump 工具
3. 在 evtx-tools 中确保 value_maps 正确加载和应用

**代码修改**:
- resolver.rs: 确保 load_value_maps() 正确工作
- substitute.rs: 在消息替换后应用值映射

### 阶段 2: 修复消息模板 (Message Template)

**目标**: 确保使用正确的消息模板

**步骤**:
1. 检查消息数据库中的模板是否正确
2. 如果模板缺失，需要更新 msgdb-dump 工具
3. 验证模板匹配逻辑

**代码修改**:
- resolver.rs: 检查 warmup_lang() 查询逻辑
- parser.rs: 检查 provider_name 提取逻辑

### 阶段 3: 修复参数替换 (Parameter Substitution)

**目标**: 确保所有参数正确替换

**步骤**:
1. 检查参数提取逻辑
2. 验证参数索引映射
3. 测试参数替换结果

**代码修改**:
- parser.rs: 检查 extract_fields() 逻辑
- substitute.rs: 检查 substitute() 逻辑

---

## 验证计划

### 测试用例

1. **EventID 566 (Kernel-Power)**
   - 验证值映射: 32 → InputHid, 12 → SessionUnlock, 31 → InputAccelerometer
   
2. **EventID 16 (Kernel-General)**
   - 验证消息模板: "已经清除配置单元"
   
3. **EventID 6013 (EventLog)**
   - 验证消息模板: "系统启动时间为 354558 秒"
   
4. **EventID 8233, 8230 (Security-SPP)**
   - 验证参数替换完整性

### 验证方法

1. 对比 Windows 事件查看器输出
2. 检查消息内容一致性
3. 验证参数替换正确性
4. 测试边界情况

---

## 预期结果

修复后，evtx-tools 应该能够：
1. 正确映射所有枚举值
2. 使用正确的消息模板
3. 完整替换所有参数
4. 与 Windows 事件查看器输出 100% 一致
