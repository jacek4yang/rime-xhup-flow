-- Bounded key/text search. Nodes retain backpointers, not copied prefix strings.
-- Native storage and candidate construction remain outside this pure search.
local M = { max_input = 128, beam_width = 8,
  max_edge = 32, max_edges = 256, max_pending = 7 }
local LOG_Q10 = 1024 / math.log(2)

function M.scorer(data, identity)
  data = data or require("xhup_flow.data.joint_scores")
  assert(data.policy == "joint-frequency-context-shape-v3", "unsupported joint scoring policy")
  assert(type(identity) == "string" and data.identity == identity, "stale joint scoring model")
  assert(type(data.primary_full_code_split_penalty) == "number"
    and data.primary_full_code_split_penalty >= 0
    and data.primary_full_code_split_penalty <= 32768, "invalid shape compatibility cost")
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
  end, data.primary_full_code_split_penalty
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
  -- Query-local columns avoid a hash table per beam node. IDs are bounded by
  -- 1 + max_input * beam_width; only backpointers survive in returned paths.
  local previous_ids, node_edges = {}, {}
  local scores, counts, serials = {0}, {0}, {0}
  local lexical_scores, contexts, compatibilities = {0}, {0}, {0}
  local allocated = 1
  local beams = { [0] = {1} }
  local function better(score, count, serial, other)
    return score > scores[other] or (score == scores[other] and
      (count < counts[other] or (count == counts[other] and serial < serials[other])))
  end
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
        local compatibility = edge.compatibility or 0
        if valid_keys and finite(edge.lexical) and finite(compatibility) then
          local target = beams[finish]
          if not target then target = {}; beams[finish] = target end
          local target_count = #target
          local last_previous, context, valid_context
          for j = 1, #beam do
            local previous = beam[j]
            local previous_edge = node_edges[previous]
            if j == 1 or previous_edge ~= last_previous then
              context = transition and transition(previous_edge, edge) or 0
              valid_context = finite(context)
              last_previous = previous_edge
            end
            if valid_context then
              local score = scores[previous] + edge.lexical + context + compatibility
              local count = counts[previous] + 1
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
                else
                  allocated = allocated + 1
                  node = allocated
                  target_count = target_count + 1
                end
                previous_ids[node], node_edges[node] = previous, edge
                scores[node], counts[node], serials[node] = score, count, serial
                lexical_scores[node] = lexical_scores[previous] + edge.lexical
                contexts[node] = contexts[previous] + context
                compatibilities[node] = compatibilities[previous] + compatibility
                table.insert(target, position, node)
              end
            end
          end
        end
      end
    end
  end
  local paths = {}
  local arena = { previous = previous_ids, edges = node_edges }
  -- Keep each recent complete frontier, even when another path consumes a new
  -- ambiguous tail key. A valid prefix is not an implicit commit boundary.
  for finish = n, math.max(1, n - M.max_pending), -1 do
    local beam = beams[finish]
    if beam then
      for rank = 1, #beam do
        local id = beam[rank]
        paths[#paths + 1] = { arena = arena, id = id, finish = finish,
          score = scores[id], count = counts[id], serial = serials[id],
          lexical = lexical_scores[id], context = contexts[id],
          compatibility = compatibilities[id] }
      end
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
  local current, arena = node.id, node.arena
  while arena.edges[current] do
    edges[#edges + 1] = arena.edges[current]
    current = arena.previous[current]
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
    compatibility = node.compatibility,
    tie = node.serial, node = node }
end

-- FIFO of immutable dictionary rows only: never native candidates, user rows,
-- scores, budget decisions or beam state. Replaced with the schema env.
local CACHE_ENTRIES, CACHE_ROWS, CACHE_BYTES = 512, 2048, 262144
function M.readonly_cache(memory, aliases, reverse)
  assert(memory.user_dict == nil and aliases.user_dict == nil, "cache must be read-only")
  return { memory = memory, aliases = aliases, reverse = reverse,
    canonical = {}, static = {}, projection = {},
    queue = {}, head = 1, count = 0, rows = 0, bytes = 0 }
end

local function remember(shared, index, code, items)
  local bytes = #code
  for _, item in ipairs(items) do
    bytes = bytes + #item.text + #(item.sound or "") + #(item.spelling or "")
  end
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
function M.lexicon(raw, memory, aliases, reverse, lengths, score, shared, split_penalty)
  split_penalty = split_penalty or 0
  assert(finite(split_penalty) and split_penalty >= 0, "invalid shape compatibility cost")
  assert(memory.user_dict == nil and aliases.user_dict == nil, "lexical lookup must be read-only")
  assert(not shared or (shared.memory == memory and shared.aliases == aliases and shared.reverse == reverse),
    "cache dictionary owner mismatch")
  local canonical, static, normalized = {}, {}, {}
  local normalization_seen = {}
  -- Compatibility depends on the following keys. Never reuse a penalized edge
  -- at a position without the same primary full-code evidence (or vice versa).
  local prepared = {
    plain = { canonical = {}, static_alias = {}, spelled_alias = {} },
    split = { canonical = {}, static_alias = {}, spelled_alias = {} },
  }
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
      -- Only eight accepted rows form lexical edges. Alias eligibility uses the
      -- separately verified 64-row projection, not this materialized shortlist.
      if #items == 8 or examined >= bound then break end
    end
    cache[code] = items
    if shared then remember(shared, index, code, items) end
    return items
  end
  local function projected_rows(text)
    local cached = shared and shared.projection[text]
    if cached then return cached end
    stats.reverse = stats.reverse + 1
    local items = {}
    for value in reverse:lookup(text):gmatch("%S+") do
      if #items == 8 then break end
      local code, id, weight = value:match("^([a-z]+)!(%d+)!(.+)$")
      if not code then code = value:match("^([a-z]+)!x$") end
      assert(code, "invalid canonical projection record")
      if id then
        id, weight = tonumber(id), tonumber(weight)
        assert(id >= 0 and id <= 2147483647 and finite(weight), "invalid canonical projection value")
        local entry = DictEntry()
        local native_code = entry.code
        native_code:push(id)
        local decoded = memory.dict:decode(native_code)
        assert(#decoded == 1 and decoded[1] == code, "stale canonical projection ID")
      end
      items[#items + 1] = { text = text, spelling = code, id = id,
        weight = weight, sound = code:sub(1, 2) }
    end
    if shared then remember(shared, shared.projection, text, items) end
    return items
  end
  local function exact_text(text)
    local cached = normalized[text]
    if cached ~= nil then return cached or nil end
    local best
    for _, item in ipairs(projected_rows(text)) do
      local code = item.spelling
      -- Replay cold-query quotas even when immutable projection rows are cached.
      if not canonical[code] and not normalization_seen[code] then
        if normalization_left == 0 then
          stats.normalization_budget = stats.normalization_budget + 1
          return nil -- A budget miss is not a dictionary miss; do not cache it.
        end
        normalization_left = normalization_left - 1
      end
      normalization_seen[code] = true
      if item.id and (not best or item.weight > best.weight) then best = item end
    end
    normalized[text] = best or false
    return best
  end
  return function(start)
    normalization_left = 32
    local edges = {}
    local remaining = #raw - start
    local native_lengths = lengths[raw:sub(start + 1, start + 2)] or ""
    local primary
    if split_penalty > 0 and remaining >= 4 and native_lengths:find("\4", 1, true) then
      local full_code = raw:sub(start + 1, start + 4)
      local first = lookup(aliases, static, full_code, 8)[1]
      if first and first.text:match("^[%z\1-\127\194-\244][\128-\191]*$") then
        for _, item in ipairs(lookup(memory, canonical, full_code, 64)) do
          if item.text == first.text then primary = first.text; break end
        end
      end
    end
    local seen = {}
    local function add(code, text, codes, weight, source, fallback_score, sound)
      local key = code .. "\0" .. text
      if seen[key] then return end
      local split = primary and (#code < 4 or (#code == 4 and text ~= primary))
      local cache = (split and prepared.split or prepared.plain)[source]
      local edge = cache[key]
      if not edge then
        local lexical = fallback_score or score(text, weight, sound)
        if not finite(lexical) then return end
        edge = { keys = code, text = text,
          codes = type(codes) == "number" and { codes } or codes,
          lexical = lexical, compatibility = split and -split_penalty or 0, source = source }
        if prepared_count < 1024 then
          cache[key] = edge
          prepared_count = prepared_count + 1
        end
      end
      seen[key] = true
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
