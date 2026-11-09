const scale = parseInt(process.argv[2] ?? "1");
const parts = [];
for (let i = 0; i < 80000 * scale; i++) parts.push(`foo-${i}-bar`);
const text = parts.join(" ");
const replaced = text.replace(/foo/g, "FOO");
const found = text.indexOf("bar");
const words = text.split(" ");
console.log(replaced.length, found, words.length);
