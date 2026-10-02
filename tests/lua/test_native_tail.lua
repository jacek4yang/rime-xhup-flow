package.path = "rime/lua/?.lua;" .. package.path
local policy = require("xhup_flow.native_tail")
local checks = 0
local function check(ok, why) assert(ok, why); checks = checks + 1 end
local function stream(items)
  return { iter = function() local i=0; return function() i=i+1; return items[i] end end }
end
Segment = function(start, finish) return { start=start, _end=finish } end
ShadowCandidate = function() error("a second shadow would break native learning") end
local function fixture(input, start)
  local env = { engine = { schema = {} } }
  local base, created, calls, emitted = {}, {}, {}, {}
  for i=1,100 do base[i]={ text=tostring(i), start=start, _end=start+#input, quality=10 } end
  local current, visits, lookups = "", 0, 0
  local exact = { ni=true, hc=true, hcn=true, hcnz=true, nihc=true, qu=true }
  local memory = {
    dict_lookup = function(_, code, predict, bound)
      check(predict==false and bound==1, "exact bounded lookup")
      current=code; lookups=lookups+1
      return false -- old bool API must not be treated as existence
    end,
    iter_dict = function()
      local i=0
      return function()
        if (exact[current] or env.dense) and i<20 then
          i=i+1; visits=visits+1
          return {text="native edge", weight=-4.2} -- weight is intentionally irrelevant
        end
      end
    end,
  }
  local native = { query = function(_, query, segment)
    calls[#calls+1] = query
    if #calls==1 then return stream(base) end
    local items={}
    for i=1,10 do
      local candidate={ text="native:"..query, start=segment.start,
        _end=segment._end-(env.partial and 1 or 0), type="sentence", quality=10, preedit=query, comment="" }
      items[i]=candidate; created[candidate]=true
    end
    return stream(items)
  end }
  Component = { Translator = function(engine, _, name)
    check(engine==env.engine and name=="table_translator@flow", "one native writer")
    return native
  end }
  Memory = function(engine, schema, name)
    check(engine==env.engine and schema==engine.schema and name=="flow_lookup", "read-only namespace")
    return memory
  end
  yield=function(candidate) emitted[#emitted+1]=candidate end
  policy.init(env)
  return env, base, created, calls, emitted, function() return visits,lookups end
end
local function run(input, start, configure)
  local env,base,created,calls,output,counts=fixture(input,start)
  if configure then configure(env) end
  policy.func(input,{start=start,_end=start+#input,tags={}},env)
  return env,base,created,calls,output,counts
end

local env,base,created,calls,output,counts=run("nihcnzqu",10)
check(#calls<=5 and calls[2]=="ni hcnz qu","structural path is native exact-boundary query")
check(output[1].type=="sentence" and output[1]._end==18,"native type and raw outer endpoint")
check(created[output[1]],"genuine native object preserved, never another shadow")
check(output[1].quality==10.5,"explicit structural utility, not lexical frequency")
local found={}
for _,c in ipairs(output) do found[c]=(found[c] or 0)+1 end
for i,c in ipairs(base) do
  check(found[c]==1,"every base object retained exactly once")
  if i>policy.HEAD then check(output[#output-#base+i]==c,"entire base tail unchanged") end
end
local visits,lookups=counts()
check(visits<=lookups and lookups<=#("nihcnzqu")*31,"single edge per exact lookup")

for _,input in ipairs({"","ni","ni'hc","ni2",string.rep("n",129)}) do
  local e,b,_,q,o,c=run(input,0)
  check(#q==1 and c()==0,"unsupported input bypasses planning")
  check(#o==#b and o[1]==b[1],"untouched native fallback")
end
local e,b,_,q,o=run("nihc",0)
check(o[1]==b[1],"standalone fixed codes not structurally promoted")
for _,c in ipairs(o) do if created[c] then check(c._end<4,"no full plan on 2/3/4 keys") end end
local dense=string.rep("n",128)
e,b,_,q,o,counts=run(dense,0,function(v) v.dense=true end)
visits,lookups=counts()
check(lookups<=128*31 and visits<=lookups,"worst-case lookup/iteration budget")
check(#q<=5 and #o<=#b+8,"native query and candidate budgets")

e,b,_,q,o=run("nihcnzqu",0,function(v) v.partial=true end)
check(#o==#b,"incomplete native query results never masquerade as full coverage")
e,b,_,q,o=run("nihcnzqu",0,function(v)
  v.memory.dict_lookup=function() error("injected lookup failure") end
end)
check(e.native_error=="boundary lookup failed" and #o==#b,"lookup failure preserves base")
e,b,_,q,o=run("nihcnzqu",0,function(v)
  v.native.query=function() error("injected provider failure") end
end)
check(e.native_error=="native query failed" and #o==0,"provider error contained")
Component=nil; policy.init(e)
check(not e.native and e.native_error=="native translator API unavailable","missing API explicit")
policy.fini(e); check(not e.native and not e.memory,"native resources released")
-- Budget enforcement happens inside the one native memorization callback.
local function learning_fixture(tick, limit)
  local native = { user_dict={tick=tick}, writes=0,
    memorize=function(self, entry) self.writes=self.writes+1; self.user_dict.tick=self.user_dict.tick+#entry:get(); return true end,
    disconnect=function(self) self.user_dict=nil end }
  local status
  local engine={ context={set_property=function(_,name,value) check(name=="xhup_flow_learning_status","status property"); status=value end},
    schema={config={get_int=function(_,name)
      if name=="flow/learning_max_updates" then return limit end
      if name=="flow/max_phrase_length" then return 20 end
      if name=="flow/max_homographs" then return 1 end
    end}}}
  Component={Translator=function() return native end}
  Memory=function() return {} end
  local e={engine=engine};policy.init(e)
  return e,native,function() return status end
end
local entry={get=function() return {{text="你"},{text="好"},{text="去"}} end}
local le,native,status=learning_fixture(0,85)
check(native.memorize_callback(native,entry) and native.writes==1,"one allowed native update")
check(not native.memorize_callback(native,entry) and native.writes==1,"quota refuses before native writes")
check(status()=="quota_exhausted","quota refusal visible")
le,native,status=learning_fixture(1000000000,65536)
check(not native.memorize_callback(native,entry) and native.writes==0,"oversize imported tick fails closed")
le,native,status=learning_fixture(0,65536)
local huge={get=function() return {{text=string.rep("x",257)}} end}
check(not native.memorize_callback(native,huge) and native.writes==0,"unbounded text never learned")
local corrupt={get=function() error("unverified commit") end}
check(not native.memorize_callback(native,corrupt) and status()=="storage_unverified","callback errors cannot bypass bounds")
policy.fini(le);check(native.memorize_callback==nil,"callback cycle released on fini")
print(string.format("PASS native boundary policy: %d assertions (Lua stubs, not librime)",checks))
