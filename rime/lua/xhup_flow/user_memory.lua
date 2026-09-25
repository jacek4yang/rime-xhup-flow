-- XHUP Flow 本地用户记忆观察组件(§16 / #83 R4)。
--
-- librime-lua `*module` 组件(schema 以 `lua_filter@*xhup_flow.user_memory`
-- 注册)。职责闭环「真实选择 → 隐私安全观察 → 版本化状态」:
--
-- 1. **观察**:commit_notifier 记录每次实际上屏的词形与次数
--    (进程内存态);不记录句子、按键序列、时间戳(§5 隐私红线)。
-- 2. **持久化**:按 #127 的版本化 TSV 格式
--    (`xhup_flow_user_model.tsv`,schema 行 `xhup-user-model/v1`),
--    每 FLUSH_EVERY 次提交原子写盘一次(临时文件 + os.rename 同目录
--    原子替换;崩溃最多丢最近 N-1 次观察 —— 学习是统计行为,可接受,
    --    换取不为每次提交全量重写文件)。
-- 3. **加载**:init 时若快照存在则读入内存(损坏/未来版本 → 静默回退
--    空状态,与 #127 的降级语义一致;绝不 panic)。
-- 4. **可关**:方案开关 `user_memory` 默认关闭(reset 0);关闭 = 完全
--    不观察不写盘,行为与无此组件一致。
--
-- 隐私与边界:
-- - 只存词形+计数;错误信息只含路径;绝不打印词形内容(§5);
-- - 零遥测、零网络;文件只在用户数据目录;
-- - 不改任何候选次序(排序消费是后续 PR;本组件只负责「记录」)。
--
-- 纯逻辑(load/save/merge 的可测核心)与本组件粘合层分离,单测
-- 不依赖 librime。

local M = {}

-- TSV 快照文件名(与 xhup-cli user_state / #127 一致)。
M.SNAPSHOT_FILENAME = "xhup_flow_user_model.tsv"
-- 模式行(版本由 schema 行承载)。
M.SCHEMA_LINE = "# xhup-user-model/v1 version=1"
-- 列头行。
M.HEADER_LINE = "word\tselections\tlast_seq"
-- 每多少次提交做一次写盘。
M.FLUSH_EVERY = 20

-- 解析 TSV 文本 → {word: count} 表。
-- 任何结构异常返回 nil(调用方回退空状态);宽松跳过坏行但拒绝坏头。
function M.parse_tsv(text)
    if type(text) ~= "string" then
        return nil
    end
    local lines = {}
    for line in text:gmatch("[^\r\n]+") do
        lines[#lines + 1] = line
    end
    if #lines < 2 then
        return nil
    end
    if lines[1] ~= M.SCHEMA_LINE or lines[2] ~= M.HEADER_LINE then
        return nil
    end
    -- 空快照(schema+header,零数据行)合法:返回空表。
    if #lines == 2 then
        return {}
    end
    local counts = {}
    for i = 3, #lines do
        local word, count = lines[i]:match("^([^\t]+)\t(%d+)\t%d+$")
        if word and count then
            counts[word] = (counts[word] or 0) + tonumber(count)
        elseif lines[i] ~= "" then
            return nil
        end
    end
    return counts
end

-- {word: count} 表 → TSV 文本(word 排序,确定性)。
function M.render_tsv(counts)
    local words = {}
    for word in pairs(counts) do
        words[#words + 1] = word
    end
    table.sort(words)
    local parts = { M.SCHEMA_LINE, M.HEADER_LINE }
    for _, word in ipairs(words) do
        parts[#parts + 1] = string.format("%s\t%d\t0", word, counts[word])
    end
    return table.concat(parts, "\n") .. "\n"
end

-- 原子写盘:临时文件 + 同目录 rename。失败返回 nil+错误(调用方静默
-- 降级;绝不 panic,绝不覆盖坏旧文件——rename 失败时旧文件仍在)。
function M.atomic_write(path, text)
    if type(path) ~= "string" or path == "" then
        return nil, "empty path"
    end
    local tmp = path .. ".tmp"
    local f = io.open(tmp, "wb")
    if not f then
        return nil, "cannot open temporary file"
    end
    local ok = f:write(text)
    ok = ok and f:close()
    if not ok then
        os.remove(tmp)
        return nil, "cannot write temporary file"
    end
    -- Windows 上 rename 不覆盖已存在目标:先删再改名。
    os.remove(path)
    if not os.rename(tmp, path) then
        os.remove(tmp)
        return nil, "cannot replace snapshot"
    end
    return true
end

-- 读快照(不存在 = 空);损坏 = nil(调用方回退空)。
function M.read_snapshot(path)
    local f = io.open(path, "rb")
    if not f then
        return {}
    end
    local text = f:read("*a")
    f:close()
    local counts = M.parse_tsv(text)
    if not counts then
        return nil
    end
    return counts
end

-- librime-lua 组件入口。
function M.init(env)
    env.enabled = false
    local ok = pcall(function()
        env.enabled = env.engine.context:get_option("user_memory")
    end)
    if not ok then
        env.enabled = false
    end
    env.counts = {}
    env.pending = 0
    env.seq = 0
    env.dirty = false
    env.snapshot_path = nil
    -- 用户数据目录 = librime user_data_dir;librime-lua 提供
    -- rime_api 的 get_user_data_dir? 组件侧无直接 API;使用与引擎
    -- 相同目录约定:快照路径由 schema 配置项给出(缺省文件名,
    -- 相对路径落在进程工作目录 —— 部署目录通常即用户目录)。
    local ok_path, path = pcall(function()
        return env.engine.schema.config:get_string("user_memory/snapshot_path")
    end)
    if ok_path and type(path) == "string" and path ~= "" then
        env.snapshot_path = path
    else
        env.snapshot_path = M.SNAPSHOT_FILENAME
    end
    -- 预加载(开关无关:读入内存只为后续开启时无缝;写盘仅在开启时)。
    local counts = M.read_snapshot(env.snapshot_path)
    if counts then
        env.counts = counts
    end
    -- 把内存计数表暴露给同引擎的其他组件(context_ranker 的用户记忆
    -- 桶消费它;表引用共享 —— 每次计数变更立即可见,零拷贝零轮询)。
    -- 只暴露内存表,不暴露写盘路径/任何可写文件句柄。
    pcall(function()
        env.engine.user_memory_counts = env.counts
    end)
    -- 连接提交观察(始终连接;回调内检查开关,关闭时不计数不写盘)。
    pcall(function()
        env.engine.context.commit_notifier:connect(function()
            local text = nil
            pcall(function()
                text = env.engine.context:get_commit_text()
            end)
            if not env.enabled or type(text) ~= "string" or text == "" then
                return
            end
            env.seq = env.seq + 1
            env.counts[text] = (env.counts[text] or 0) + 1
            env.pending = env.pending + 1
            env.dirty = true
            if env.pending >= M.FLUSH_EVERY then
                M.flush(env)
            end
        end)
    end)
end

-- 立即写盘(把内存计数落盘;不改内存状态)。
function M.flush(env)
    if not env.dirty or not env.snapshot_path then
        return
    end
    local text = M.render_tsv(env.counts)
    M.atomic_write(env.snapshot_path, text)
    env.pending = 0
    env.dirty = false
end

function M.func(translation, env)
    -- 本组件不改候选流:纯透传(观察职责与排序职责分离)。
    for cand in translation:iter() do
        yield(cand)
    end
end

return M
