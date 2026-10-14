-- Bounded key/text search. Nodes retain backpointers, not copied prefix strings.
-- Native storage and candidate construction remain outside this pure search.
local M = { max_input = 128, beam_width = 8,
  max_edge = 32, max_edges = 256, max_pending = 7 }
local LOG_Q10 = 1024 / math.log(2)

function M.scorer(data, identity)
  data = data or require("xhup_flow.data.joint_scores")
  assert(data.policy == "joint-frequency-context-v2", "unsupported joint scoring policy")
  assert(type(identity) == "string" and data.identity == identity, "stale joint scoring model")
  local scale = LOG_Q10
  local offset = math.log(100000000) * scale - data.total_reward
  return function(text, weight, sound)
    local readings = data.characters[sound]
    local character = readings and readings[text]
    if character then return character end
    return math.floor(weight * scale + offset)
  end, function(previous, edge)
    local row = data.transitions[previous and previous.text or "<s>"]
    return row and row[edge.text] or 0
  end
end

local function better(score, count, serial, other)
  return score > other.score or (score == other.score and
    (count < other.count or (count == other.count and serial < other.serial)))
end

local INFINITY = math.huge
local function finite(value)
  return type(value) == "number" and value == value
    and value ~= INFINITY and value ~= -INFINITY
end

-- edges_at(start) returns bounded lexical alternatives, in deterministic order.
-- Each edge owns keys/text/source/codes/lexical; transition(previous, edge)
-- supplies a contextual score. Callbacks are deterministic and must not mutate
-- retained edges.
-- start/finish are zero-based raw-key offsets, never virtual query offsets.
function M.search(raw, edges_at, transition)
  local n = #raw
  if n == 0 or n > M.max_input or raw:find("[^a-z]") then
    return { paths = {}, reason = "unsupported_input", truncated_edges = 0 }
  end
  local root = { finish = 0, score = 0, lexical = 0, context = 0,
    count = 0, serial = 0 }
  local beams = { [0] = { root } }
  local serial, truncated = 0, 0
  local width = M.beam_width
  for start = 0, n - 1 do
    local beam = beams[start]
    if beam then
      local edges = edges_at(start)
      truncated = truncated + math.max(0, #edges - M.max_edges)
      local last_keys, finish, valid_keys
      for i = 1, math.min(#edges, M.max_edges) do
        local edge = edges[i]
        if i == 1 or edge.keys ~= last_keys then
          local length = #edge.keys
          finish = start + length
          valid_keys = length > 0 and length <= M.max_edge and finish <= n
            and raw:sub(start + 1, finish) == edge.keys
          last_keys = edge.keys
        end
        if valid_keys and finite(edge.lexical) then
          local target = beams[finish]
          if not target then target = {}; beams[finish] = target end
          local target_count = #target
          local last_previous, context, valid_context
          for j = 1, #beam do
            local previous = beam[j]
            if j == 1 or previous.edge ~= last_previous then
              context = transition and transition(previous.edge, edge) or 0
              valid_context = finite(context)
              last_previous = previous.edge
            end
            if valid_context then
              local score, count = previous.score + edge.lexical + context, previous.count + 1
              serial = serial + 1
              if (target_count < width or better(score, count, serial, target[target_count]))
                and finite(score) then
                local position = target_count + 1
                while position > 1 and better(score, count, serial, target[position - 1]) do
                  position = position - 1
                end
                local node
                if target_count == width then
                  -- finish > start: this beam has not expanded, so its evicted
                  -- node cannot be an ancestor. Never recycle an expanded node.
                  node = target[target_count]
                  target[target_count] = nil
                  node.previous, node.edge, node.finish = previous, edge, finish
                  node.score, node.count, node.serial = score, count, serial
                  node.lexical = previous.lexical + edge.lexical
                  node.context = previous.context + context
                else
                  node = { previous = previous, edge = edge, finish = finish,
                    score = score, count = count, serial = serial,
                    lexical = previous.lexical + edge.lexical,
                    context = previous.context + context }
                  target_count = target_count + 1
                end
                table.insert(target, position, node)
              end
            end
          end
        end
      end
    end
  end
  local paths = {}
  -- Keep each recent complete frontier, even when another path consumes a new
  -- ambiguous tail key. A valid prefix is not an implicit commit boundary.
  for finish = n, math.max(1, n - M.max_pending), -1 do
    local beam = beams[finish]
    if beam then
      for rank = 1, #beam do paths[#paths + 1] = beam[rank] end
    end
  end
  return { paths = paths, truncated_edges = truncated }
end

-- Native emission retains segmentation/source metadata in node backpointers.
-- Expand diagnostic arrays only when requested, not for every menu alternative.
function M.describe(raw, node, start, diagnostics)
  local edges, keys, texts, codes = {}, {}, {}, {}
  local sources, spans
  if diagnostics ~= false then sources, spans = {}, {} end
  local current = node
  while current.edge do
    edges[#edges + 1] = current.edge
    current = current.previous
  end
  local offset = start or 0
  for i = #edges, 1, -1 do
    local edge = edges[i]
    keys[#keys + 1], texts[#texts + 1] = edge.keys, edge.text
    if spans then
      sources[#sources + 1] = edge.source
      spans[#spans + 1] = { start = offset, finish = offset + #edge.keys }
    end
    offset = offset + #edge.keys
    for _, code in ipairs(edge.codes or {}) do codes[#codes + 1] = code end
  end
  return { start = start or 0, finish = offset, remaining = raw:sub(node.finish + 1),
    keys = keys, words = texts, sources = sources, spans = spans, codes = codes,
    text = table.concat(texts), preedit = table.concat(keys, " "),
    lexical = node.lexical, context = node.context, score = node.score,
    tie = node.serial, node = node }
end

-- FIFO of immutable dictionary rows only: never native candidates, user rows,
-- scores, normalization decisions or beam state. Replaced with the schema env.
local CACHE_ENTRIES, CACHE_ROWS, CACHE_BYTES = 512, 2048, 262144
function M.readonly_cache(memory, aliases)
  assert(memory.user_dict == nil and aliases.user_dict == nil, "cache must be read-only")
  return { memory = memory, aliases = aliases, canonical = {}, static = {},
    queue = {}, head = 1, count = 0, rows = 0, bytes = 0 }
end

local function remember(shared, index, code, items)
  local bytes = #code
  for _, item in ipairs(items) do bytes = bytes + #item.text + #(item.sound or "") end
  if #items > CACHE_ROWS or bytes > CACHE_BYTES then return end
  while shared.count >= CACHE_ENTRIES or shared.rows + #items > CACHE_ROWS
    or shared.bytes + bytes > CACHE_BYTES do
    local old = shared.queue[shared.head]
    old.index[old.code] = nil
    shared.queue[shared.head] = nil
    shared.head = shared.head % CACHE_ENTRIES + 1
    shared.count = shared.count - 1
    shared.rows, shared.bytes = shared.rows - #old.items, shared.bytes - old.bytes
  end
  local tail = (shared.head + shared.count - 1) % CACHE_ENTRIES + 1
  shared.queue[tail] = { index = index, code = code, items = items, bytes = bytes }
  index[code] = items
  shared.count = shared.count + 1
  shared.rows, shared.bytes = shared.rows + #items, shared.bytes + bytes
end

-- Query-local normalization budgets remain identical on cold and warm caches.
function M.lexicon(raw, memory, aliases, reverse, lengths, score, shared)
  assert(memory.user_dict == nil and aliases.user_dict == nil, "lexical lookup must be read-only")
  assert(not shared or (shared.memory == memory and shared.aliases == aliases),
    "cache dictionary owner mismatch")
  local canonical, static, normalized = {}, {}, {}
  -- Position-free immutable edges can be shared inside this query. Keep source
  -- variants separate: a budget-limited spelling may later resolve a word ID.
  local prepared = { canonical = {}, static_alias = {}, spelled_alias = {} }
  local prepared_count = 0
  local stats = { lookups = 0, reverse = 0, rows = 0, normalization_budget = 0 }
  local normalization_left = 0
  local function lookup(owner, cache, code, bound)
    local cached = cache[code]
    if cached then return cached end
    local index = shared and (owner == memory and shared.canonical or shared.static)
    local remembered = index and index[code]
    if remembered then
      cache[code] = remembered
      return remembered
    end
    local items, examined = {}, 0
    local sound = owner == memory and code:sub(1, 2) or nil
    owner:dict_lookup(code, false, bound)
    stats.lookups = stats.lookups + 1
    for entry in owner:iter_dict() do
      examined = examined + 1
      stats.rows = stats.rows + 1
      local id = owner == memory and entry.code:print() or nil
      if owner ~= memory or id:match("^%d+$") then
        items[#items + 1] = { text = entry.text, weight = entry.weight,
          id = id and tonumber(id) or nil, sound = sound }
      end
      if examined >= bound then break end
    end
    cache[code] = items
    if shared then remember(shared, index, code, items) end
    return items
  end
  local function exact_text(text)
    local cached = normalized[text]
    if cached ~= nil then return cached or nil end
    stats.reverse = stats.reverse + 1
    local best, visited = nil, 0
    for code in reverse:lookup(text):gmatch("[a-z]+") do
      visited = visited + 1
      if visited > 8 then break end
      if not canonical[code] then
        if normalization_left == 0 then
          stats.normalization_budget = stats.normalization_budget + 1
          return nil -- A budget miss is not a dictionary miss; do not cache it.
        end
        normalization_left = normalization_left - 1
      end
      for _, item in ipairs(lookup(memory, canonical, code, 64)) do
        if item.text == text and finite(item.weight)
          and (not best or item.weight > best.weight) then best = item end
      end
    end
    normalized[text] = best or false
    return best
  end
  return function(start)
    normalization_left = 32
    local edges = {}
    local remaining = #raw - start
    local native_lengths = lengths[raw:sub(start + 1, start + 2)] or ""
    local seen = {}
    local function add(code, text, codes, weight, source, fallback_score, sound)
      local key = code .. "\0" .. text
      if seen[key] then return end
      local lexical = fallback_score or score(text, weight, sound)
      if not finite(lexical) then return end
      seen[key] = true
      local cache = prepared[source]
      local edge = cache[key]
      if not edge then
        edge = { keys = code, text = text,
          codes = type(codes) == "number" and { codes } or codes,
          lexical = lexical, source = source }
        if prepared_count < 1024 then
          cache[key] = edge
          prepared_count = prepared_count + 1
        end
      end
      edges[#edges + 1] = edge
    end
    -- Native edges come first, preserving their exact canonical syllable IDs
    -- when a static spelling denotes the same text.
    for i = 1, #native_lengths do
      local length = native_lengths:byte(i)
      if length <= remaining and length <= M.max_edge then
        local code = raw:sub(start + 1, start + length)
        local items = lookup(memory, canonical, code, 64)
        for j = 1, math.min(8, #items) do
          local item = items[j]
          add(code, item.text, item.id, item.weight, "canonical", nil, item.sound)
        end
      end
    end
    -- The frozen static dictionary is the authoritative read-only alias index.
    -- PRIMARY includes five-key spellings; it is not restricted to four keys.
    for length = 1, math.min(5, remaining) do
      local code = raw:sub(start + 1, start + length)
      for _, item in ipairs(lookup(aliases, static, code, 8)) do
        if not seen[code .. "\0" .. item.text] then
          local canonical_item = exact_text(item.text)
          if canonical_item then
            add(code, item.text, canonical_item.id, canonical_item.weight,
              "static_alias", nil, canonical_item.sound)
          elseif #item.text <= 128 then
            local codes, lexical, characters = {}, 0, {}
            for character in item.text:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
              local primitive = exact_text(character)
              if not primitive or #codes == 32 then codes = {}; break end
              codes[#codes + 1] = primitive.id
              characters[#characters + 1] = character
              lexical = lexical + score(character, primitive.weight, primitive.sound)
            end
            if #codes > 0 and table.concat(characters) == item.text then
              add(code, item.text, codes, nil, "spelled_alias", lexical)
            end
          end
        end
      end
    end
    return edges
  end, stats
end

local function native_entry(path)
  if path.native_entry then return path.native_entry end
  assert(#path.codes > 0 and #path.codes <= 1024, "native code budget exceeded")
  local entry = DictEntry()
  -- Code() returns a shared-pointer wrapper. This getter supplies the Code
  -- value required by DictEntry.code on supported native bindings.
  local code = entry.code
  for _, id in ipairs(path.codes) do
    assert(id >= 0 and id % 1 == 0, "invalid native syllable")
    code:push(id)
  end
  entry.text, entry.code, entry.weight = path.text, code, path.score
  path.native_entry = entry
  return entry
end

-- Query the native serialized key prefix, not an unbounded homophone bucket.
-- A trailing space + TAB separates canonical syllables from the target text.
-- Predictive mode makes the native limit apply even to exact code matches.
-- Only a byte-exact code/text record with an actual commit can affect ranking.
function M.user_reward(path, memory, dictionary)
  if not dictionary then return 0 end
  local entry = native_entry(path)
  local code = table.concat(memory.dict:decode(entry.code), " ") .. " "
  local found = dictionary:lookup_words(code .. "\t" .. path.text, true, 1)
  for learned in found:iter() do
    if learned.text == path.text and learned.custom_code == code
      and learned.remaining_code_length == 0 and learned.commit_count > 0
      and finite(learned.weight) then
      -- Reuse librime's decayed user probability, including its native exact
      -- credibility. No second persistence ledger or invented commit counter.
      return math.max(0, math.floor(learned.weight * LOG_Q10) - path.score)
    end
    break
  end
  return 0
end

-- Real Phrase identity; the sole writer receives the ordinary native commit.
function M.candidate(path, memory, language, quality)
  assert(memory.user_dict == nil and memory.lang_name == language, "native language/readonly mismatch")
  local entry = native_entry(path)
  entry.weight = path.score
  local phrase = Phrase(memory, "phrase", path.start, path.finish, entry)
  phrase.preedit, phrase.quality = path.preedit, quality
  local candidate = phrase:toCandidate()
  if path.remaining ~= "" then candidate.comment = "待续:" .. path.remaining end
  path.native_candidate, path.native_language = candidate, language
  return candidate
end

return M
