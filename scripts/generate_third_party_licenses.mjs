#!/usr/bin/env node

// Generate the checked-in dependency report from cargo-about and
// license-checker. Release archives use collect_licenses.mjs instead so that
// the archive contains the original per-package LICENSE/NOTICE files.

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { execFileSync } from 'node:child_process'

const root = process.cwd()
const output = path.join(root, 'THIRD_PARTY_LICENSES.md')
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'text-search-licenses-'))

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'inherit'],
    ...options,
  })
}

try {
  const cargoMarkdown = path.join(temp, 'cargo-about.md')
  run('cargo', [
    'about', 'generate',
    '--manifest-path', path.join(root, 'Cargo.toml'),
    '--config', path.join(root, 'cargo-about.toml'),
    '--output-file', cargoMarkdown,
    path.join(root, 'scripts', 'cargo-about.hbs'),
  ])
  const cargoReport = fs.readFileSync(cargoMarkdown, 'utf8')

  const npmJson = path.join(temp, 'npm-licenses.json')
  run('npx', [
    '--yes', 'license-checker@25.0.1',
    '--start', path.join(root, 'frontend'),
    '--json', '--relativeLicensePath', '--out', npmJson,
  ], { shell: process.platform === 'win32' })
  const npm = JSON.parse(fs.readFileSync(npmJson, 'utf8'))
  const npmRows = Object.entries(npm)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([pkg, info]) => {
      const license = Array.isArray(info.licenses) ? info.licenses.join(' OR ') : (info.licenses || '')
      const repository = typeof info.repository === 'string' ? info.repository : (info.repository?.url || '')
      return `| ${pkg} | ${license} | ${info.licenseFile || ''} | ${repository} |`
    })

  const header = [
    '# 第三方依赖许可证',
    '',
    '本文件由 cargo-about 和 license-checker 自动生成，请勿手工编辑。',
    '重新安装依赖或升级锁定版本后运行：',
    '',
    '    node scripts/generate_third_party_licenses.mjs',
    '',
    '## npm 依赖',
    '',
    '| 包及版本 | 许可证 | 许可证文件 | 仓库 |',
    '| --- | --- | --- | --- |',
    ...npmRows,
    '',
  ].join('\n')

  fs.writeFileSync(output, `${header}${cargoReport.trim()}\n`, 'utf8')
  console.log(`Generated ${output}`)
} finally {
  fs.rmSync(temp, { recursive: true, force: true })
}
