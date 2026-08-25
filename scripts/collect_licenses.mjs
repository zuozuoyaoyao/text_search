#!/usr/bin/env node

// Collect license and NOTICE files for a release package.
// The script intentionally reads the exact Cargo/npm packages installed for
// this checkout instead of maintaining a hand-written dependency list.

import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { execFileSync } from 'node:child_process'

const root = process.cwd()
const args = process.argv.slice(2)
let output = null
for (let i = 0; i < args.length; i += 1) {
  if (args[i] === '--output' && args[i + 1]) output = path.resolve(args[++i])
  if (args[i] === '--root' && args[i + 1]) process.chdir(path.resolve(args[++i]))
}
if (!output) {
  console.error('Usage: node scripts/collect_licenses.mjs --output <directory>')
  process.exit(2)
}

const projectRoot = process.cwd()
fs.mkdirSync(output, { recursive: true })

const safe = (value) => String(value || 'unknown').replace(/[^a-zA-Z0-9._-]+/g, '_')
const licenseFile = (name) => /^(license|copying|notice)([-_.].*)?$/i.test(name)
const records = []
const usedNames = new Set()

function copyLicenseFiles(packageKind, name, version, packageRoot, declaredLicense) {
  if (!packageRoot || !fs.existsSync(packageRoot)) return
  const entries = fs.readdirSync(packageRoot, { withFileTypes: true })
    .filter((entry) => entry.isFile() && licenseFile(entry.name))
  const prefix = `${packageKind}-${safe(name)}-${safe(version)}`
  let copied = 0
  for (const entry of entries) {
    let destination = `${prefix}-${safe(entry.name)}`
    let suffix = 2
    while (usedNames.has(destination)) destination = `${prefix}-${safe(entry.name)}-${suffix++}`
    usedNames.add(destination)
    fs.copyFileSync(path.join(packageRoot, entry.name), path.join(output, destination))
    copied += 1
  }
  if (!copied) {
    const destination = `${prefix}-DECLARED.txt`
    fs.writeFileSync(
      path.join(output, destination),
      `${name} ${version}\nDeclared license: ${declaredLicense || 'not declared'}\n\nNo license file was present at the package root. Consult the package metadata/source for the authoritative license.\n`,
      'utf8',
    )
    copied = 1
  }
  records.push({ kind: packageKind, name, version, license: declaredLicense || '', files: copied })
}

function cargoMetadata() {
  try {
    const text = execFileSync('cargo', ['metadata', '--format-version', '1'], {
      cwd: projectRoot,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'inherit'],
    })
    return JSON.parse(text)
  } catch (error) {
    console.warn(`Warning: cargo metadata unavailable: ${error.message}`)
    return null
  }
}

const metadata = cargoMetadata()
if (metadata) {
  for (const pkg of metadata.packages || []) {
    // Workspace packages are the project itself; their license is copied below.
    if (!pkg.source) continue
    copyLicenseFiles(
      'cargo',
      pkg.name,
      pkg.version,
      path.dirname(pkg.manifest_path),
      pkg.license,
    )
  }
}

function collectNpm(rootDir) {
  if (!fs.existsSync(rootDir)) {
    console.warn(`Warning: npm directory not found: ${rootDir}`)
    return
  }
  const packages = []
  const visit = (dir) => {
    if (!fs.existsSync(dir)) return
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory() && entry.name.startsWith('@')) visit(full)
      else if (entry.isDirectory()) {
        const manifest = path.join(full, 'package.json')
        if (fs.existsSync(manifest)) packages.push(manifest)
        const nested = path.join(full, 'node_modules')
        if (fs.existsSync(nested)) visit(nested)
      }
    }
  }
  visit(rootDir)
  for (const manifestPath of packages) {
    try {
      const pkg = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
      copyLicenseFiles('npm', pkg.name, pkg.version, path.dirname(manifestPath), pkg.license)
    } catch (error) {
      console.warn(`Warning: unable to read ${manifestPath}: ${error.message}`)
    }
  }
}
collectNpm(path.join(projectRoot, 'frontend', 'node_modules'))

function copyProjectFile(source, destination) {
  if (fs.existsSync(source)) fs.copyFileSync(source, path.join(output, destination))
}
copyProjectFile(path.join(projectRoot, 'LICENSE'), 'PROJECT-LICENSE')
copyProjectFile(path.join(projectRoot, 'THIRD_PARTY_LICENSES.md'), 'THIRD_PARTY_LICENSES.md')
const standardDir = path.join(projectRoot, 'LICENSES')
if (fs.existsSync(standardDir)) {
  for (const entry of fs.readdirSync(standardDir, { withFileTypes: true })) {
    if (entry.isFile()) copyProjectFile(path.join(standardDir, entry.name), `PROJECT-${entry.name}`)
  }
}

const lines = [
  'Text Search release license inventory',
  `Generated: ${new Date().toISOString()}`,
  '',
  'kind\tpackage\tversion\tdeclared-license\tlicense-files-copied',
  ...records
    .sort((a, b) => `${a.kind}:${a.name}`.localeCompare(`${b.kind}:${b.name}`))
    .map((r) => `${r.kind}\t${r.name}\t${r.version}\t${r.license}\t${r.files}`),
]
fs.writeFileSync(path.join(output, 'MANIFEST.tsv'), `${lines.join('\n')}\n`, 'utf8')
console.log(`Collected ${records.length} dependency license records in ${output}`)
