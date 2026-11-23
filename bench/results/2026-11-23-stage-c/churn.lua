local function churn(n)
  local s = 0
  for i = 0, n - 1 do
    s = s + (i * i) % 7
  end
  return s
end
print(churn(3000000))
