def churn(n):
    s = 0
    for i in range(n):
        s = s + (i * i) % 7
    return s


print(churn(3000000))
