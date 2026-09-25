-- user_memory 纯逻辑单测(lua,不依赖 librime)。
-- 运行: lua tests/lua/test_user_memory.lua (工作目录 = 仓库根)

package.path = "rime/lua/?.lua;rime/lua/?/init.lua;" .. package.path

local um = dofile("rime/lua/xhup_flow/user_memory.lua")

local passed, failed = 0, 0
local function check(name, actual, expected)
  if actual == expected then
    passed = passed + 1
  else
    failed = failed + 1
    print(string.format("FAIL %s: 期望 %s,实际 %s", name, tostring(expected), tostring(actual)))
  end
end

local SCHEMA = um.SCHEMA_LINE
local HEADER = um.HEADER_LINE

-- 1. TSV 往返
local counts = { ["时间"] = 3, ["我们"] = 1, ["旅游景点"] = 7 }
local tsv = um.render_tsv(counts)
check("schema 行", tsv:sub(1, #SCHEMA), SCHEMA)
check("header 行", tsv:gsub("^.-\n(.-)\n.*$", "%1"), HEADER)
local parsed = um.parse_tsv(tsv)
check("往返:时间", parsed["时间"], 3)
check("往返:我们", parsed["我们"], 1)
check("往返:旅游景点", parsed["旅游景点"], 7)
check("往返:词数", (function() local n=0 for _ in pairs(parsed) do n=n+1 end return n end)(), 3)

-- 2. 排序确定性:同一表两次渲染逐字节一致
check("渲染确定性", um.render_tsv(counts), um.render_tsv(counts))

-- 3. 空表渲染/解析
local empty_tsv = um.render_tsv({})
check("空表行数", select(2, empty_tsv:gsub("\n", "\n")), 2)
check("空表解析", um.parse_tsv(empty_tsv) ~= nil, true)

-- 4. 损坏输入 → nil(降级空状态)
check("垃圾文本", um.parse_tsv("garbage"), nil)
-- header 行本身恰好匹配数据行格式,作为「数据行」合法 —— 有 header 的
-- 完整结构必须成功(此用例原误标为缺 header)。
check("header 行作数据行合法", um.parse_tsv(SCHEMA .. "\n" .. HEADER .. "\n时间\t1\t0\n") ~= nil, true)
check("坏 schema 版本", um.parse_tsv("# xhup-user-model/v2 version=2\n" .. HEADER .. "\n时间\t1\t0\n"), nil)
check("坏数据行", um.parse_tsv(SCHEMA .. "\n" .. HEADER .. "\n时间\tx\t0\n"), nil)
check("只有 schema 行", um.parse_tsv(SCHEMA .. "\n"), nil)
check("非字符串", um.parse_tsv(nil), nil)
check("空字符串", um.parse_tsv(""), nil)
-- 缺 header(只有 schema 行 + 数据行)→ nil;schema+数据无列头是坏结构
check("缺 header", um.parse_tsv(SCHEMA .. "\n\n时间\t1\t0\n"), nil)

-- 5. 坏行宽松跳过 vs 结构拒绝:合法坏行(空行)跳过,其余坏行拒绝
local with_blank = SCHEMA .. "\n" .. HEADER .. "\n\n时间\t2\t5\n\n"
local p2 = um.parse_tsv(with_blank)
check("空行宽松跳过", p2 ~= nil and p2["时间"] == 2, true)

-- 6. 累计合并语义:同词多行计数相加
local multi = SCHEMA .. "\n" .. HEADER .. "\n时间\t2\t1\n时间\t3\t2\n"
check("同词计数相加", um.parse_tsv(multi)["时间"], 5)

-- 7. 原子写盘 + 读回(临时目录)
local sep = package.config:sub(1, 1)
local tmpdir = os.tmpname():gsub("%.[^%.]*$", "")
os.remove(tmpdir)
if sep == "\\" then
  os.execute('mkdir "' .. tmpdir .. '" 2>nul')
else
  os.execute('mkdir -p "' .. tmpdir .. '"')
end
local snap = tmpdir .. sep .. um.SNAPSHOT_FILENAME
check("首次原子写", um.atomic_write(snap, tsv) ~= nil, true)
check("写后读回", um.read_snapshot(snap)["时间"], 3)
check("覆盖写", um.atomic_write(snap, um.render_tsv({["景点"] = 2})) ~= nil, true)
check("覆盖后读回", um.read_snapshot(snap)["景点"], 2)
check("旧词不再存在", um.read_snapshot(snap)["时间"], nil)
-- 无临时文件残留
local f = io.open(snap .. ".tmp", "rb")
check("无临时文件残留", f == nil, true)
if f then f:close() end
-- 空路径拒绝
check("空路径拒绝", um.atomic_write("", "x"), nil)
-- 读不存在的快照 = 空表(非 nil:文件缺失是正常情况)
local missing = um.read_snapshot(tmpdir .. sep .. "nope.tsv")
check("缺失快照返回空表", type(missing) == "table" and next(missing) == nil, true)

-- 清理
os.remove(snap)
os.remove(tmpdir)

print(string.format("PASS user_memory 纯逻辑单测:%d 通过,%d 失败", passed, failed))
if failed > 0 then
  os.exit(1)
end
