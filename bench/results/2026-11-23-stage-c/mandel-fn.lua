local function escape(cr, ci, limit)
  local x, y, i = 0.0, 0.0, 0
  while i < limit do
    local xx, yy = x * x, y * y
    if xx + yy > 4.0 then return i end
    y = 2.0 * x * y + ci
    x = xx - yy + cr
    i = i + 1
  end
  return limit
end
local function mandel(n, limit)
  local inside = 0
  for py = 0, n - 1 do
    for px = 0, n - 1 do
      local cr = px * 3.0 / n - 2.0
      local ci = py * 2.0 / n - 1.0
      if escape(cr, ci, limit) == limit then inside = inside + 1 end
    end
  end
  return inside
end
print(mandel(400, 100))
