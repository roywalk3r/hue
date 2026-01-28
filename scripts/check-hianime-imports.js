const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..', 'vendor', 'hianime-v1', 'src');
const exts = new Set(['.js', '.json', '.mjs', '.cjs', '.node']);

function isRelative(spec) {
  return spec.startsWith('./') || spec.startsWith('../');
}

function hasExtension(spec) {
  return exts.has(path.extname(spec));
}

function walk(dir, files = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, files);
    else if (entry.isFile() && entry.name.endsWith('.js')) files.push(full);
  }
  return files;
}

const importRe = /from\s+['"]([^'"]+)['"]/g;
const dynamicRe = /import\(\s*['"]([^'"]+)['"]\s*\)/g;

const offenders = [];
for (const file of walk(root)) {
  const text = fs.readFileSync(file, 'utf8');
  let match;
  while ((match = importRe.exec(text)) !== null) {
    const spec = match[1];
    if (isRelative(spec) && !hasExtension(spec)) {
      offenders.push({ file, spec });
    }
  }
  while ((match = dynamicRe.exec(text)) !== null) {
    const spec = match[1];
    if (isRelative(spec) && !hasExtension(spec)) {
      offenders.push({ file, spec });
    }
  }
}

if (offenders.length) {
  console.error('Found relative imports without extensions:');
  for (const off of offenders) {
    console.error(`- ${path.relative(process.cwd(), off.file)}: ${off.spec}`);
  }
  process.exit(1);
}

console.log('OK: all HiAnime relative imports include extensions.');
