import sys
scale = int(sys.argv[1]) if len(sys.argv) > 1 else 1
parts = []
for i in range(80000 * scale):
    parts.append(f"foo-{i}-bar")
text = " ".join(parts)
replaced = text.replace("foo", "FOO")
found = text.find("bar")
words = text.split(" ")
print(len(replaced), found, len(words))
